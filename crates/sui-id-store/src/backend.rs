//! Storage backend abstraction (RFC 009, Step 1).
//!
//! Introduces the `Backend` trait so that the storage layer can be
//! progressively extended to support PostgreSQL and MariaDB without
//! changing any call site above the repo layer.
//!
//! # Dyn-safety and generic erasure
//!
//! Rust traits with generic methods are not dyn-compatible.  The public repo
//! API uses `Database::with_conn<F,R>` with generic `F` and `R`.  To keep
//! those generic call sites and still store `Arc<dyn Backend>`, `Database`
//! itself is a thin wrapper that provides the generic helpers by calling the
//! type-erased `Backend::with_conn_erased` / `with_tx_erased` methods.
//!
//! The type-erased methods take a `Box<dyn FnOnce(&Connection) -> Box<dyn Any + Send>>`
//! which avoids any generic parameter on the trait.  `Database` boxes and
//! unboxes automatically — all repos see the original `with_conn<F,R>` API.
//!
//! # Step 1 scope
//!
//! One implementation: [`SqliteBackend`].  No behaviour change; this is a
//! pure architectural refactor preparing the seam for Steps 2-3.

use crate::{
    crypto::MasterKey,
    errors::{StoreError, StoreResult},
};
use rusqlite::Connection;
use std::{
    any::Any,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

// ── Type aliases for the erased closure types ─────────────────────────────────

pub(crate) type ConnFn = Box<dyn FnOnce(&Connection) -> Box<dyn Any + Send> + Send>;
pub(crate) type TxFn = Box<
    dyn for<'tx> FnOnce(
            &rusqlite::Transaction<'tx>,
        ) -> crate::errors::StoreResult<Box<dyn Any + Send>>
        + Send,
>;
pub(crate) type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// ── Backend trait (dyn-compatible) ────────────────────────────────────────────

/// Async storage backend — object-safe via type erasure.
///
/// Call sites never use these methods directly; they go through
/// `Database::with_conn` and `Database::with_tx` which re-introduce generics
/// on top of the erased interface.
pub trait Backend: Send + Sync {
    /// Run an erased closure on the connection asynchronously.
    /// The `Box<dyn Any + Send>` return is unwrapped by `Database::with_conn`.
    fn with_conn_erased<'a>(&'a self, f: ConnFn)
    -> BoxFuture<'a, StoreResult<Box<dyn Any + Send>>>;

    /// Run an erased closure inside a transaction asynchronously.
    fn with_tx_erased<'a>(&'a self, f: TxFn) -> BoxFuture<'a, StoreResult<Box<dyn Any + Send>>>;

    /// The master encryption key.
    fn key(&self) -> &MasterKey;

    /// Short driver name (`"sqlite"`, `"postgres"`, `"mariadb"`).
    /// Stored in `sui_meta` for RFC 009 P6 startup match check.
    fn driver_name(&self) -> &'static str;

    /// Downcast helper for the sync interface (SQLite-only in Step 1).
    fn as_any(&self) -> &dyn Any;
}

// ── SqliteBackend ─────────────────────────────────────────────────────────────

/// The SQLite backend (Step 1 — the only backend).
///
/// Stores `Arc<Mutex<Connection>>` so that clones are cheap and `'static`,
/// suitable for moving into `spawn_blocking` closures.
#[derive(Clone)]
pub struct SqliteBackend {
    conn: Arc<Mutex<Connection>>,
    key: Arc<MasterKey>,
}

impl SqliteBackend {
    pub fn new(conn: Connection, key: MasterKey) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            key: Arc::new(key),
        }
    }

    pub fn with_conn_sync<R: 'static>(
        &self,
        f: impl FnOnce(&Connection) -> StoreResult<R>,
    ) -> StoreResult<R> {
        #[allow(clippy::expect_used)]
        let guard = self.conn.lock().expect("database mutex poisoned");
        f(&guard)
    }

    pub fn with_tx_sync<R: 'static>(
        &self,
        f: impl FnOnce(&rusqlite::Transaction<'_>) -> StoreResult<R>,
    ) -> StoreResult<R> {
        #[allow(clippy::expect_used)]
        let mut guard = self.conn.lock().expect("database mutex poisoned");
        let tx = guard.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}

impl Backend for SqliteBackend {
    fn with_conn_erased<'a>(
        &'a self,
        f: ConnFn,
    ) -> BoxFuture<'a, StoreResult<Box<dyn Any + Send>>> {
        let conn = self.conn.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                #[allow(clippy::expect_used)]
                let guard = conn.lock().expect("database mutex poisoned");
                Ok(f(&guard))
            })
            .await
            .map_err(|e| StoreError::JoinError(e.to_string()))?
        })
    }

    fn with_tx_erased<'a>(&'a self, f: TxFn) -> BoxFuture<'a, StoreResult<Box<dyn Any + Send>>> {
        let conn = self.conn.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                #[allow(clippy::expect_used)]
                let mut guard = conn.lock().expect("database mutex poisoned");
                let tx = guard.transaction().map_err(StoreError::from)?;
                let result = f(&tx)?; // propagates Err → rollback (tx dropped)
                tx.commit().map_err(StoreError::from)?;
                Ok(result)
            })
            .await
            .map_err(|e| StoreError::JoinError(e.to_string()))?
        })
    }

    fn key(&self) -> &MasterKey {
        &self.key
    }

    fn driver_name(&self) -> &'static str {
        "sqlite"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::{Database, crypto::MasterKey};

    fn open_test_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).unwrap()
    }

    #[test]
    fn driver_name_is_sqlite() {
        let db = open_test_db();
        assert_eq!(db.driver_name(), "sqlite");
    }

    #[tokio::test]
    async fn with_conn_executes_on_blocking_thread() {
        let db = open_test_db();
        let result = db
            .with_conn(|conn| {
                conn.query_row("SELECT 1 + 1", [], |row| row.get::<_, i64>(0))
                    .map_err(crate::errors::StoreError::from)
            })
            .await
            .unwrap();
        assert_eq!(result, 2);
    }

    #[tokio::test]
    async fn with_tx_commits() {
        let db = open_test_db();
        db.with_tx(|tx| {
            tx.execute_batch("CREATE TABLE IF NOT EXISTS _test_backend (x INTEGER);")?;
            tx.execute("INSERT INTO _test_backend VALUES (42)", [])?;
            Ok(())
        })
        .await
        .unwrap();

        let val: i64 = db
            .with_conn(|conn| {
                conn.query_row("SELECT x FROM _test_backend", [], |r| r.get(0))
                    .map_err(crate::errors::StoreError::from)
            })
            .await
            .unwrap();
        assert_eq!(val, 42);
    }

    #[tokio::test]
    async fn with_tx_rolls_back_on_error() {
        let db = open_test_db();
        db.with_tx(|tx| {
            tx.execute_batch("CREATE TABLE IF NOT EXISTS _rb (x INTEGER);")?;
            Ok(())
        })
        .await
        .unwrap();

        let _: crate::errors::StoreResult<()> = db
            .with_tx(|tx| {
                tx.execute("INSERT INTO _rb VALUES (99)", [])?;
                Err(crate::errors::StoreError::InvalidData("rollback".into()))
            })
            .await;

        let count: i64 = db
            .with_conn(|conn| {
                conn.query_row("SELECT COUNT(*) FROM _rb", [], |r| r.get(0))
                    .map_err(crate::errors::StoreError::from)
            })
            .await
            .unwrap();
        assert_eq!(count, 0, "rolled-back insert must not persist");
    }

    #[test]
    fn sync_methods_work() {
        let db = open_test_db();
        db.with_conn_sync(|conn| {
            conn.execute_batch("CREATE TABLE IF NOT EXISTS _sync (y INTEGER);")
                .map_err(crate::errors::StoreError::from)
        })
        .unwrap();
        db.with_tx_sync(|tx| {
            tx.execute("INSERT INTO _sync VALUES (7)", [])
                .map(|_| ())
                .map_err(crate::errors::StoreError::from)
        })
        .unwrap();
        let y: i64 = db
            .with_conn_sync(|conn| {
                conn.query_row("SELECT y FROM _sync", [], |r| r.get(0))
                    .map_err(crate::errors::StoreError::from)
            })
            .unwrap();
        assert_eq!(y, 7);
    }
}
