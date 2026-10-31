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
    db.with_conn_sync(mark_initialized_within_tx)
}

/// Same check as [`is_initialized`], for a caller that already holds a
/// transaction (RFC 094 cycle I stage 4, S10: the sealed Class-A
/// capability). Takes `&rusqlite::Connection` rather than `&Transaction`
/// so it accepts either a bare connection or a `WriteTx`'s transaction —
/// `rusqlite::Transaction` derefs to `Connection`. The durable, inside-the-
/// transaction answer, not the outer wrappers' cheap early check: this is
/// what closes the race two concurrent first-run submissions could
/// otherwise both pass.
pub fn is_initialized_within_tx(conn: &rusqlite::Connection) -> StoreResult<bool> {
    let v: Option<String> = conn
        .prepare("SELECT value FROM sui_meta WHERE key = ?1")?
        .query_row([META_KEY_INITIALIZED], |r| r.get(0))
        .ok();
    Ok(v.as_deref() == Some("true"))
}

/// Same write as [`mark_initialized`], run inside a caller-supplied
/// transaction. `commands::complete_initial_setup` (S10) is the only
/// sealed caller.
pub fn mark_initialized_within_tx(conn: &rusqlite::Connection) -> StoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sui_meta(key, value) VALUES(?1, 'true')",
        [META_KEY_INITIALIZED],
    )?;
    Ok(())
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
