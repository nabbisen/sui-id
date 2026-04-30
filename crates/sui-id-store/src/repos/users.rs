//! User CRUD.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::UserRow;
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::UserId;

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserRow> {
    let user_uuid_str: String = row.get(8)?;
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
        user_uuid: uuid::Uuid::parse_str(&user_uuid_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
        })?,
        created_at: row.get::<_, DateTime<Utc>>(6)?,
        updated_at: row.get::<_, DateTime<Utc>>(7)?,
        failed_login_count: row.get::<_, i64>(9)?,
        locked_until: row.get::<_, Option<DateTime<Utc>>>(10)?,
    })
}

const SELECT_USER: &str = "SELECT id, username, display_name, is_admin, is_disabled, \
                           is_deleted, created_at, updated_at, user_uuid, \
                           failed_login_count, locked_until FROM users";

pub fn create(db: &Database, user: &UserRow) -> StoreResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO users(id, username, display_name, is_admin, is_disabled, is_deleted, \
                                created_at, updated_at, user_uuid, \
                                failed_login_count, locked_until) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                user.id.to_string(),
                user.username,
                user.display_name,
                user.is_admin as i64,
                user.is_disabled as i64,
                user.is_deleted as i64,
                user.created_at,
                user.updated_at,
                user.user_uuid.to_string(),
                user.failed_login_count,
                user.locked_until,
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

/// Increment the user's consecutive-failure counter and (when the
/// caller decides the lock window applies) stamp `locked_until`.
/// Returns the new failure count.
///
/// `lock_until` is the wall-clock time before which the account is
/// refused. `None` means "increment the counter but don't lock yet"
/// — used at low failure counts where we want to count but not yet
/// punish. The decision is intentionally outside this function so
/// that the `sui_id_core` layer can choose the backoff curve.
pub fn record_login_failure(
    db: &Database,
    id: UserId,
    lock_until: Option<DateTime<Utc>>,
) -> StoreResult<i64> {
    db.with_conn(|conn| {
        let tx = conn.unchecked_transaction()?;
        let count: i64 = tx
            .query_row(
                "SELECT failed_login_count FROM users WHERE id = ?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                other => StoreError::from(other),
            })?;
        let new_count = count + 1;
        tx.execute(
            "UPDATE users SET failed_login_count = ?1, locked_until = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_count, lock_until, Utc::now(), id.to_string()],
        )?;
        tx.commit()?;
        Ok(new_count)
    })
}

/// Reset the user's failure counter and clear any active lock.
/// Called on a successful password verification.
pub fn clear_lockout(db: &Database, id: UserId) -> StoreResult<()> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE users SET failed_login_count = 0, locked_until = NULL, updated_at = ?1 WHERE id = ?2",
            params![Utc::now(), id.to_string()],
        )?;
        Ok(())
    })
}

/// Admin-initiated unlock: reset both fields without requiring a
/// successful password check. Used by `sui-id admin unlock-user`.
pub fn admin_unlock(db: &Database, id: UserId) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE users SET failed_login_count = 0, locked_until = NULL, updated_at = ?1 WHERE id = ?2",
            params![Utc::now(), id.to_string()],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    })
}
