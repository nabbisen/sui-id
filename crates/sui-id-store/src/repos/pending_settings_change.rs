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
    db.with_conn(move |conn| {
        let result: Result<(String, DateTime<Utc>), _> = conn.query_row(
            "SELECT summary, expires_at FROM pending_settings_change WHERE id = ?1",
            [id_str.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        );
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
