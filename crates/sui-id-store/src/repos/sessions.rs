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
        id: row
            .get::<_, String>(0)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
        user_id: row
            .get::<_, String>(1)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?,
        expires_at: row.get::<_, DateTime<Utc>>(2)?,
        created_at: row.get::<_, DateTime<Utc>>(3)?,
        revoked_at: row.get::<_, Option<DateTime<Utc>>>(4)?,
        auth_methods,
    })
}

pub fn insert(db: &Database, s: &SessionRow) -> StoreResult<()> {
    let methods_json = serde_json::to_string(&s.auth_methods)?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO sessions(id, user_id, expires_at, created_at, revoked_at, auth_methods) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                s.id.to_string(),
                s.user_id.to_string(),
                s.expires_at,
                s.created_at,
                s.revoked_at,
                methods_json,
            ],
        )?;
        Ok(())
    })
}

pub fn get(db: &Database, id: SessionId) -> StoreResult<SessionRow> {
    db.with_conn(|conn| {
        conn.query_row(
            "SELECT id, user_id, expires_at, created_at, revoked_at, auth_methods \
             FROM sessions WHERE id = ?1",
            [id.to_string()],
            map,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
}

pub fn revoke(db: &Database, id: SessionId) -> StoreResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE sessions SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), id.to_string()],
        )?;
        Ok(())
    })
}

pub fn revoke_all_for_user(db: &Database, user_id: UserId) -> StoreResult<usize> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL",
            params![Utc::now(), user_id.to_string()],
        )?;
        Ok(n)
    })
}

/// Delete sessions that are past their expiry. Hygiene only — expired
/// sessions are already filtered out at lookup time.
pub fn purge_expired(db: &Database) -> StoreResult<usize> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "DELETE FROM sessions WHERE expires_at < ?1",
            [Utc::now()],
        )?;
        Ok(n)
    })
}
