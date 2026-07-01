//! Server-side admin session store.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::SessionRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::{SessionId, UserId};

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRow> {
    let auth_methods_json: String = row.get(5)?;
    let auth_methods = serde_json::from_str(&auth_methods_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(SessionRow {
        id: row.get::<_, String>(0)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        user_id: row.get::<_, String>(1)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        expires_at: row.get::<_, DateTime<Utc>>(2)?,
        created_at: row.get::<_, DateTime<Utc>>(3)?,
        revoked_at: row.get::<_, Option<DateTime<Utc>>>(4)?,
        auth_methods,
        last_step_up_at: row.get::<_, Option<DateTime<Utc>>>(6)?,
        last_used_at: row.get::<_, Option<DateTime<Utc>>>(7)?,
    })
}

const SELECT_COLS: &str =
    "id, user_id, expires_at, created_at, revoked_at, auth_methods, last_step_up_at, last_used_at";

pub async fn insert(db: &Database, s: &SessionRow) -> StoreResult<()> {
    let methods_json = serde_json::to_string(&s.auth_methods)?;
    let s = s.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO sessions(id, user_id, expires_at, created_at, revoked_at, \
             auth_methods, last_step_up_at, last_used_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                s.id.to_string(),
                s.user_id.to_string(),
                s.expires_at,
                s.created_at,
                s.revoked_at,
                methods_json,
                s.last_step_up_at,
                s.last_used_at,
            ],
        )?;
        Ok(())
    })
    .await
}

pub async fn get(db: &Database, id: SessionId) -> StoreResult<SessionRow> {
    db.with_conn(move |conn| {
        conn.query_row(
            &format!("SELECT {SELECT_COLS} FROM sessions WHERE id = ?1"),
            [id.to_string()],
            map,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
    .await
}

pub async fn revoke(db: &Database, id: SessionId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE sessions SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), id.to_string()],
        )?;
        Ok(())
    })
    .await
}

pub async fn revoke_all_for_user(db: &Database, user_id: UserId) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), user_id.to_string()],
        )?;
        Ok(n)
    })
    .await
}

/// Same as [`revoke_all_for_user`] but runs inside a caller-owned
/// transaction, so it participates in the caller's atomicity boundary.
pub fn revoke_all_for_user_within_tx(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<usize> {
    let n = tx.execute(
        "UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL",
        params![now, user_id.to_string()],
    )?;
    Ok(n)
}

/// Delete sessions that are past their expiry. Hygiene only — expired
/// sessions are already filtered out at lookup time.
pub async fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute("DELETE FROM sessions WHERE expires_at < ?1", [Utc::now()])?;
        Ok(n)
    })
    .await
}

/// List every currently-active session belonging to a given user, newest first.
pub async fn list_active_for_user(db: &Database, user_id: UserId) -> StoreResult<Vec<SessionRow>> {
    let now = Utc::now();
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLS} FROM sessions \
                      WHERE user_id = ?1 AND revoked_at IS NULL AND expires_at > ?2 \
                      ORDER BY created_at DESC"
        ))?;
        let rows = stmt
            .query_map(params![user_id.to_string(), now], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// Revoke every active session for the user *except* the supplied id.
pub async fn revoke_all_for_user_except(
    db: &Database,
    user_id: UserId,
    keep: SessionId,
) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE sessions SET revoked_at = ?1 \
             WHERE user_id = ?2 AND id != ?3 AND revoked_at IS NULL",
            params![Utc::now(), user_id.to_string(), keep.to_string()],
        )?;
        Ok(n)
    })
    .await
}

/// Update a session's `last_step_up_at` timestamp to `at`. Used after a
/// successful step-up challenge to mark the session as freshly
/// MFA-elevated. Idempotent: writing the same value twice is harmless.
///
/// We do not gate this on `revoked_at IS NULL` — the caller has already
/// resolved the session through `session::resolve` (which does that
/// check) and a race where the session is revoked between resolve and
/// touch is benign: a revoked session can't be used for anything
/// regardless of step-up state.
pub async fn touch_step_up(db: &Database, id: SessionId, at: DateTime<Utc>) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE sessions SET last_step_up_at = ?1 WHERE id = ?2",
            params![at, id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Update `last_used_at` to `at`. Called by the application layer
/// from authenticated request handlers, throttled so a busy session
/// produces at most one UPDATE per minute (see
/// `core::session::touch_last_used`). A revoked session being
/// touched is benign.
pub async fn touch_last_used(db: &Database, id: SessionId, at: DateTime<Utc>) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE sessions SET last_used_at = ?1 WHERE id = ?2",
            params![at, id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Count the active (un-expired, un-revoked) sessions for a user
/// at the given moment. Used by the concurrent-session-cap check
/// at login time.
pub async fn count_active_for_user(
    db: &Database,
    user_id: UserId,
    now: DateTime<Utc>,
) -> StoreResult<i64> {
    db.with_conn(move |conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions \
             WHERE user_id = ?1 AND revoked_at IS NULL AND expires_at > ?2",
            params![user_id.to_string(), now],
            |row| row.get(0),
        )?;
        Ok(n)
    })
    .await
}

/// Return up to `limit` of the oldest active sessions for a user,
/// ordered by `created_at` ascending. Drives the FIFO eviction
/// path: at login time, when the post-insert active count would
/// exceed the cap by `k`, the application revokes the `k` oldest
/// rows returned here.
pub async fn oldest_active_for_user(
    db: &Database,
    user_id: UserId,
    now: DateTime<Utc>,
    limit: i64,
) -> StoreResult<Vec<SessionRow>> {
    db.with_conn(move |conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLS} FROM sessions \
             WHERE user_id = ?1 AND revoked_at IS NULL AND expires_at > ?2 \
             ORDER BY created_at ASC LIMIT ?3"
        ))?;
        let rows = stmt
            .query_map(params![user_id.to_string(), now, limit], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// Count all non-revoked, non-expired sessions across all users.
/// Used by the admin dashboard to display the active-session stat card.
pub async fn count_active_total(db: &Database) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions              WHERE revoked = 0 AND expires_at > unixepoch('now')",
            [],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    }).await
}
