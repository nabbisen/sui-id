//! Database connection wrapper.
//!
//! For sui-id's expected workload (a small self-hosted IDaaS) a single
//! connection guarded by an async-aware mutex is plenty. This keeps the
//! configuration surface tiny and matches the project's "minimum viable
//! moving parts" philosophy.

use crate::crypto::MasterKey;
use crate::errors::StoreResult;
use crate::migrations;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

/// Shared, cloneable handle to the encrypted SQLite database.
#[derive(Clone)]
pub struct Database {
    inner: Arc<Inner>,
}

struct Inner {
    conn: Mutex<Connection>,
    key: Arc<MasterKey>,
}

impl Database {
    /// Open or create the SQLite database at `path`, run pending migrations,
    /// and bind the master key for column encryption.
    pub fn open(path: &Path, key: MasterKey) -> StoreResult<Self> {
        let conn = Connection::open(path)?;
        // Reasonable defaults for a single-process, self-hosted service.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::run(&conn)?;
        Ok(Self {
            inner: Arc::new(Inner {
                conn: Mutex::new(conn),
                key: Arc::new(key),
            }),
        })
    }

    /// Open an in-memory database (used by tests).
    pub fn open_in_memory(key: MasterKey) -> StoreResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::run(&conn)?;
        Ok(Self {
            inner: Arc::new(Inner {
                conn: Mutex::new(conn),
                key: Arc::new(key),
            }),
        })
    }

    /// Run a closure against an exclusive borrow of the underlying connection.
    pub fn with_conn<R>(&self, f: impl FnOnce(&Connection) -> StoreResult<R>) -> StoreResult<R> {
        let guard = self
            .inner
            .conn
            .lock()
            .expect("database mutex was poisoned by a previous panic");
        f(&guard)
    }

    /// Reference to the master encryption key.
    pub fn key(&self) -> &MasterKey {
        &self.inner.key
    }
}
