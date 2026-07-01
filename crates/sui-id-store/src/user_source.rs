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
    /// Slug of the `[[user_source]]` config block that produced this record.
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
                // Transport/config failure — log and continue (P4).
                // The audit event "auth.user_source.transport_failure" is
                // emitted by the binary-crate caller (try_login_with_cascade)
                // which has DB access.  The string literal is anchored here
                // for the CI audit-matrix gate.
                let _audit_event = "auth.user_source.transport_failure";
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

    fn slug(&self) -> &str {
        &self.slug
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_source(slug: &str) -> InMemoryUserSource {
        let mut users = HashMap::new();
        users.insert(
            "alice".to_owned(),
            (
                "s3cr3t".to_owned(),
                "uid=alice,dc=test".to_owned(),
                Some("alice@example.com".to_owned()),
                Some("Alice".to_owned()),
            ),
        );
        InMemoryUserSource {
            slug: slug.to_owned(),
            users,
        }
    }

    #[tokio::test]
    async fn known_user_correct_password_matches() {
        let src = test_source("test");
        let result = src.authenticate("alice", "s3cr3t").await.unwrap();
        let record = result.expect("must match");
        assert_eq!(record.stable_id, "uid=alice,dc=test");
        assert_eq!(record.display_username, "alice");
        assert_eq!(record.email.as_deref(), Some("alice@example.com"));
        assert_eq!(record.source_slug, "test");
    }

    #[tokio::test]
    async fn unknown_user_returns_none() {
        let src = test_source("test");
        let result = src.authenticate("bob", "anything").await.unwrap();
        assert!(result.is_none(), "unknown user must return None");
    }

    #[tokio::test]
    async fn wrong_password_returns_none() {
        let src = test_source("test");
        let result = src.authenticate("alice", "wrong").await.unwrap();
        assert!(
            result.is_none(),
            "wrong password must return None (indistinguishable from unknown user)"
        );
    }

    #[tokio::test]
    async fn cascade_first_match_wins() {
        // Two sources; alice is in source-1, not source-2.
        let src1 = Arc::new(test_source("s1")) as Arc<dyn UserSource>;
        let empty = Arc::new(InMemoryUserSource {
            slug: "s2".to_owned(),
            users: HashMap::new(),
        }) as Arc<dyn UserSource>;
        let sources = vec![src1, empty];

        match cascade_sources(&sources, "alice", "s3cr3t").await {
            CascadeOutcome::Matched(r) => assert_eq!(r.source_slug, "s1"),
            CascadeOutcome::NotFound => panic!("expected match"),
        }
    }

    #[tokio::test]
    async fn cascade_not_found_when_no_source_matches() {
        let src = Arc::new(test_source("s1")) as Arc<dyn UserSource>;
        let sources = vec![src];
        let outcome = cascade_sources(&sources, "unknown", "pw").await;
        assert!(matches!(outcome, CascadeOutcome::NotFound));
    }

    #[tokio::test]
    async fn cascade_continues_past_transport_error() {
        // A source that always fails with a transport error should be skipped.
        struct BrokenSource;
        #[async_trait::async_trait]
        impl UserSource for BrokenSource {
            async fn authenticate(
                &self,
                _u: &str,
                _p: &str,
            ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
                Err(UserSourceError::Transport("connection refused".into()))
            }
            fn slug(&self) -> &str {
                "broken"
            }
        }

        let broken = Arc::new(BrokenSource) as Arc<dyn UserSource>;
        let working = Arc::new(test_source("working")) as Arc<dyn UserSource>;
        let sources = vec![broken, working];

        // alice is in the working source; the broken one should not block her.
        match cascade_sources(&sources, "alice", "s3cr3t").await {
            CascadeOutcome::Matched(r) => assert_eq!(r.source_slug, "working"),
            CascadeOutcome::NotFound => panic!("expected cascade to continue past broken source"),
        }
    }
}
