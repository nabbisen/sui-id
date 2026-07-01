//! Database connection wrapper (RFC 013: async DB layer).
//!
//! All application-layer repo calls go through the async `with_conn()` /
//! `with_tx()` methods, which hand the synchronous rusqlite work off to
//! Tokio's blocking thread pool via `tokio::task::spawn_blocking`. This
//! frees Tokio runtime workers during DB I/O — they can handle other
//! requests while a query is in progress on a blocking thread.
//!
//! The synchronous `with_conn_sync()` and `with_tx_sync()` methods are
//! kept for the migration runner (called during `open()` before the async
//! runtime is available) and for test helpers that deliberately run
//! synchronously.

use crate::crypto::MasterKey;
use crate::errors::{StoreError, StoreResult};
use crate::migrations;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

/// Shared, cloneable handle to the encrypted SQLite database.
#[derive(Clone)]
pub struct Database {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) conn: Mutex<Connection>,
    pub(crate) key: Arc<MasterKey>,
}

impl Database {
    pub fn open(path: &Path, key: MasterKey) -> StoreResult<Self> {
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::run(&mut conn)?;
        Ok(Self {
            inner: Arc::new(Inner {
                conn: Mutex::new(conn),
                key: Arc::new(key),
            }),
        })
    }

    pub fn open_in_memory(key: MasterKey) -> StoreResult<Self> {
        let mut conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::run(&mut conn)?;
        Ok(Self {
            inner: Arc::new(Inner {
                conn: Mutex::new(conn),
                key: Arc::new(key),
            }),
        })
    }

    /// Execute a synchronous closure on a Tokio blocking thread.
    /// The calling worker is freed for other requests during DB I/O.
    pub async fn with_conn<F, R>(&self, f: F) -> StoreResult<R>
    where
        F: FnOnce(&Connection) -> StoreResult<R> + Send + 'static,
        R: Send + 'static,
    {
        let db = self.clone();
        tokio::task::spawn_blocking(move || {
            // Poisoned mutex = thread panic while holding the lock; unrecoverable.
            #[allow(clippy::expect_used)]
            let guard = db.inner.conn.lock().expect("database mutex poisoned");
            f(&guard)
        })
        .await
        .map_err(|e| StoreError::JoinError(e.to_string()))?
    }

    /// Execute a synchronous closure inside a transaction on a blocking thread.
    /// Commits on `Ok`, rolls back on `Err` or panic.
    pub async fn with_tx<F, R>(&self, f: F) -> StoreResult<R>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> StoreResult<R> + Send + 'static,
        R: Send + 'static,
    {
        let db = self.clone();
        tokio::task::spawn_blocking(move || {
            #[allow(clippy::expect_used)]
            let mut guard = db.inner.conn.lock().expect("database mutex poisoned");
            let tx = guard.transaction().map_err(StoreError::from)?;
            let result = f(&tx)?;
            tx.commit().map_err(StoreError::from)?;
            Ok(result)
        })
        .await
        .map_err(|e| StoreError::JoinError(e.to_string()))?
    }

    /// Synchronous `with_conn` — use only in the migration runner and
    /// in blocking test helpers that run outside an async runtime.
    pub fn with_conn_sync<R>(
        &self,
        f: impl FnOnce(&Connection) -> StoreResult<R>,
    ) -> StoreResult<R> {
        #[allow(clippy::expect_used)]
        let guard = self.inner.conn.lock().expect("database mutex poisoned");
        f(&guard)
    }

    /// Synchronous `with_tx` — same restrictions as `with_conn_sync`.
    pub fn with_tx_sync<R>(
        &self,
        f: impl FnOnce(&rusqlite::Transaction<'_>) -> StoreResult<R>,
    ) -> StoreResult<R> {
        #[allow(clippy::expect_used)]
        let mut guard = self.inner.conn.lock().expect("database mutex poisoned");
        let tx = guard.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn key(&self) -> &MasterKey {
        &self.inner.key
    }
}
