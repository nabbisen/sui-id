//! Lightweight system-state queries that do not belong to any one entity.

use crate::db::Database;
use crate::errors::StoreResult;

const META_KEY_INITIALIZED: &str = "initialized";

/// Returns true once the initial admin has been created and setup is closed.
pub fn is_initialized(db: &Database) -> StoreResult<bool> {
    db.with_read_sync(|conn| {
        let v: Option<String> = conn
            .prepare("SELECT value FROM sui_meta WHERE key = ?1")?
            .query_row([META_KEY_INITIALIZED], |r| r.get(0))
            .ok();
        Ok(v.as_deref() == Some("true"))
    })
}

/// Mark the system as initialized. Idempotent.
pub fn mark_initialized(db: &Database) -> StoreResult<()> {
    db.with_conn_sync(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO sui_meta(key, value) VALUES(?1, 'true')",
            [META_KEY_INITIALIZED],
        )?;
        Ok(())
    })
}

/// Number of users currently present (including disabled/deleted) — used for
/// safe-fail on initialization checks.
pub fn user_count(db: &Database) -> StoreResult<i64> {
    db.with_read_sync(|conn| {
        let n: i64 = conn
            .prepare("SELECT COUNT(*) FROM users")?
            .query_row([], |r| r.get(0))?;
        Ok(n)
    })
}
