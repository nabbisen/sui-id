//! Pending settings change — domain layer (RFC 090).
//!
//! Wraps the `sui-id-store` repository with encryption, binding checks,
//! and audit emission so that high-risk settings changes that include
//! secrets (e.g. SMTP password) never appear in hidden form fields.
//!
//! # Flow
//!
//! ```text
//! POST /admin/settings/email  (includes new password)
//!   → pending_change::create(...)    encrypts payload, inserts row, returns id
//!   → redirect to /admin/settings/email/confirm?pending_change_id={id}
//!
//! GET  /admin/settings/email/confirm
//!   → renders non-secret summary from PendingChange.summary
//!
//! POST /admin/settings/email/confirm  (submits pending_change_id + CSRF)
//!   → pending_change::apply::<SmtpPayload>(...)
//!       ├─ consumes (deletes) the row
//!       ├─ verifies session, actor, CSRF, expiry bindings
//!       ├─ decrypts payload
//!       └─ returns the typed payload for the caller to apply
//! ```
//!
//! # Binding invariants
//!
//! See RFC 090 §Security properties. All five checks (admin role,
//! session binding, actor binding, CSRF, expiry) are enforced in `apply`.
//! Role is enforced by requiring `&AdminActor` on both create and apply.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sui_id_shared::ids::{PendingChangeId, SessionId, UserId};
use sui_id_store::{
    models::AuditLogRow,
    repos::{audit, pending_settings_change},
    Database,
};

use crate::{
    actor::AdminActor,
    errors::{CoreError, CoreResult},
    time::SharedClock,
};

// ── TTL ──────────────────────────────────────────────────────────────────────

/// Lifetime of a pending settings change: 5 minutes, matching the step-up
/// freshness window (Appendix E).
pub const PENDING_CHANGE_TTL_SECS: i64 = 300;

// ── AAD label for AES-GCM ────────────────────────────────────────────────────

/// Additional-authenticated-data string for `seal`/`open`.
/// Ties the ciphertext to this specific use; prevents re-use of other
/// encrypted blobs stored in the database.
const PENDING_CHANGE_AAD: &[u8] = b"sui-id:pending_settings_change:v1";

// ── Public return type ────────────────────────────────────────────────────────

/// The public view of a pending settings change: no ciphertext, no secrets.
/// Returned by `create` and used to build the confirm page.
#[derive(Debug, Clone)]
pub struct PendingChange {
    pub id: PendingChangeId,
    pub intent: String,
    /// Non-secret human-readable description.  Safe to display on the
    /// confirm page and to include in audit log notes.
    pub summary: String,
    pub expires_at: DateTime<Utc>,
}

// ── create ────────────────────────────────────────────────────────────────────

/// Encrypt `payload` and store a pending settings change.
///
/// The payload must be `Serialize`; it is serialised to JSON and encrypted
/// with AES-GCM under the master key before storage.  The `summary` must
/// contain only non-secret text (it appears on the confirm page and in the
/// audit log).
///
/// Returns the public `PendingChange` (no ciphertext, no secrets).
pub async fn create<T: Serialize>(
    db: &Database,
    actor: &AdminActor,
    session_id: SessionId,
    intent: &str,
    payload: &T,
    summary: &str,
    csrf_token: &str,
    clock: &SharedClock,
) -> CoreResult<PendingChange> {
    let now = clock.now();
    let expires_at = now + chrono::Duration::seconds(PENDING_CHANGE_TTL_SECS);
    let id = PendingChangeId::new();

    // Serialise and encrypt the payload.
    let json = serde_json::to_vec(payload).map_err(|_| {
        CoreError::Internal // serialisation failure should never happen
    })?;
    let payload_enc = sui_id_store::crypto::seal(db.key(), &json, PENDING_CHANGE_AAD)
        .map_err(CoreError::from)?;

    let row = pending_settings_change::PendingSettingsChangeRow {
        id,
        session_id,
        actor_id: actor.user_id(),
        intent: intent.to_owned(),
        payload_enc,
        summary: summary.to_owned(),
        csrf_token: csrf_token.to_owned(),
        expires_at,
        created_at: now,
    };
    pending_settings_change::insert(db, &row)
        .await
        .map_err(CoreError::from)?;

    // Audit: creation (non-secret).
    let _ = audit::append(
        db,
        &AuditLogRow {
            at: now,
            actor: Some(actor.user_id()),
            action: "settings.pending_change.created".into(),
            target: None,
            result: "ok".into(),
            note: Some(format!("intent={intent} id={id} summary={summary}")),
        },
    )
    .await;

    Ok(PendingChange {
        id,
        intent: intent.to_owned(),
        summary: summary.to_owned(),
        expires_at,
    })
}

// ── apply ─────────────────────────────────────────────────────────────────────

/// Consume, validate, decrypt, and return the pending settings change payload.
///
/// Enforces all five binding invariants:
/// - **Role**: `actor` is `&AdminActor` (compile-time proof).
/// - **Session**: `session_id` must match the creating session.
/// - **Actor**: `actor.user_id()` must match the creating actor.
/// - **CSRF**: `csrf_token` must match the stored token.
/// - **Expiry**: enforced by `pending_settings_change::consume`.
///
/// On any failure (including binding mismatch, expiry, or unknown id), returns
/// `CoreError::BadRequest` with a neutral "expired or invalid" message so
/// callers cannot distinguish which check failed.
///
/// On success, the row is deleted from the database (single-use, P3).
pub async fn apply<T: for<'de> Deserialize<'de>>(
    db: &Database,
    id: PendingChangeId,
    actor: &AdminActor,
    session_id: SessionId,
    csrf_token: &str,
    clock: &SharedClock,
) -> CoreResult<T> {
    let now = clock.now();

    // Consume the row (deletes it atomically; returns NotFound if absent or
    // expired — both are treated as the same neutral error).
    let row = pending_settings_change::consume(db, id, now)
        .await
        .map_err(|_| {
            CoreError::BadRequest("This pending change has expired or is no longer valid.".into())
        })?;

    // Verify binding invariants (P2). All failures use the same neutral error
    // message so the caller cannot distinguish which check failed.
    let binding_ok = row.session_id == session_id
        && row.actor_id == actor.user_id()
        && row.csrf_token == csrf_token;
    if !binding_ok {
        // Audit: binding failure (non-secret — note does not reveal which
        // check failed, only that one did).
        let _ = audit::append(
            db,
            &AuditLogRow {
                at: now,
                actor: Some(actor.user_id()),
                action: "settings.pending_change.binding_failed".into(),
                target: None,
                result: "denied".into(),
                note: Some(format!("intent={} id={id}", row.intent)),
            },
        )
        .await;
        return Err(CoreError::BadRequest(
            "This pending change has expired or is no longer valid.".into(),
        ));
    }

    // Decrypt the payload.
    let plaintext =
        sui_id_store::crypto::open(db.key(), &row.payload_enc, PENDING_CHANGE_AAD)
            .map_err(|_| {
                CoreError::BadRequest(
                    "This pending change has expired or is no longer valid.".into(),
                )
            })?;
    let payload: T = serde_json::from_slice(&plaintext).map_err(|_| {
        CoreError::Internal // should not happen unless schema changed mid-flight
    })?;

    // Audit: applied (non-secret summary only).
    let _ = audit::append(
        db,
        &AuditLogRow {
            at: now,
            actor: Some(actor.user_id()),
            action: "settings.pending_change.applied".into(),
            target: None,
            result: "ok".into(),
            note: Some(format!("intent={} summary={}", row.intent, row.summary)),
        },
    )
    .await;

    Ok(payload)
}

// ── cancel ────────────────────────────────────────────────────────────────────

/// Cancel a pending change (user pressed "Cancel" on the confirm page).
///
/// Idempotent: succeeds even if the row is already absent.
pub async fn cancel(
    db: &Database,
    id: PendingChangeId,
    actor: &AdminActor,
    clock: &SharedClock,
) -> CoreResult<()> {
    pending_settings_change::cancel(db, id)
        .await
        .map_err(CoreError::from)?;

    let _ = audit::append(
        db,
        &AuditLogRow {
            at: clock.now(),
            actor: Some(actor.user_id()),
            action: "settings.pending_change.cancelled".into(),
            target: None,
            result: "ok".into(),
            note: Some(format!("id={id}")),
        },
    )
    .await;

    Ok(())
}

// ── purge_expired ─────────────────────────────────────────────────────────────

/// Delete all pending changes whose `expires_at` is ≤ `now`.
///
/// Called at startup. Returns the number of rows removed.
pub async fn purge_expired(db: &Database, clock: &SharedClock) -> CoreResult<usize> {
    pending_settings_change::purge_expired(db, clock.now())
        .await
        .map_err(CoreError::from)
}
