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
//!   → pending_change::apply(...)
//!       ├─ peeks the row (read-only; does not delete)
//!       ├─ decrypts and validates the payload, seals the new password
//!       │   (all outside any transaction -- RFC 094 keeps crypto there)
//!       └─ one transaction: re-reads the row, checks the binding,
//!          deletes it only if the binding matches, writes `smtp_config`,
//!          and emits the event -- success and denial alike (S08)
//! ```
//!
//! # Binding invariants
//!
//! See RFC 090 §Security properties. All five checks (admin role,
//! session binding, actor binding, CSRF, expiry) are enforced in `apply`.
//! Role is enforced by requiring `&AdminActor` on both create and apply.
//!
//! RFC 094 M2b stage 2: `apply` is no longer generic over the pending
//! payload's type. Measured before this stage: exactly one intent exists
//! (`"smtp_password_update"`, the only `create` call site), so a concrete
//! command for that intent is the shape stage plan §2 asks for — a
//! generic command taking a mutation closure would put caller-supplied
//! code inside a Class-A transaction, exactly the boundary RFC 094 exists
//! to narrow. `create`/`cancel` stay generic/untyped-payload; neither
//! needs to interpret the payload the way `apply` now does.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sui_id_shared::ids::{PendingChangeId, SessionId};
use sui_id_store::{Database, repos::pending_settings_change};

use crate::{
    actor::AdminActor,
    errors::{CoreError, CoreResult},
    time::SharedClock,
};

// ── The one concrete payload (RFC 094 M2b stage 2) ──────────────────────────

/// The SMTP settings payload a pending change carries. Shared by `create`
/// (the handler builds one and passes it in, generically, as `T:
/// Serialize`) and `apply` (which now decrypts into this concrete type
/// directly, rather than an open `T: Deserialize`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpPasswordChangePayload {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub tls_mode: String,
    pub username: Option<String>,
    /// Plaintext. Encrypted by `create` before storage; decrypted by
    /// `apply` immediately before sealing it under `smtp_config`'s own
    /// AAD — never stored, logged, or echoed in either form.
    pub password: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub base_url: String,
}

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
#[allow(clippy::too_many_arguments)]
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
    let payload_enc =
        sui_id_store::crypto::seal(db.key(), &json, PENDING_CHANGE_AAD).map_err(CoreError::from)?;

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
    sui_id_store::commands::create_pending_settings_change(db, actor.user_id(), row)
        .await
        .map_err(CoreError::from)?;

    Ok(PendingChange {
        id,
        intent: intent.to_owned(),
        summary: summary.to_owned(),
        expires_at,
    })
}

// ── apply ─────────────────────────────────────────────────────────────────────

/// The caller-facing message for every pre-success failure on this path —
/// unknown id, expired, or a binding mismatch. RFC 090 P3/P2
/// non-disclosure: unchanged by this stage, deliberately (stage plan §3).
const EXPIRED_OR_INVALID: &str = "This pending change has expired or is no longer valid.";

/// Validate, decrypt, and apply a pending SMTP-password settings change
/// (RFC 094 M2b stage 2, S08).
///
/// Three steps, in RFC 094's forced order (crypto stays outside the
/// transaction):
///
/// 1. **Read-only.** [`pending_settings_change::peek`] fetches the row
///    without deleting it. Not a TOCTOU hole: nothing read here is
///    trusted for the eventual write — step 3 re-reads the row from
///    inside the transaction, and its own delete is what authorises the
///    write. This step exists only to get the plaintext out for
///    validation while the row is still intact, so a validation failure
///    (unknown `tls_mode`) leaves it exactly that way — the defect this
///    stage closes (stage plan §1): before this change, the row was
///    already consumed by the time validation could fail.
/// 2. **Outside the transaction.** Decrypt the payload, validate
///    `tls_mode`, read the existing `smtp_config` row (only for its
///    `created_at`), and seal the new password under `smtp_config`'s own
///    AAD.
/// 3. **One transaction.** [`sui_id_store::commands::apply_pending_settings_change`]
///    re-reads the row, checks the binding, and — only if it matches —
///    deletes the row and writes `smtp_config`, together. A binding
///    mismatch commits a denial event instead and leaves the row intact
///    (Decision B, stage plan §3): an id that is not fully secret must
///    not become a free denial of a named change.
///
/// Every failure path returns [`EXPIRED_OR_INVALID`] **except** an
/// unrecognised `tls_mode`, which gets its own specific message — the
/// method name is not a secret, and naming it saves an administrator a
/// guessing session (the same reasoning RFC 095 M3 stage 4b §7 applied to
/// a registered auth method).
pub async fn apply(
    db: &Database,
    id: PendingChangeId,
    actor: &AdminActor,
    session_id: SessionId,
    csrf_token: &str,
    clock: &SharedClock,
) -> CoreResult<()> {
    let now = clock.now();

    // Step 1: read-only peek.
    let peeked = pending_settings_change::peek(db, id, now)
        .await
        .map_err(|_| CoreError::BadRequest(EXPIRED_OR_INVALID.into()))?;

    // Step 2: decrypt, validate, seal -- outside any transaction.
    let plaintext = sui_id_store::crypto::open(db.key(), &peeked.payload_enc, PENDING_CHANGE_AAD)
        .map_err(|_| CoreError::BadRequest(EXPIRED_OR_INVALID.into()))?;
    let payload: SmtpPasswordChangePayload = serde_json::from_slice(&plaintext).map_err(|_| {
        CoreError::Internal // should not happen unless schema changed mid-flight
    })?;
    let tls_mode = sui_id_store::models::SmtpTlsMode::parse(&payload.tls_mode)
        .ok_or_else(|| CoreError::BadRequest(format!("unknown tls_mode: {}", payload.tls_mode)))?;
    let existing = sui_id_store::repos::smtp_config::get(db)
        .await
        .map_err(CoreError::from)?;
    let password_enc = Some(
        sui_id_store::repos::smtp_config::seal_password(&payload.password, db.key())
            .await
            .map_err(CoreError::from)?,
    );
    let new_smtp_config = sui_id_store::models::SmtpConfigRow {
        enabled: payload.enabled,
        host: payload.host.trim().to_owned(),
        port: payload.port,
        tls_mode,
        username: payload.username,
        password_enc,
        from_address: payload.from_address.trim().to_owned(),
        from_name: payload.from_name,
        base_url: payload.base_url.trim().trim_end_matches('/').to_owned(),
        created_at: existing.map(|r| r.created_at).unwrap_or(now),
        updated_at: now,
    };

    // Step 3: the sealed command -- one transaction, success or denial.
    let outcome = sui_id_store::commands::apply_pending_settings_change(
        db,
        actor.user_id(),
        id,
        session_id,
        csrf_token.to_owned(),
        now,
        new_smtp_config,
    )
    .await
    .map_err(CoreError::from)?
    .into_inner();

    match outcome {
        sui_id_store::commands::ApplyPendingSettingsChangeOutcome::Applied => Ok(()),
        sui_id_store::commands::ApplyPendingSettingsChangeOutcome::BindingFailed => {
            Err(CoreError::BadRequest(EXPIRED_OR_INVALID.into()))
        }
    }
}

// ── cancel ────────────────────────────────────────────────────────────────────

/// Cancel a pending change (user pressed "Cancel" on the confirm page).
///
/// Idempotent: succeeds even if the row is already absent.
///
/// RFC 094 M2b stage 1: no production caller exists for this function
/// today (measured: no `/admin/settings/*/cancel` route) -- the signature
/// still requires `&AdminActor`, which is what the S09 command's
/// `ActorRequirement::Required` is checked against. The former `clock:
/// &SharedClock` parameter is dropped: its only use was the best-effort
/// audit append's `at` field, which the sealed command below does not
/// take from the caller (every Class-A audit row's timestamp is the
/// commit-time wall clock, via `build_audit_row`, matching every other
/// converted command).
pub async fn cancel(db: &Database, id: PendingChangeId, actor: &AdminActor) -> CoreResult<()> {
    sui_id_store::commands::cancel_pending_settings_change(db, actor.user_id(), id)
        .await
        .map_err(CoreError::from)?;

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

#[cfg(test)]
#[path = "pending_change/tests.rs"]
mod tests;
