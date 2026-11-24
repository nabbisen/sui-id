use super::*;
use crate::crypto::MasterKey;
use crate::migrations::{self, MAX_SCHEMA_VERSION};
use crate::{Database, errors::StoreError};
use rusqlite::{Connection, OptionalExtension};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

/// `MasterKey` derives no `Clone` (by design); every caller here that
/// needs the same key bytes twice copies them this way, the same idiom
/// `commands::reseal_master_key` uses.
fn copy_key(key: &MasterKey) -> MasterKey {
    MasterKey::from_bytes(*key.as_bytes())
}

fn remove_after_verified() -> RotationDisposition {
    RotationDisposition::RemoveAfterVerified {
        verification_reference: "verified-by-test".into(),
    }
}

async fn insert(
    db: &Database,
    record: RotationRecord,
    old_key: &MasterKey,
    new_key: &MasterKey,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let old_key = copy_key(old_key);
    let new_key = copy_key(new_key);
    db.with_conn(move |c| insert_pending_within_tx(c, &record, &old_key, &new_key, now))
        .await
}

#[tokio::test]
async fn insert_then_read_round_trips_remove_after_verified() {
    let db = fresh_db();
    let old_key = copy_key(db.key());
    let new_key = MasterKey::generate();
    let record = RotationRecord {
        rotation_id: "r1".into(),
        old_fingerprint: crate::crypto::fingerprint(&old_key),
        backup_filename: "key.pre-rotation".into(),
        disposition: remove_after_verified(),
    };
    let now = chrono::Utc::now();
    insert(&db, record, &old_key, &new_key, now)
        .await
        .expect("insert");

    let row = db
        .with_conn(read_within_tx)
        .await
        .expect("read")
        .expect("a row exists");
    assert_eq!(row.rotation_id, "r1");
    assert_eq!(row.old_fingerprint, crate::crypto::fingerprint(&old_key));
    assert_eq!(row.new_fingerprint, crate::crypto::fingerprint(&new_key));
    assert_eq!(row.state, RotationState::DbCommittedPendingActivation);
    assert_eq!(row.completed_at, None);
    assert_eq!(row.disposition, remove_after_verified());
}

#[tokio::test]
async fn insert_then_read_round_trips_retain_until() {
    let db = fresh_db();
    let old_key = copy_key(db.key());
    let new_key = MasterKey::generate();
    let review_at = chrono::Utc::now() + chrono::TimeDelta::days(90);
    let record = RotationRecord {
        rotation_id: "r2".into(),
        old_fingerprint: crate::crypto::fingerprint(&old_key),
        backup_filename: "key.pre-rotation".into(),
        disposition: RotationDisposition::RetainUntil {
            review_at,
            custody_reference: "ticket-1234".into(),
        },
    };
    let now = chrono::Utc::now();
    insert(&db, record, &old_key, &new_key, now)
        .await
        .expect("insert");

    let row = db
        .with_conn(read_within_tx)
        .await
        .expect("read")
        .expect("a row exists");
    match row.disposition {
        RotationDisposition::RetainUntil {
            review_at: got_review_at,
            custody_reference,
        } => {
            // `DateTime` round-trips through SQLite text storage with
            // sub-second truncation possible depending on format -- a
            // test that compared with `assert_eq!` could be fragile to
            // that, so this compares the timestamp a human would read.
            assert_eq!(got_review_at.timestamp(), review_at.timestamp());
            assert_eq!(custody_reference, "ticket-1234");
        }
        other => panic!("expected RetainUntil, got {other:?}"),
    }
}

#[tokio::test]
async fn insert_refuses_when_already_pending() {
    let db = fresh_db();
    let old_key = copy_key(db.key());
    let new_key = MasterKey::generate();
    let record = RotationRecord {
        rotation_id: "r3".into(),
        old_fingerprint: crate::crypto::fingerprint(&old_key),
        backup_filename: "key.pre-rotation".into(),
        disposition: remove_after_verified(),
    };
    let now = chrono::Utc::now();
    insert(&db, record.clone(), &old_key, &new_key, now)
        .await
        .expect("first insert");

    let second = RotationRecord {
        rotation_id: "r4".into(),
        ..record
    };
    let result = insert(&db, second, &old_key, &new_key, now).await;
    assert!(matches!(
        result,
        Err(StoreError::RotationRefused(
            crate::errors::RotationRefusal::AlreadyPending
        ))
    ));
}

#[tokio::test]
async fn insert_refuses_on_old_fingerprint_mismatch() {
    let db = fresh_db();
    let old_key = copy_key(db.key());
    let new_key = MasterKey::generate();
    let wrong_old_key = MasterKey::generate();
    let record = RotationRecord {
        rotation_id: "r5".into(),
        old_fingerprint: crate::crypto::fingerprint(&wrong_old_key),
        backup_filename: "key.pre-rotation".into(),
        disposition: remove_after_verified(),
    };
    let now = chrono::Utc::now();
    let result = insert(&db, record, &old_key, &new_key, now).await;
    assert!(matches!(
        result,
        Err(StoreError::RotationRefused(
            crate::errors::RotationRefusal::OldFingerprintMismatch
        ))
    ));
    assert!(
        db.with_conn(read_within_tx).await.expect("read").is_none(),
        "no row written"
    );
}

#[tokio::test]
async fn complete_refuses_when_no_row() {
    let db = fresh_db();
    let now = chrono::Utc::now();
    let result = db
        .with_conn(move |c| mark_complete_within_tx(c, "r1", now))
        .await;
    assert!(matches!(
        result,
        Err(StoreError::RotationRefused(
            crate::errors::RotationRefusal::NoRow
        ))
    ));
}

#[tokio::test]
async fn complete_refuses_on_rotation_id_mismatch() {
    let db = fresh_db();
    let old_key = copy_key(db.key());
    let new_key = MasterKey::generate();
    let record = RotationRecord {
        rotation_id: "r6".into(),
        old_fingerprint: crate::crypto::fingerprint(&old_key),
        backup_filename: "key.pre-rotation".into(),
        disposition: remove_after_verified(),
    };
    let now = chrono::Utc::now();
    insert(&db, record, &old_key, &new_key, now)
        .await
        .expect("insert");

    let result = db
        .with_conn(move |c| mark_complete_within_tx(c, "not-r6", now))
        .await;
    assert!(matches!(
        result,
        Err(StoreError::RotationRefused(
            crate::errors::RotationRefusal::RotationIdMismatch
        ))
    ));
}

#[tokio::test]
async fn complete_run_twice_is_idempotent_and_reports_no_write_the_second_time() {
    let db = fresh_db();
    let old_key = copy_key(db.key());
    let new_key = MasterKey::generate();
    let record = RotationRecord {
        rotation_id: "r7".into(),
        old_fingerprint: crate::crypto::fingerprint(&old_key),
        backup_filename: "key.pre-rotation".into(),
        disposition: remove_after_verified(),
    };
    let now = chrono::Utc::now();
    insert(&db, record, &old_key, &new_key, now)
        .await
        .expect("insert");

    let wrote_first = db
        .with_conn(move |c| mark_complete_within_tx(c, "r7", now))
        .await
        .expect("first complete");
    assert!(wrote_first);
    let wrote_second = db
        .with_conn(move |c| mark_complete_within_tx(c, "r7", now))
        .await
        .expect("second complete");
    assert!(!wrote_second, "the second call writes nothing");

    let row = db
        .with_conn(read_within_tx)
        .await
        .expect("read")
        .expect("row");
    assert_eq!(row.state, RotationState::Complete);
    assert!(row.completed_at.is_some());
}

// ── migration 0053 itself ───────────────────────────────────────────────

#[test]
fn migration_0053_upgrades_an_empty_0_93_1_shaped_table() {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_up_to(&mut conn, 52).expect("run_up_to(52)");
    // The 0.93.1 shape: `started_at`/`updated_at`, free-text
    // `custody_disposition`, no `prepared_at`/`committed_at`/
    // `completed_at`, no structured disposition columns.
    conn.query_row(
        "SELECT started_at, updated_at, custody_disposition FROM master_key_rotation",
        [],
        |_| Ok(()),
    )
    .optional()
    .expect("the 0.93.1 columns exist and the table is queryable, even empty");
    let count: i64 = conn
        .query_row("SELECT count(*) FROM master_key_rotation", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 0, "no release has ever written a row");

    migrations::run_up_to(&mut conn, MAX_SCHEMA_VERSION).expect("run_up_to head");

    // The full shape now exists and is still empty.
    let count: i64 = conn
        .query_row("SELECT count(*) FROM master_key_rotation", [], |r| r.get(0))
        .expect("count after 0053");
    assert_eq!(count, 0);
    let row = read_within_tx(&conn).expect("read_within_tx on the upgraded, empty table");
    assert!(row.is_none());
    // The full-shape columns are queryable (0053 actually ran, not a
    // no-op): a query naming them would fail with "no such column" on
    // the pre-0053 shape.
    conn.query_row(
        "SELECT prepared_at, committed_at, completed_at, disposition_kind, review_at, \
                custody_reference, verification_reference \
         FROM master_key_rotation",
        [],
        |_| Ok(()),
    )
    .optional()
    .expect("the 0053 columns exist");
}

#[test]
fn migration_0053_refuses_a_populated_table_and_changes_nothing() {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_up_to(&mut conn, 52).expect("run_up_to(52)");
    // Constructed directly, in the 0.93.1 shape -- no release has ever
    // written one, so this is the "should never happen" case the
    // migration's own guard exists for.
    conn.execute(
        "INSERT INTO master_key_rotation \
         (id, rotation_id, old_fingerprint, new_fingerprint, state, started_at, updated_at, \
          backup_filename, custody_disposition) \
         VALUES (1, 'r1', 'old-fp', 'new-fp', 'complete', '2026-01-01T00:00:00Z', \
                 '2026-01-01T00:00:00Z', 'key.pre-rotation', 'free text, pre-0053')",
        [],
    )
    .expect("seed a 0.93.1-shaped row");

    let before_schema: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'master_key_rotation'",
            [],
            |r| r.get(0),
        )
        .expect("schema before");
    let before_version: i64 = conn
        .query_row(
            "SELECT value FROM sui_meta WHERE key = 'schema_version'",
            [],
            |r| {
                r.get::<_, String>(0)?
                    .parse()
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .expect("stored version before");

    let result = migrations::run_up_to(&mut conn, MAX_SCHEMA_VERSION);
    assert!(
        result.is_err(),
        "a populated table refuses the upgrade: {result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(&err, StoreError::MigrationFailed { version: 53, .. }),
        "{err:?}"
    );
    let source_message = std::error::Error::source(&err)
        .map(ToString::to_string)
        .unwrap_or_default();
    assert!(
        source_message.contains("already holds a row"),
        "{source_message}"
    );

    // Byte-identical: the schema, the stored version, and the row.
    let after_schema: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'master_key_rotation'",
            [],
            |r| r.get(0),
        )
        .expect("schema after");
    assert_eq!(
        before_schema, after_schema,
        "the table's own shape is unchanged"
    );
    let after_version: i64 = conn
        .query_row(
            "SELECT value FROM sui_meta WHERE key = 'schema_version'",
            [],
            |r| {
                r.get::<_, String>(0)?
                    .parse()
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .expect("stored version after");
    assert_eq!(
        before_version, after_version,
        "the stored schema version did not advance"
    );
    let (rotation_id, custody_disposition): (String, String) = conn
        .query_row(
            "SELECT rotation_id, custody_disposition FROM master_key_rotation WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the seeded row is still there, in its original shape");
    assert_eq!(rotation_id, "r1");
    assert_eq!(custody_disposition, "free text, pre-0053");
}

// ── M5: a row with no disposition is unrepresentable ──────────────────

/// M5 says K05 cannot complete a row whose disposition is missing. The
/// 0053 CHECK constraint makes that state impossible to write in the
/// first place -- this is that claim, shown directly against the real
/// schema rather than inferred from K08/K05's own call sites.
#[tokio::test]
async fn a_row_with_no_disposition_fields_at_all_violates_the_check_constraint() {
    let db = fresh_db();
    let err = db
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO master_key_rotation (
                    id, rotation_id, old_fingerprint, new_fingerprint, state,
                    prepared_at, committed_at, backup_filename, disposition_kind
                ) VALUES (1, 'r-no-disposition', 'old-fp', 'new-fp', 'db_committed_pending_activation',
                    '2026-11-24T00:00:00Z', '2026-11-24T00:00:00Z', 'backup.bin', 'retain_until')",
                [],
            )
            .map_err(StoreError::from)
        })
        .await
        .expect_err("a disposition_kind with none of its own fields present must be refused");
    let message = std::error::Error::source(&err)
        .map(|s| s.to_string())
        .unwrap_or_default();
    assert!(
        message.contains("CHECK") || message.contains("constraint"),
        "expected a CHECK-constraint failure, got: {message}"
    );
}

/// The same claim for `remove_after_verified`, missing its own required
/// field.
#[tokio::test]
async fn a_remove_after_verified_row_with_no_verification_reference_violates_the_check_constraint()
{
    let db = fresh_db();
    let err = db
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO master_key_rotation (
                    id, rotation_id, old_fingerprint, new_fingerprint, state,
                    prepared_at, committed_at, backup_filename, disposition_kind
                ) VALUES (1, 'r-no-verification-ref', 'old-fp', 'new-fp', 'db_committed_pending_activation',
                    '2026-11-24T00:00:00Z', '2026-11-24T00:00:00Z', 'backup.bin', 'remove_after_verified')",
                [],
            )
            .map_err(StoreError::from)
        })
        .await
        .expect_err("remove_after_verified with no verification_reference must be refused");
    let message = std::error::Error::source(&err)
        .map(|s| s.to_string())
        .unwrap_or_default();
    assert!(
        message.contains("CHECK") || message.contains("constraint"),
        "expected a CHECK-constraint failure, got: {message}"
    );
}
