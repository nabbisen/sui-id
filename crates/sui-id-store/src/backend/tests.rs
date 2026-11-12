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

/// RFC 096-B2 S4 §A row 2/§E: `Database::with_tx_immediate` must open
/// `BEGIN IMMEDIATE`, not SQLite's `DEFERRED` default
/// (`Database::with_tx`'s own mode, unchanged). A file-backed database,
/// WAL mode (the same mode `Database::open` always sets): under WAL, a
/// `DEFERRED` transaction's first statement, if it is only a read, never
/// waits for an uncommitted writer at all -- readers see the last
/// committed snapshot regardless. `IMMEDIATE` requests the write lock
/// before any statement runs, so it must wait for a concurrent,
/// uncommitted write to finish, real contention from a second OS-level
/// connection to the same file (empirically confirmed against this
/// crate's own journal mode and `busy_timeout` before relying on it, with
/// a throwaway Python/`sqlite3` probe, not assumed from SQLite's docs
/// alone).
#[tokio::test]
async fn with_tx_immediate_waits_for_a_held_write_lock_but_with_tx_does_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("immediate.db");
    let db = Database::open(&path, MasterKey::generate()).expect("open file db");
    db.with_conn(|conn| {
        conn.execute_batch("CREATE TABLE IF NOT EXISTS _immediate_probe (x INTEGER);")
            .map_err(crate::errors::StoreError::from)
    })
    .await
    .expect("create probe table");

    // A second, independent connection to the same file holds an
    // uncommitted write -- the real external lock neither side of this
    // test goes through `Database`'s own single-connection mutex for.
    let lock_conn = rusqlite::Connection::open(&path).expect("second connection");
    lock_conn
        .pragma_update(None, "busy_timeout", 5000)
        .expect("busy_timeout");
    lock_conn
        .execute_batch("BEGIN; INSERT INTO _immediate_probe VALUES (1);")
        .expect("hold the write lock, uncommitted");

    // DEFERRED: a read-only transaction must not wait for it at all.
    let deferred_read = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        db.with_tx(|tx| {
            tx.query_row("SELECT COUNT(*) FROM _immediate_probe", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(crate::errors::StoreError::from)
        }),
    )
    .await
    .expect("a DEFERRED read must not wait for the external write lock")
    .expect("query");
    assert_eq!(
        deferred_read, 0,
        "WAL readers see the last *committed* snapshot, not the in-flight write"
    );

    // IMMEDIATE: must wait. Spawned so the assertion below can observe
    // it has not finished *before* the external lock is released.
    let db_for_task = db.clone();
    let task = tokio::spawn(async move {
        db_for_task
            .with_tx_immediate(|tx| {
                tx.query_row("SELECT COUNT(*) FROM _immediate_probe", [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(crate::errors::StoreError::from)
            })
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(
        !task.is_finished(),
        "BEGIN IMMEDIATE must still be waiting for the held write lock"
    );

    lock_conn
        .execute_batch("COMMIT;")
        .expect("release the write lock");

    let result = tokio::time::timeout(std::time::Duration::from_secs(5), task)
        .await
        .expect("must complete once unblocked")
        .expect("task panicked");
    assert_eq!(
        result.expect("must succeed"),
        1,
        "now sees the committed write -- it genuinely waited for it, not a stale read"
    );
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
