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
