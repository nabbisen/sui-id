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
use crate::errors::{StoreError, StoreResult};
use crate::migrations;
use crate::read_conn::ReadConn;
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
    ///
    /// **A database this build does not understand is refused before anything
    /// is written to the file** (RFC 112 D3): the stored schema version is read
    /// on a read-only connection first, so the pragmas below (the journal-mode
    /// switch is a write) and the migration runner only ever touch a database
    /// that passed. Newer than this build, or unreadable, or missing from a
    /// database that has tables, is `SchemaTooNew` / `SchemaVersionInvalid`.
    pub fn open(path: &Path, key: MasterKey) -> StoreResult<Self> {
        migrations::check_database_file(path)?;
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // No busy-timeout pragma here: rusqlite sets 5 s on every connection
        // it opens (`inner_connection.rs`, `sqlite3_busy_timeout(db, 5000)`).
        // C17's `BEGIN IMMEDIATE` relies on it to wait for another process's
        // write instead of failing (S4 review).
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

    /// Like [`Self::with_tx`], except the transaction opens `BEGIN
    /// IMMEDIATE` (RFC 096-B2 S4: C17 enable's `guard_at`, RFC 096
    /// `:440`, must be read only after the write lock is actually held).
    /// `Database::class_a_immediate` is the one caller; every other
    /// command keeps calling [`Self::with_tx`]/`class_a` unchanged.
    pub async fn with_tx_immediate<F, R>(&self, f: F) -> StoreResult<R>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> StoreResult<R> + Send + 'static,
        R: Send + 'static,
    {
        let erased: crate::backend::TxFn = Box::new(move |tx| {
            let r: StoreResult<R> = f(tx);
            r.map(|v| -> Box<dyn std::any::Any + Send> { Box::new(v) })
        });

        let boxed = self.backend.with_tx_immediate_erased(erased).await?;
        #[allow(clippy::expect_used)]
        Ok(*boxed
            .downcast::<R>()
            .expect("with_tx_immediate: type mismatch — internal error"))
    }

    /// Like [`Self::with_conn`], but the closure receives a [`ReadConn`]
    /// rather than a raw `&Connection` — see RFC 096-B1 stage 0 /
    /// `read_conn.rs`'s module doc. Built on `with_conn` itself: no new
    /// dispatch path, no backend trait change, just a narrower view of the
    /// same connection `with_conn` already hands out.
    pub async fn with_read<F, R>(&self, f: F) -> StoreResult<R>
    where
        F: FnOnce(&ReadConn<'_>) -> StoreResult<R> + Send + 'static,
        R: Send + 'static,
    {
        self.with_conn(move |conn| f(&ReadConn::new(conn))).await
    }

    /// The schema version this database's `sui_meta` row actually
    /// records (RFC 112 D4) — distinct from
    /// [`migrations::MAX_SCHEMA_VERSION`], this build's ceiling. RFC 094
    /// cycle J stage 6b: a store-level query so a caller outside
    /// `sui-id-store` (the admin settings page) asking "what schema
    /// version is this database on" has one to call, rather than
    /// reaching into `migrations::read_stored_version` with a raw
    /// `with_conn` connection of its own — the deeper problem the
    /// write-authority boundary this stage adds would otherwise flag.
    pub async fn stored_schema_version(&self) -> StoreResult<i64> {
        self.with_conn(|conn| {
            migrations::read_stored_version(conn)
                .map(migrations::StoredVersion::number)
                .map_err(StoreError::from)
        })
        .await
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

    /// Synchronous `with_read` — same restrictions as `with_conn_sync`, and
    /// built on it the same way `with_read` is built on `with_conn`.
    pub fn with_read_sync<R: 'static>(
        &self,
        f: impl FnOnce(&ReadConn<'_>) -> StoreResult<R>,
    ) -> StoreResult<R> {
        self.with_conn_sync(|conn| f(&ReadConn::new(conn)))
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

    // ── Test-only fault injection (RFC 094 Stage 2 item 2) ─────────────────────
    //
    // `#[cfg(test)]`: compiles only into `sui-id-store`'s own test binary.
    // See `backend::FaultInjector`'s doc comment for why this is safe
    // against the M2a exit condition's raw-access concern.

    #[cfg(test)]
    pub(crate) fn fault_injector(&self) -> std::sync::Arc<crate::backend::FaultInjector> {
        self.sqlite_backend().fault_injector.clone()
    }

    #[cfg(test)]
    pub(crate) fn fail_next_commit_for_test(&self) {
        self.sqlite_backend().fail_next_commit_for_test();
    }

    /// RFC 096-B2 S4: the test-only seam for C17's `guard_at`. See
    /// `backend::GuardClock`'s own doc comment.
    #[cfg(any(test, feature = "test-support"))]
    pub fn guard_clock(&self) -> std::sync::Arc<crate::backend::GuardClock> {
        self.sqlite_backend().guard_clock.clone()
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
