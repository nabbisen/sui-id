//! Short-lived "password verified, MFA pending" rows.
//!
//! Inserted right after a successful password check when the user has
//! TOTP enabled. The HTTP layer hands the user a temporary cookie
//! pointing at this row; on submission of a valid TOTP code, we delete
//! the row and create the real session.
//!
//! The row carries no authority on its own — we still need a valid
//! TOTP code to promote it.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::LoginPendingMfaRow;
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use sui_id_shared::ids::PendingMfaId;

const SELECT: &str = "SELECT id, user_id, expires_at, created_at FROM login_pending_mfa";

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<LoginPendingMfaRow> {
    let id_str: String = row.get(0)?;
    let user_id_str: String = row.get(1)?;
    Ok(LoginPendingMfaRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        user_id: user_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        expires_at: row.get::<_, DateTime<Utc>>(2)?,
        created_at: row.get::<_, DateTime<Utc>>(3)?,
    })
}

pub async fn insert(db: &Database, row: &LoginPendingMfaRow) -> StoreResult<()> {
    let row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO login_pending_mfa(id, user_id, expires_at, created_at) \
             VALUES(?1, ?2, ?3, ?4)",
            params![
                row.id.to_string(),
                row.user_id.to_string(),
                row.expires_at,
                row.created_at,
            ],
        )?;
        Ok(())
    })
    .await
}

pub async fn get(db: &Database, id: PendingMfaId) -> StoreResult<Option<LoginPendingMfaRow>> {
    db.with_conn(move |conn| {
        Ok(conn
            .query_row(&format!("{SELECT} WHERE id = ?1"), [id.to_string()], map)
            .optional()?)
    })
    .await
}

/// RFC 102 L02: consume a pending-MFA row inside the caller's transaction.
/// The row must exist, belong to `user_id` and be unexpired at `now`;
/// otherwise nothing is deleted and the result is `NotFound`. Two
/// completions of one row cannot both succeed.
pub fn consume_within_tx(
    conn: &rusqlite::Connection,
    id: PendingMfaId,
    user_id: sui_id_shared::ids::UserId,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let n = conn.execute(
        "DELETE FROM login_pending_mfa WHERE id = ?1 AND user_id = ?2 AND expires_at > ?3",
        params![id.to_string(), user_id.to_string(), now],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// RFC 102 L07: delete every pending-MFA row for a user, inside the
/// caller's transaction. Returns how many were deleted.
pub fn delete_all_for_user_within_tx(
    conn: &rusqlite::Connection,
    user_id: sui_id_shared::ids::UserId,
) -> StoreResult<usize> {
    Ok(conn.execute(
        "DELETE FROM login_pending_mfa WHERE user_id = ?1",
        [user_id.to_string()],
    )?)
}

pub async fn delete(db: &Database, id: PendingMfaId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM login_pending_mfa WHERE id = ?1",
            [id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
    .await
}

/// Hygiene: drop expired rows. Called from the GC task.
pub async fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM login_pending_mfa WHERE expires_at < ?1",
            [Utc::now()],
        )?;
        Ok(n)
    })
    .await
}
