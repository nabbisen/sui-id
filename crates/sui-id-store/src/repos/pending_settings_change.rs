//! Repository for `pending_settings_change` rows (RFC 090).
//!
//! A pending settings change is a short-lived, encrypted, session-bound
//! record that holds a high-risk settings payload (e.g. a new SMTP
//! password) until the admin confirms it on a second form.  The confirm
//! page receives only the opaque `id`; the actual payload never appears
//! in a form field after the initial entry.
//!
//! # Security invariants
//!
//! - **P1 (no secret in transit):** `payload_enc` is AES-GCM ciphertext;
//!   the plaintext is never stored or logged.
//! - **P2 (session binding):** `session_id` and `actor_id` are verified
//!   at apply time; a token stolen from a different session cannot apply.
//! - **P3 (single-use):** `consume` deletes the row atomically; a second
//!   call on the same id returns `NotFound`.
//! - **P4 (expiry):** `consume` checks `expires_at`; expired rows surface
//!   as `NotFound` and are also cleared by `purge_expired`.
//! - **P6 (non-secret log):** the `summary` column is non-secret copy;
//!   callers must never put secret values there.

use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::{PendingChangeId, SessionId, UserId};

use crate::{Database, StoreError, StoreResult};

// ── Row type ─────────────────────────────────────────────────────────────────

/// A stored pending-settings-change record as returned by `get` or `consume`.
#[derive(Debug, Clone)]
pub struct PendingSettingsChangeRow {
    pub id: PendingChangeId,
    pub session_id: SessionId,
    pub actor_id: UserId,
    pub intent: String,
    /// AES-GCM ciphertext of the JSON payload.  Decrypt with the caller's
    /// master key before deserialising.
    pub payload_enc: Vec<u8>,
    /// Non-secret human-readable description for the confirm page.
    pub summary: String,
    /// CSRF token the confirm POST must echo.
    pub csrf_token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<PendingSettingsChangeRow> {
    Ok(PendingSettingsChangeRow {
        id: row.get::<_, String>(0)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        session_id: row.get::<_, String>(1)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        actor_id: row.get::<_, String>(2)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        intent: row.get(3)?,
        payload_enc: row.get(4)?,
        summary: row.get(5)?,
        csrf_token: row.get(6)?,
        expires_at: row.get(7)?,
        created_at: row.get(8)?,
    })
}

// ── Insert ────────────────────────────────────────────────────────────────────

/// Store a new pending settings change.
///
/// `payload_enc` must be AES-GCM ciphertext produced by the caller (typically
/// `crypto::seal`); the plaintext is never stored.  `summary` must contain
/// only non-secret human-readable text suitable for the confirm page and audit
/// log.
pub async fn insert(db: &Database, row: &PendingSettingsChangeRow) -> StoreResult<()> {
    let id = row.id.to_string();
    let sid = row.session_id.to_string();
    let aid = row.actor_id.to_string();
    let intent = row.intent.clone();
    let payload = row.payload_enc.clone();
    let summary = row.summary.clone();
    let csrf = row.csrf_token.clone();
    let expires = row.expires_at;
    let created = row.created_at;
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO pending_settings_change \
             (id, session_id, actor_id, intent, payload_enc, summary, csrf_token, expires_at, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, sid, aid, intent, payload, summary, csrf, expires, created],
        )?;
        Ok(())
    })
    .await
}

/// RFC 094 M2b stage 1, S07: the same insert as [`insert`], run inside a
/// caller-supplied transaction rather than opening its own connection.
/// `commands::create_pending_settings_change` is the only sealed caller;
/// [`insert`] remains for test seeding unrelated to the sealed command's
/// own audit path (confirmed by its remaining callers before this stage's
/// `commands.rs` conversion).
pub fn insert_within_tx(
    tx: &rusqlite::Transaction<'_>,
    row: &PendingSettingsChangeRow,
) -> StoreResult<()> {
    tx.execute(
        "INSERT INTO pending_settings_change \
         (id, session_id, actor_id, intent, payload_enc, summary, csrf_token, expires_at, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.id.to_string(),
            row.session_id.to_string(),
            row.actor_id.to_string(),
            row.intent,
            row.payload_enc,
            row.summary,
            row.csrf_token,
            row.expires_at,
            row.created_at,
        ],
    )?;
    Ok(())
}

// ── Get summary (non-destructive) ───────────────────────────────────────────────

/// Fetch the non-secret summary of a pending change for display on the
/// confirm page. Does NOT consume (delete) the row.
///
/// Returns `StoreError::NotFound` if the row is absent or expired.
pub async fn get_summary(
    db: &Database,
    id: PendingChangeId,
    now: DateTime<Utc>,
) -> StoreResult<String> {
    let id_str = id.to_string();
    db.with_read(move |conn| {
        let result: Result<(String, DateTime<Utc>), _> = conn
            .prepare("SELECT summary, expires_at FROM pending_settings_change WHERE id = ?1")?
            .query_row([id_str.as_str()], |r| Ok((r.get(0)?, r.get(1)?)));
        match result {
            Ok((summary, expires_at)) => {
                if expires_at <= now {
                    Err(StoreError::NotFound)
                } else {
                    Ok(summary)
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(StoreError::NotFound),
            Err(e) => Err(StoreError::from(e)),
        }
    })
    .await
}

/// RFC 094 M2b stage 2, S08 §2 step 1: fetch the **full** row (ciphertext
/// included) without deleting it. Distinct from [`get_summary`], which
/// returns only the non-secret summary string — `apply`'s caller needs
/// the ciphertext too, to decrypt and validate the payload *before* the
/// destructive transaction (RFC 094 keeps crypto outside the transaction,
/// so the read has to come first and separately).
///
/// **Not a TOCTOU hole.** Nothing read here is trusted for the eventual
/// write: [`apply_within_tx`] re-reads the row from inside the
/// transaction, and its own delete is what actually authorises the
/// write. This call exists only to get the plaintext out for validation
/// before any irreversible step runs — a stale or since-changed row read
/// here cannot smuggle anything past the re-read.
pub async fn peek(
    db: &Database,
    id: PendingChangeId,
    now: DateTime<Utc>,
) -> StoreResult<PendingSettingsChangeRow> {
    let id_str = id.to_string();
    db.with_read(move |conn| {
        let row: PendingSettingsChangeRow = conn
            .prepare(
                "SELECT id, session_id, actor_id, intent, payload_enc, summary, csrf_token, \
                 expires_at, created_at FROM pending_settings_change WHERE id = ?1",
            )?
            .query_row([id_str.as_str()], map)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })?;
        if row.expires_at <= now {
            return Err(StoreError::NotFound);
        }
        Ok(row)
    })
    .await
}

// ── Consume (atomic get + delete) ─────────────────────────────────────────────

/// Atomically fetch and delete a pending settings change (RFC 090 P3).
///
/// Returns `StoreError::NotFound` when the row is absent (already consumed,
/// never existed, or purged) or when it is expired at the time of the call.
///
/// The caller must additionally verify `session_id` and `actor_id` match the
/// requesting actor (P2) and check the CSRF token before calling `consume`.
/// This function only enforces the delete-on-read invariant; binding checks
/// belong at the domain layer.
///
/// RFC 094 M2b stage 2: that split is still accurate **for this function**,
/// which is unchanged — but no longer universal in this module. `apply`
/// (S08) no longer calls `consume`; it uses [`apply_within_tx`] below,
/// whose own binding check runs *inside* the transaction, atomic with the
/// delete, because the delete decision depends on it (a binding failure
/// must not consume the row — RFC 123 D3). `consume` remains for test
/// seeding unrelated to `apply`'s own path, the same reasoning stage 1
/// left `insert`/`cancel` in place for.
pub async fn consume(
    db: &Database,
    id: PendingChangeId,
    now: DateTime<Utc>,
) -> StoreResult<PendingSettingsChangeRow> {
    let id_str = id.to_string();
    db.with_tx(move |tx| {
        // Fetch the row; 404 if absent.
        let row: PendingSettingsChangeRow = tx
            .query_row(
                "SELECT id, session_id, actor_id, intent, payload_enc, summary, csrf_token, \
                 expires_at, created_at FROM pending_settings_change WHERE id = ?1",
                [id_str.as_str()],
                map,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })?;

        // Enforce expiry (P4) before deleting.
        // Expired rows surface as NotFound (same as absent — P3 non-disclosure).
        // Cleanup is handled by `purge_expired`; we do not attempt a side-effect
        // delete here because we are inside `with_tx` and an Err return rolls back.
        if row.expires_at <= now {
            return Err(StoreError::NotFound);
        }

        // Delete the row atomically (P3 single-use).
        tx.execute(
            "DELETE FROM pending_settings_change WHERE id = ?1",
            [id_str.as_str()],
        )?;

        Ok(row)
    })
    .await
}

// ── Cancel ────────────────────────────────────────────────────────────────────

/// Delete a pending change without applying it (user cancelled).
///
/// Returns `Ok(())` even if the row was already absent, because a missing row
/// is a correct outcome of cancel (idempotent).
pub async fn cancel(db: &Database, id: PendingChangeId) -> StoreResult<()> {
    let id_str = id.to_string();
    db.with_conn(move |conn| {
        conn.execute(
            "DELETE FROM pending_settings_change WHERE id = ?1",
            [id_str.as_str()],
        )?;
        Ok(())
    })
    .await
}

/// RFC 094 M2b stage 1, S09: the same delete as [`cancel`], run inside a
/// caller-supplied transaction. `commands::cancel_pending_settings_change`
/// is the only sealed caller; [`cancel`] remains for test seeding, same
/// reasoning as [`insert_within_tx`]'s doc comment.
pub fn cancel_within_tx(tx: &rusqlite::Transaction<'_>, id: PendingChangeId) -> StoreResult<()> {
    tx.execute(
        "DELETE FROM pending_settings_change WHERE id = ?1",
        [id.to_string()],
    )?;
    Ok(())
}

// ── Apply (re-read, conditionally consume) ──────────────────────────────────

/// The outcome of [`apply_within_tx`]'s re-read: whether the binding
/// matched. RFC 094 M2b stage 2, Decision B — a binding failure must
/// **not** consume the row (RFC 123 D3: an id that is not fully secret
/// must not become a free denial of a named change), so this cannot be a
/// plain fetch-or-delete; the caller needs to know which branch it got,
/// to build the right audit event (`Applied` vs `BindingFailed`) without
/// expressing the denial as a transaction error that would roll the
/// event back too.
pub enum ApplyAttempt {
    /// Binding matched. The row is deleted (P3); `intent` is returned so
    /// the caller's audit event can carry it.
    Applied { intent: String },
    /// The row exists and is unexpired, but `session_id`, `actor_id` or
    /// `csrf_token` did not match. **Not deleted.**
    BindingFailed { intent: String },
}

/// RFC 094 M2b stage 2, S08 §2 step 3: re-read the row from inside the
/// caller's transaction, check the binding, and delete it only if the
/// binding matches. An absent or expired row is `StoreError::NotFound`
/// (same non-disclosure shape as [`consume`]); a present-but-unbound row
/// is `Ok(ApplyAttempt::BindingFailed)`, not an error — the caller still
/// needs to commit a denial event for it, and a closure returning `Err`
/// would roll that back along with everything else.
pub fn apply_within_tx(
    tx: &rusqlite::Transaction<'_>,
    id: PendingChangeId,
    session_id: SessionId,
    actor_id: UserId,
    csrf_token: &str,
    now: DateTime<Utc>,
) -> StoreResult<ApplyAttempt> {
    let id_str = id.to_string();
    let row: PendingSettingsChangeRow = tx
        .query_row(
            "SELECT id, session_id, actor_id, intent, payload_enc, summary, csrf_token, \
             expires_at, created_at FROM pending_settings_change WHERE id = ?1",
            [id_str.as_str()],
            map,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })?;
    if row.expires_at <= now {
        return Err(StoreError::NotFound);
    }
    let binding_ok =
        row.session_id == session_id && row.actor_id == actor_id && row.csrf_token == csrf_token;
    if !binding_ok {
        return Ok(ApplyAttempt::BindingFailed { intent: row.intent });
    }
    tx.execute(
        "DELETE FROM pending_settings_change WHERE id = ?1",
        [id_str.as_str()],
    )?;
    Ok(ApplyAttempt::Applied { intent: row.intent })
}

// ── Purge expired ─────────────────────────────────────────────────────────────

/// Delete all rows whose `expires_at` is ≤ `now`.
///
/// Called at startup and periodically.  Returns the number of rows removed.
pub async fn purge_expired(db: &Database, now: DateTime<Utc>) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM pending_settings_change WHERE expires_at <= ?1",
            params![now],
        )?;
        Ok(n)
    })
    .await
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
