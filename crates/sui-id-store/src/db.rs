//! Database handle (RFC 013; RFC 009 Step 1: Backend trait).
//!
//! `Database` is a cloneable, `Send + Sync` handle to the storage backend.
//! Internally it wraps `Arc<dyn Backend>` (RFC 009 Step 1), allowing future
//! steps to swap in Postgres or MariaDB without changing call sites.
//!
//! The generic `with_conn<F,R>` / `with_tx<F,R>` methods are implemented on
//! `Database` itself (not on the trait) to avoid the dyn-incompatibility of
//! generic trait methods.  They box the caller's closure, dispatch through
//! the type-erased `Backend::with_conn_erased`, and unbox the result.

use crate::backend::{Backend, SqliteBackend};
use crate::crypto::MasterKey;
use crate::errors::StoreResult;
use crate::migrations;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;

/// Shared, cloneable handle to the storage backend.
///
/// Cloning is cheap (Arc clone).  All async repo calls go through
/// `with_conn()` / `with_tx()` which dispatch to Tokio blocking threads.
#[derive(Clone)]
pub struct Database {
    backend: Arc<dyn Backend>,
}

impl Database {
    /// Open (or create) a SQLite database at `path`, run pending migrations,
    /// and return a ready handle.
    pub fn open(path: &Path, key: MasterKey) -> StoreResult<Self> {
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::run(&mut conn)?;
        Ok(Self {
            backend: Arc::new(SqliteBackend::new(conn, key)),
        })
    }

    /// Open an in-memory SQLite database (for tests).
    pub fn open_in_memory(key: MasterKey) -> StoreResult<Self> {
        let mut conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::run(&mut conn)?;
        Ok(Self {
            backend: Arc::new(SqliteBackend::new(conn, key)),
        })
    }

    // ── Async interface ───────────────────────────────────────────────────────

    /// Execute a synchronous closure on the connection, dispatched to a
    /// Tokio blocking thread.  Frees the async worker during DB I/O.
    pub async fn with_conn<F, R>(&self, f: F) -> StoreResult<R>
    where
        F: FnOnce(&Connection) -> StoreResult<R> + Send + 'static,
        R: Send + 'static,
    {
        // Wrap the typed closure so it returns `Box<dyn Any + Send>`.
        // After awaiting, downcast back to `StoreResult<R>`.
        use std::any::Any;

        let erased: crate::backend::ConnFn = Box::new(move |conn| {
            let result: Box<dyn Any + Send> = Box::new(f(conn));
            result
        });

        let boxed = self.backend.with_conn_erased(erased).await?;
        #[allow(clippy::expect_used)]
        *boxed
            .downcast::<StoreResult<R>>()
            .expect("with_conn: type mismatch — internal error")
    }

    /// Execute a synchronous closure inside a transaction, dispatched to a
    /// Tokio blocking thread.  Commits on `Ok`; rolls back on `Err`.
    pub async fn with_tx<F, R>(&self, f: F) -> StoreResult<R>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> StoreResult<R> + Send + 'static,
        R: Send + 'static,
    {
        let erased: crate::backend::TxFn = Box::new(move |tx| {
            let r: StoreResult<R> = f(tx);
            // Map Ok(r) → Ok(Box<r>), Err(e) → Err(e) so with_tx_erased
            // can use `?` to trigger rollback on error.
            r.map(|v| -> Box<dyn std::any::Any + Send> { Box::new(v) })
        });

        let boxed = self.backend.with_tx_erased(erased).await?;
        #[allow(clippy::expect_used)]
        Ok(*boxed
            .downcast::<R>()
            .expect("with_tx: type mismatch — internal error"))
    }

    // ── Synchronous interface (migration runner + blocking tests) ─────────────

    /// Synchronous `with_conn` — use only in the migration runner and in
    /// blocking test helpers outside an async runtime.
    pub fn with_conn_sync<R: 'static>(
        &self,
        f: impl FnOnce(&Connection) -> StoreResult<R>,
    ) -> StoreResult<R> {
        self.sqlite_backend().with_conn_sync(f)
    }

    /// Synchronous `with_tx` — same restrictions as `with_conn_sync`.
    pub fn with_tx_sync<R: 'static>(
        &self,
        f: impl FnOnce(&rusqlite::Transaction<'_>) -> StoreResult<R>,
    ) -> StoreResult<R> {
        self.sqlite_backend().with_tx_sync(f)
    }

    // ── Key / driver access ───────────────────────────────────────────────────

    /// The master encryption key for this database.
    pub fn key(&self) -> &MasterKey {
        self.backend.key()
    }

    /// Driver name: `"sqlite"` in Step 1.  Stored in `sui_meta` for P6.
    pub fn driver_name(&self) -> &'static str {
        self.backend.driver_name()
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    /// Downcast to `SqliteBackend` for the sync interface.
    ///
    /// Step 1 only has one backend, so this can never fail in practice.
    /// Steps 2-3 will eliminate this method by adding sync-compatible
    /// alternatives or removing the sync interface from `Database`.
    fn sqlite_backend(&self) -> &SqliteBackend {
        #[allow(clippy::expect_used)]
        self.backend
            .as_any()
            .downcast_ref::<SqliteBackend>()
            .expect("with_conn_sync / with_tx_sync require SqliteBackend (Step 1 only)")
    }
}
