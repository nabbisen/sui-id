//! User CRUD.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::UserRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::UserId;

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserRow> {
    Ok(UserRow {
        id: row
            .get::<_, String>(0)?
            .parse()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        is_admin: row.get::<_, i64>(3)? != 0,
        is_disabled: row.get::<_, i64>(4)? != 0,
        is_deleted: row.get::<_, i64>(5)? != 0,
        created_at: row.get::<_, DateTime<Utc>>(6)?,
        updated_at: row.get::<_, DateTime<Utc>>(7)?,
    })
}

const SELECT_USER: &str =
    "SELECT id, username, display_name, is_admin, is_disabled, is_deleted, created_at, updated_at FROM users";

pub fn create(db: &Database, user: &UserRow) -> StoreResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO users(id, username, display_name, is_admin, is_disabled, is_deleted, created_at, updated_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                user.id.to_string(),
                user.username,
                user.display_name,
                user.is_admin as i64,
                user.is_disabled as i64,
                user.is_deleted as i64,
                user.created_at,
                user.updated_at,
            ],
        )
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                StoreError::Conflict
            }
            other => StoreError::from(other),
        })?;
        Ok(())
    })
}

pub fn get(db: &Database, id: UserId) -> StoreResult<UserRow> {
    db.with_conn(|conn| {
        conn.query_row(
            &format!("{SELECT_USER} WHERE id = ?1"),
            [id.to_string()],
            map_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
}

pub fn find_by_username(db: &Database, username: &str) -> StoreResult<UserRow> {
    db.with_conn(|conn| {
        conn.query_row(
            &format!("{SELECT_USER} WHERE username = ?1"),
            [username],
            map_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
}

pub fn list(db: &Database) -> StoreResult<Vec<UserRow>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT_USER} ORDER BY created_at ASC"))?;
        let rows = stmt
            .query_map([], map_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

/// Toggle the `is_disabled` flag (suspend / un-suspend).
pub fn set_disabled(db: &Database, id: UserId, disabled: bool) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE users SET is_disabled = ?1, updated_at = ?2 WHERE id = ?3",
            params![disabled as i64, Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
}

/// Soft-delete a user. Hard delete is intentionally not exposed at this
/// layer: it would orphan audit-log references.
pub fn soft_delete(db: &Database, id: UserId) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE users SET is_deleted = 1, is_disabled = 1, updated_at = ?1 WHERE id = ?2",
            params![Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
}
