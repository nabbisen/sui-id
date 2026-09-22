//! Administrator-issued recovery links: the data path (RFC 103 stage 3).
//!
//! An administrator, or the operator on the host, issues a **link, never a
//! password** (D1). The link is a `password_reset_tokens` row, redeemed at the
//! existing `/reset-password` completion (U10). This module generates the
//! token, hands its hash to the sealed command U37, and returns the plaintext
//! to the caller exactly once. It builds no URL and has no HTTP or CLI
//! surface: those are later stages.
//!
//! The plaintext token is generated outside the database transaction and
//! only its SHA-256 is stored. It is wrapped in [`RecoveryToken`], whose
//! `Debug` output is redacted and which zeroes its memory on drop, so a stray
//! `{:?}` or `tracing` field cannot leak it. Callers must never log it.

use crate::actor::AdminActor;
use crate::errors::{CoreError, CoreResult};
use crate::forgot_password::{DEFAULT_TOKEN_TTL, mint_random_token};
use crate::time::SharedClock;
use chrono::{DateTime, Utc};
use sui_id_shared::ids::UserId;
use sui_id_store::Database;
use sui_id_store::errors::RecoveryRefusal;
use zeroize::Zeroize;

#[cfg(test)]
#[path = "recovery_link/tests.rs"]
mod tests;

/// The plaintext recovery token. Shown once, to whoever issued the link.
pub struct RecoveryToken(String);

impl RecoveryToken {
    /// The token text, for building the link or printing it once. Do not log.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for RecoveryToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryToken([redacted])")
    }
}

impl Drop for RecoveryToken {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// The link an administrator or operator hands to the user:
/// `<issuer>/reset-password#t=<token>` (RFC 103 D10). The token is in the
/// URL **fragment**, which a browser does not send, so it reaches no proxy, no
/// access log and no server log. The same base, `server.issuer`, is used for
/// every origin. A trailing slash on the issuer is dropped, so the path is
/// never doubled.
///
/// The returned string contains the token. Show it to whoever issued the link
/// once; never log it.
pub fn completion_url(issuer: &str, token: &RecoveryToken) -> String {
    format!(
        "{}/reset-password#t={}",
        issuer.trim_end_matches('/'),
        token.expose()
    )
}

/// A freshly issued recovery link.
#[derive(Debug)]
pub struct RecoveryLink {
    pub token: RecoveryToken,
    pub expires_at: DateTime<Utc>,
    /// How many of the target's outstanding links this issuance invalidated.
    pub invalidated: usize,
}

/// Issue a recovery link for `target` as the administrator `actor` (the web
/// operation). Refusals are `CoreError::Store(StoreError::RecoveryRefused(_))`
/// or `StepUpRequired`; see
/// [`sui_id_store::commands::issue_recovery_link_as_admin`].
pub async fn issue_as_admin(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: UserId,
    reason: &str,
) -> CoreResult<RecoveryLink> {
    let (plaintext, hash) = mint_random_token()?;
    let token = RecoveryToken(plaintext);
    let now = clock.now();
    let expires_at = now + DEFAULT_TOKEN_TTL;
    let grant = sui_id_store::commands::issue_recovery_link_as_admin(
        db,
        actor.user_id(),
        actor.session_id(),
        target,
        hash,
        reason.to_owned(),
        expires_at,
        now,
    )
    .await?
    .into_inner();
    Ok(RecoveryLink {
        token,
        expires_at: grant.expires_at,
        invalidated: grant.invalidated,
    })
}

/// Issue a recovery link for the local user named `username` as the operator
/// on the host (the CLI operation): the system principal, no session. Returns
/// the user's id with the link. An unknown username is refused as
/// `TargetUnknown`, like an unknown id.
pub async fn issue_as_operator(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    reason: &str,
) -> CoreResult<(UserId, RecoveryLink)> {
    let user = sui_id_store::repos::users::find_by_username(db, username)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => CoreError::Store(
                sui_id_store::StoreError::RecoveryRefused(RecoveryRefusal::TargetUnknown),
            ),
            other => CoreError::from(other),
        })?;
    let (plaintext, hash) = mint_random_token()?;
    let token = RecoveryToken(plaintext);
    let now = clock.now();
    let expires_at = now + DEFAULT_TOKEN_TTL;
    let grant = sui_id_store::commands::issue_recovery_link_as_operator(
        db,
        user.id,
        hash,
        reason.to_owned(),
        expires_at,
        now,
    )
    .await?
    .into_inner();
    Ok((
        user.id,
        RecoveryLink {
            token,
            expires_at: grant.expires_at,
            invalidated: grant.invalidated,
        },
    ))
}

/// A signed-in user's own most recent recovery-link event, for the account
/// page (RFC 103 5b). Built from an `audit_log` row by
/// [`summarize_recent_event`]; carries no token, no reason, and no actor's
/// identity — only what kind of actor and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryEventSummary {
    /// `user.recovery_link.issued`: a link exists (or existed) for this
    /// user, issued on the web by an administrator (`true`) or by the
    /// operator CLI (`false`).
    Issued { by_admin: bool, at: DateTime<Utc> },
    /// `auth.password.reset_completed`: the password was last reset through
    /// a link of this origin.
    Completed {
        origin: sui_id_store::models::ResetTokenOrigin,
        at: DateTime<Utc>,
    },
}

/// The value of the last `key=…` field in a `key=value`-joined audit note,
/// up to the next space. Notes are not escaped (RFC 102 stage 3 finding,
/// authorized then, not yet built): a free-text field earlier in the note
/// (U37's `reason`) could contain something that looks like `key=`, so this
/// reads from the **end**, matching the fields U37 and U10 always write
/// last and the convention documented in `operators.md`.
fn last_note_field<'a>(note: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    note.split(' ')
        .rev()
        .find_map(|tok| tok.strip_prefix(prefix.as_str()))
}

/// Interpret one `audit_log` row as a [`RecoveryEventSummary`]. `None` for
/// any row that is not one of the two events this reads, or whose note does
/// not carry the field this needs (a row from before that field existed, or
/// a corrupt one) — the caller treats that the same as no event.
pub fn summarize_recent_event(
    row: &sui_id_store::models::AuditLogRow,
) -> Option<RecoveryEventSummary> {
    let note = row.note.as_deref()?;
    match row.action.as_str() {
        "user.recovery_link.issued" => Some(RecoveryEventSummary::Issued {
            by_admin: last_note_field(note, "via")? == "web",
            at: row.at,
        }),
        "auth.password.reset_completed" => Some(RecoveryEventSummary::Completed {
            origin: sui_id_store::models::ResetTokenOrigin::parse(last_note_field(
                note, "origin",
            )?)?,
            at: row.at,
        }),
        _ => None,
    }
}
