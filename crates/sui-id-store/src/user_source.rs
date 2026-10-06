//! Pluggable user-source trait for the auth cascade (RFC 005).
//!
//! A `UserSource` supplies authentication against an external identity
//! provider (e.g. LDAP).  The auth cascade tries the local credential store
//! first, then any configured `UserSource` implementations in order.  The
//! first source returning `Ok(Some(_))` wins.
//!
//! # Design constraints
//!
//! - **Read-only.** Sources never write to the directory.
//! - **Local-first, hardcoded.** The cascade order is local → external
//!   sources.  This is never configurable: the local admin is always the
//!   escape hatch even if every external source is misconfigured (P4).
//! - **Fail-soft.** A transport error from a source (directory unreachable)
//!   is logged and the cascade continues to the next source.
//! - **Timing equivalence.** Implementations must return `Ok(None)` for both
//!   unknown-user and wrong-password, spending comparable time on both
//!   branches so an attacker cannot distinguish them by timing (P3).

use std::fmt;
use std::sync::Arc;

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors from a `UserSource`.
///
/// `Ok(None)` means "this source does not know this user" (or "wrong
/// password") — the cascade continues.  `Err(UserSourceError)` means a
/// *transport* failure (directory unreachable, TLS negotiation failed) —
/// the failure is logged and the cascade continues (fail-soft, P4).
#[derive(Debug)]
pub enum UserSourceError {
    /// The directory could not be reached (network, TLS, timeout).
    Transport(String),
    /// The service-account bind failed (misconfigured credentials).
    ServiceBind(String),
    /// A configuration error caught at connect time.
    Config(String),
}

impl fmt::Display for UserSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(m) => write!(f, "user-source transport error: {m}"),
            Self::ServiceBind(m) => write!(f, "user-source service-account bind failed: {m}"),
            Self::Config(m) => write!(f, "user-source configuration error: {m}"),
        }
    }
}

impl std::error::Error for UserSourceError {}

// ── ExternalUserRecord ────────────────────────────────────────────────────────

/// Information returned by a `UserSource` on successful authentication.
///
/// Used to create or update the local *shadow* row in the `users` table.
/// None of these fields are trusted for authorization — they are display
/// metadata only.
#[derive(Debug, Clone)]
pub struct ExternalUserRecord {
    /// Opaque stable identifier from the external source (DN, objectGUID,
    /// entryUUID, …).  Must never change for the lifetime of this identity
    /// so that display-field changes do not create a second shadow row.
    pub stable_id: String,
    /// Display username to use when creating the local shadow row.
    /// Derived from the upstream `uid`/`sAMAccountName`/`preferred_username`
    /// attribute; conflict-resolved with a numeric suffix at shadow creation
    /// time if the name is already taken.
    pub display_username: String,
    /// Email address from the upstream, if available.
    pub email: Option<String>,
    /// Display name from the upstream, if available (`cn`, `displayName`, …).
    pub display_name: Option<String>,
    /// Slug of the `[[user_sources]]` config block that produced this record.
    /// Used in audit log notes.
    pub source_slug: String,
}

// ── UserSource trait ──────────────────────────────────────────────────────────

/// A read-only external identity source for the auth cascade.
///
/// Implementors must be `Send + Sync` (the cascade is async and the
/// `Arc<dyn UserSource>` is shared across request threads).
#[async_trait::async_trait]
pub trait UserSource: Send + Sync {
    /// Attempt to authenticate `username` with `password`.
    ///
    /// Returns:
    /// - `Ok(Some(record))` — authentication succeeded; populate the local
    ///   shadow row with the returned record.
    /// - `Ok(None)` — this source does not recognise `username`, or the
    ///   password is wrong.  Both cases **must be indistinguishable** to
    ///   callers — the cascade continues to the next source.
    /// - `Err(UserSourceError)` — transport/service-account failure.  The
    ///   cascade logs the error and continues (P4 fail-soft); does NOT return
    ///   an authentication failure to the end user.
    async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError>;

    /// Authenticate the entry identified by `stable_id` — the
    /// `ExternalUserRecord::stable_id` a previous `authenticate` returned
    /// and a shadow row stores — with `password`. Used for a returning
    /// directory user, whose local username may differ from the name the
    /// directory knows (a collision suffix), and for RFC 102 B7's re-bind.
    ///
    /// Same result contract as [`authenticate`](Self::authenticate): an
    /// unknown id and a wrong password are both `Ok(None)` (P3), and a
    /// source's own restrictions on who may sign in still apply.
    async fn authenticate_stable_id(
        &self,
        stable_id: &str,
        password: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError>;

    /// Human-readable slug for audit log notes (matches the config block slug).
    fn slug(&self) -> &str;
}

// ── Cascade ───────────────────────────────────────────────────────────────────

/// Result of the user-source cascade.
pub enum CascadeOutcome {
    /// A source authenticated the user; returns the external record.
    Matched(ExternalUserRecord),
    /// No source authenticated the user (unknown or wrong password).
    NotFound,
}

/// Run the pluggable user-source cascade.
///
/// Tries each source in `sources` in order.  Returns on the first
/// `Ok(Some(_))`.  Logs and continues on `Err(_)` (P4 fail-soft).  Returns
/// `CascadeOutcome::NotFound` if all sources return `Ok(None)` or an error.
pub async fn cascade_sources(
    sources: &[Arc<dyn UserSource>],
    username: &str,
    password: &str,
) -> CascadeOutcome {
    for source in sources {
        match source.authenticate(username, password).await {
            Ok(Some(record)) => {
                tracing::debug!(source = source.slug(), "user-source cascade matched");
                return CascadeOutcome::Matched(record);
            }
            Ok(None) => {
                // This source doesn't know the user — try the next.
                tracing::trace!(source = source.slug(), "user-source cascade miss");
            }
            Err(e) => {
                // Transport or config failure: logged; the cascade continues (P4). No audit row.
                tracing::warn!(
                    source = source.slug(),
                    error = %e,
                    "user-source transport failure; continuing cascade"
                );
            }
        }
    }
    CascadeOutcome::NotFound
}

// ── In-memory test source (used in tests and as a no-op placeholder) ─────────

/// A simple in-memory user source for testing.  Accepts any user whose
/// username is in the provided map with the exact matching password.
///
/// `Ok(None)` is returned for unknown users and wrong passwords — consistent
/// with the timing-equivalence requirement (P3) (no timing guarantee for test
/// code, only for production implementations).
pub struct InMemoryUserSource {
    pub slug: String,
    /// Map of username → (password, external_stable_id, email, display_name)
    pub users: std::collections::HashMap<String, (String, String, Option<String>, Option<String>)>,
}

#[async_trait::async_trait]
impl UserSource for InMemoryUserSource {
    async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
        let Some((stored_pw, stable_id, email, display_name)) = self.users.get(username) else {
            return Ok(None);
        };
        if stored_pw != password {
            return Ok(None);
        }
        Ok(Some(ExternalUserRecord {
            stable_id: stable_id.clone(),
            display_username: username.to_owned(),
            email: email.clone(),
            display_name: display_name.clone(),
            source_slug: self.slug.clone(),
        }))
    }

    async fn authenticate_stable_id(
        &self,
        stable_id: &str,
        password: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
        let Some((username, (stored_pw, id, email, display_name))) =
            self.users.iter().find(|(_, (_, id, _, _))| id == stable_id)
        else {
            return Ok(None);
        };
        if stored_pw != password {
            return Ok(None);
        }
        Ok(Some(ExternalUserRecord {
            stable_id: id.clone(),
            display_username: username.clone(),
            email: email.clone(),
            display_name: display_name.clone(),
            source_slug: self.slug.clone(),
        }))
    }

    fn slug(&self) -> &str {
        &self.slug
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;
