#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::clone_on_copy,
    clippy::panic
)]
//! RFC 112 stage 1: a database this build does not understand is refused, and
//! nothing is written doing it.
//!
//! One test per branch of the rule: the reader's classification, the ceiling,
//! and `Database::open` against files (rollback-journal and WAL, each asserted
//! byte for byte), a foreign SQLite file, a populated database whose version row
//! is gone, and every unreadable stamp; plus the runner's race and its failure
//! type. The backup callers of the same reader are tested beside `backup`.

use crate::Database;
use crate::crypto::MasterKey;
use crate::errors::StoreError;
use crate::migrations::{
    self, MAX_SCHEMA_VERSION, SchemaError, StoredVersion, check_supported, last_migrated_by,
    read_stored_version,
};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const TOO_NEW: i64 = MAX_SCHEMA_VERSION as i64 + 1;

fn migrated_file(dir: &Path, journal: &str) -> PathBuf {
    let path = dir.join("t.sqlite");
    let db = Database::open(&path, MasterKey::generate()).expect("open a fresh database");
    drop(db);
    let conn = Connection::open(&path).expect("raw");
    // A user row, so "every row count is identical" has something to lose.
    conn.execute(
        "INSERT INTO users(id, username, is_admin, is_disabled, is_deleted, created_at, \
                           updated_at, user_uuid, failed_login_count) \
         VALUES('u1','u1',0,0,0,datetime('now'),datetime('now'),?1,0)",
        [uuid::Uuid::new_v4().to_string()],
    )
    .expect("a user");
    let _: String = conn
        .query_row(&format!("PRAGMA journal_mode={journal}"), [], |r| r.get(0))
        .expect("journal mode");
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").ok();
    path
}

fn stamp(path: &Path, sql: &str) {
    Connection::open(path)
        .expect("raw")
        .execute_batch(sql)
        .expect("stamp");
    // Leave the main file the whole truth, in either journal mode.
    let c = Connection::open(path).expect("raw");
    c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").ok();
}

/// Everything the refusal must leave alone.
#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    main_file: Vec<u8>,
    master: Vec<String>,
    rows: Vec<(String, i64)>,
    stored: String,
}

fn fingerprint(path: &Path) -> Fingerprint {
    let c = Connection::open(path).expect("raw");
    let master: Vec<String> = c
        .prepare("SELECT COALESCE(sql,'') FROM sqlite_master ORDER BY type, name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let names: Vec<String> = c
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' \
             ORDER BY name",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let rows = names
        .into_iter()
        .map(|n| {
            let k: i64 = c
                .query_row(&format!("SELECT COUNT(*) FROM \"{n}\""), [], |r| r.get(0))
                .unwrap();
            (n, k)
        })
        .collect();
    let stored: String = c
        .query_row(
            "SELECT COALESCE(CAST(value AS TEXT), 'NULL') || ' (' || typeof(value) || ')' \
             FROM sui_meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "<no row>".into());
    drop(c);
    Fingerprint {
        main_file: std::fs::read(path).unwrap(),
        master,
        rows,
        stored,
    }
}

fn open(path: &Path) -> Result<Database, StoreError> {
    Database::open(path, MasterKey::generate())
}

// ── the reader ───────────────────────────────────────────────────────────

fn mem(sql: &str) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(sql).unwrap();
    c
}

fn with_version(value_sql: &str) -> Connection {
    mem(&format!(
        "CREATE TABLE sui_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL); \
         CREATE TABLE users(id TEXT); \
         INSERT INTO sui_meta(key, value) VALUES('schema_version', {value_sql});"
    ))
}

#[test]
fn an_empty_database_is_fresh_and_so_is_one_with_only_sui_meta() {
    assert_eq!(read_stored_version(&mem("")).unwrap(), StoredVersion::Fresh);
    let only_meta = mem(
        "CREATE TABLE sui_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL); \
                         INSERT INTO sui_meta VALUES('initialized','true');",
    );
    assert_eq!(
        read_stored_version(&only_meta).unwrap(),
        StoredVersion::Fresh
    );
}

#[test]
fn a_recorded_canonical_version_is_read_as_that_number() {
    for (text, want) in [("'1'", 1), ("'43'", 43), ("'44'", 44), ("'10'", 10)] {
        assert_eq!(
            read_stored_version(&with_version(text)).unwrap(),
            StoredVersion::At(want),
            "{text}"
        );
    }
}

#[test]
fn a_populated_database_with_no_version_is_invalid_and_names_the_table_count() {
    // The version row is gone.
    let c = mem(
        "CREATE TABLE sui_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL); \
                 CREATE TABLE a(x); CREATE TABLE b(x);",
    );
    match read_stored_version(&c) {
        Err(SchemaError::Invalid { tables, detail }) => {
            assert_eq!(tables, 2);
            assert!(detail.contains("no schema version is recorded"), "{detail}");
        }
        other => panic!("{other:?}"),
    }
    // A foreign SQLite file: no sui_meta at all.
    match read_stored_version(&mem("CREATE TABLE ledger(x TEXT);")) {
        Err(SchemaError::Invalid { tables: 1, .. }) => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn every_unreadable_stamp_is_invalid_not_a_version() {
    for (label, value) in [
        ("garbage", "'garbage'"),
        ("empty", "''"),
        ("negative", "'-1'"),
        ("a leading plus", "'+43'"),
        ("a leading space", "' 43'"),
        ("a trailing space", "'43 '"),
        ("a leading zero", "'043'"),
        ("a fraction", "'43.0'"),
        ("hex", "'0x2b'"),
        ("a BLOB", "X'3433'"),
    ] {
        match read_stored_version(&with_version(value)) {
            Err(SchemaError::Invalid { tables: 1, detail }) => {
                assert!(!detail.is_empty(), "{label}");
            }
            other => panic!("{label}: {other:?}"),
        }
    }
    // A BLOB says so.
    match read_stored_version(&with_version("X'3433'")) {
        Err(SchemaError::Invalid { detail, .. }) => assert!(detail.contains("BLOB"), "{detail}"),
        other => panic!("{other:?}"),
    }
    // An explicit `0` in a populated database is not "fresh".
    assert!(matches!(
        read_stored_version(&with_version("'0'")),
        Err(SchemaError::Invalid { .. })
    ));
}

#[test]
fn a_value_beyond_i32_is_too_new_not_invalid() {
    // Parsed as i64 (RFC 112 D6): it used to overflow i32 and be called invalid.
    let big = read_stored_version(&with_version("'99999999999'")).unwrap();
    assert_eq!(big, StoredVersion::At(99_999_999_999));
    assert!(matches!(
        check_supported(big),
        Err(SchemaError::TooNew {
            found: 99_999_999_999,
            ..
        })
    ));
    // Beyond i64 too: all digits, so it saturates and is still too new.
    let huge = read_stored_version(&with_version("'99999999999999999999999999'")).unwrap();
    assert_eq!(huge, StoredVersion::At(i64::MAX));
    assert!(matches!(
        check_supported(huge),
        Err(SchemaError::TooNew { .. })
    ));
}

#[test]
fn a_read_error_is_returned_not_collapsed_to_a_version() {
    // `sui_meta` exists but cannot answer the question.
    let c = mem("CREATE TABLE sui_meta(x); CREATE TABLE users(id TEXT);");
    assert!(matches!(read_stored_version(&c), Err(SchemaError::Read(_))));
    let e: StoreError = read_stored_version(&c).unwrap_err().into();
    assert!(matches!(e, StoreError::Db(_)));
}

#[test]
fn the_ceiling_is_exact() {
    assert!(check_supported(StoredVersion::Fresh).is_ok());
    assert!(check_supported(StoredVersion::At(0)).is_ok());
    assert!(check_supported(StoredVersion::At(i64::from(MAX_SCHEMA_VERSION))).is_ok());
    match check_supported(StoredVersion::At(TOO_NEW)) {
        Err(SchemaError::TooNew { found, supported }) => {
            assert_eq!((found, supported), (TOO_NEW, MAX_SCHEMA_VERSION));
        }
        other => panic!("{other:?}"),
    }
    let e: StoreError = check_supported(StoredVersion::At(TOO_NEW))
        .unwrap_err()
        .into();
    assert!(matches!(e, StoreError::SchemaTooNew { .. }));
}

// ── Database::open refuses, and writes nothing ──────────────────────────

fn assert_refused_too_new_and_untouched(journal: &str) {
    let dir = TempDir::new().unwrap();
    let path = migrated_file(dir.path(), journal);
    stamp(
        &path,
        &format!("UPDATE sui_meta SET value='{TOO_NEW}' WHERE key='schema_version'"),
    );
    let mode: String = Connection::open(&path)
        .unwrap()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    let before = fingerprint(&path);
    let result = open(&path);
    match result {
        Err(StoreError::SchemaTooNew { found, supported }) => {
            assert_eq!((found, supported), (TOO_NEW, MAX_SCHEMA_VERSION));
        }
        Err(other) => panic!("{journal}: {other:?}"),
        Ok(_) => panic!("{journal}: a too-new database was opened"),
    }
    let after = fingerprint(&path);
    assert_eq!(
        after.main_file, before.main_file,
        "{journal}: the main file's bytes"
    );
    assert_eq!(after.master, before.master, "{journal}: sqlite_master");
    assert_eq!(after.rows, before.rows, "{journal}: every row count");
    assert_eq!(after.stored, before.stored, "{journal}: the stored version");
    let mode_after: String = Connection::open(&path)
        .unwrap()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        mode_after, mode,
        "{journal}: the journal mode was not switched"
    );
}

#[test]
fn a_too_new_database_is_refused_and_untouched_in_rollback_journal_mode() {
    // The case the RFC 112 review measured: `Database::open` used to convert this
    // file to WAL (header bytes 18-19 1 -> 2) before any check could refuse it.
    assert_refused_too_new_and_untouched("DELETE");
}

#[test]
fn a_too_new_database_is_refused_and_untouched_in_wal_mode() {
    assert_refused_too_new_and_untouched("WAL");
}

#[test]
fn a_foreign_sqlite_file_is_refused_not_migrated() {
    for journal in ["DELETE", "WAL"] {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("foreign.sqlite");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE ledger(x TEXT); INSERT INTO ledger VALUES('keep me');")
                .unwrap();
            let _: String = c
                .query_row(&format!("PRAGMA journal_mode={journal}"), [], |r| r.get(0))
                .unwrap();
        }
        stamp(&path, "");
        let before = fingerprint(&path);
        match open(&path) {
            Err(StoreError::SchemaVersionInvalid { tables, detail }) => {
                assert_eq!(tables, 1, "{journal}");
                assert!(detail.contains("no schema version"), "{detail}");
            }
            other => panic!("{journal}: {:?}", other.err()),
        }
        let after = fingerprint(&path);
        assert_eq!(after.main_file, before.main_file, "{journal}: bytes");
        assert_eq!(
            after.rows, before.rows,
            "{journal}: it still has one table, not 27"
        );
        assert_eq!(
            after.master, before.master,
            "{journal}: no sui_meta was created"
        );
    }
}

#[test]
fn a_populated_database_whose_version_row_is_gone_is_refused_and_the_row_is_not_recreated() {
    for journal in ["DELETE", "WAL"] {
        let dir = TempDir::new().unwrap();
        let path = migrated_file(dir.path(), journal);
        stamp(&path, "DELETE FROM sui_meta WHERE key='schema_version'");
        let before = fingerprint(&path);
        assert_eq!(before.stored, "<no row>");
        match open(&path) {
            Err(StoreError::SchemaVersionInvalid { tables, .. }) => {
                assert!(tables > 20, "{tables}")
            }
            other => panic!("{journal}: {:?}", other.err()),
        }
        let after = fingerprint(&path);
        assert_eq!(
            after, before,
            "{journal}: nothing changed, and no `1` was stamped"
        );
    }
}

#[test]
fn every_unreadable_stamp_is_refused_and_leaves_the_true_version_recorded() {
    // Each of these used to re-run 0001, stamp `1`, and fail at 0002, destroying
    // the real version and leaving the database unstartable.
    for (label, sql) in [
        (
            "garbage",
            "UPDATE sui_meta SET value='garbage' WHERE key='schema_version'",
        ),
        (
            "empty",
            "UPDATE sui_meta SET value='' WHERE key='schema_version'",
        ),
        (
            "negative",
            "UPDATE sui_meta SET value='-1' WHERE key='schema_version'",
        ),
        (
            "a leading plus",
            "UPDATE sui_meta SET value='+43' WHERE key='schema_version'",
        ),
        (
            "a BLOB",
            "UPDATE sui_meta SET value=X'3433' WHERE key='schema_version'",
        ),
    ] {
        let dir = TempDir::new().unwrap();
        let path = migrated_file(dir.path(), "WAL");
        stamp(&path, sql);
        let before = fingerprint(&path);
        match open(&path) {
            Err(StoreError::SchemaVersionInvalid { .. }) => {}
            other => panic!("{label}: {:?}", other.err()),
        }
        assert_eq!(fingerprint(&path), before, "{label}: nothing changed");
    }
}

#[test]
fn an_overflowing_stamp_is_reported_as_too_new() {
    let dir = TempDir::new().unwrap();
    let path = migrated_file(dir.path(), "WAL");
    stamp(
        &path,
        "UPDATE sui_meta SET value='99999999999' WHERE key='schema_version'",
    );
    match open(&path) {
        Err(StoreError::SchemaTooNew { found, .. }) => assert_eq!(found, 99_999_999_999),
        other => panic!("{:?}", other.err()),
    }
}

// ── what still works ────────────────────────────────────────────────────

#[test]
fn a_path_that_does_not_exist_and_an_empty_file_are_fresh_and_migrate() {
    let dir = TempDir::new().unwrap();
    let new_path = dir.path().join("new.sqlite");
    let db = open(&new_path).expect("a new path is a fresh database");
    drop(db);
    assert_eq!(
        read_stored_version(&Connection::open(&new_path).unwrap()).unwrap(),
        StoredVersion::At(i64::from(MAX_SCHEMA_VERSION))
    );

    let empty = dir.path().join("empty.sqlite");
    std::fs::write(&empty, b"").unwrap();
    open(&empty).expect("a zero-length file is a fresh database");
}

#[test]
fn a_current_database_reopens_and_open_in_memory_is_unchanged() {
    let dir = TempDir::new().unwrap();
    let path = migrated_file(dir.path(), "WAL");
    open(&path).expect("a current database reopens");
    Database::open_in_memory(MasterKey::generate()).expect("in memory");
}

#[test]
fn an_older_database_is_migrated_forward_and_stamped_with_the_release() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("old.sqlite");
    {
        let mut c = Connection::open(&path).unwrap();
        migrations::run_up_to(&mut c, 20).unwrap();
        assert_eq!(
            last_migrated_by(&c),
            Some(env!("CARGO_PKG_VERSION").to_owned())
        );
        c.execute_batch("UPDATE sui_meta SET value='20' WHERE key='schema_version'")
            .unwrap();
    }
    open(&path).expect("migrates forward");
    let c = Connection::open(&path).unwrap();
    assert_eq!(
        read_stored_version(&c).unwrap(),
        StoredVersion::At(i64::from(MAX_SCHEMA_VERSION))
    );
    assert_eq!(
        last_migrated_by(&c).as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_release_is_stamped_in_the_same_transaction_as_the_version() {
    // Both keys or neither: a migration that fails leaves the previous pair.
    let mut c = Connection::open_in_memory().unwrap();
    migrations::run_up_to(&mut c, 1).unwrap();
    // Migration 2 adds `clients.allowed_scopes`; add it first so 2 fails.
    c.execute_batch("ALTER TABLE clients ADD COLUMN allowed_scopes TEXT")
        .unwrap();
    c.execute_batch("UPDATE sui_meta SET value='1' WHERE key='schema_version'")
        .unwrap();
    c.execute_batch("UPDATE sui_meta SET value='0.0.0-before' WHERE key='last_migrated_by'")
        .unwrap();
    match migrations::run(&mut c) {
        Err(StoreError::MigrationFailed { version: 2, .. }) => {}
        other => panic!("{other:?}"),
    }
    assert_eq!(
        read_stored_version(&c).unwrap(),
        StoredVersion::At(1),
        "the version was not advanced"
    );
    assert_eq!(
        last_migrated_by(&c).as_deref(),
        Some("0.0.0-before"),
        "and neither was the stamp"
    );
}

#[test]
fn a_migration_that_fails_is_a_migration_failure_not_an_io_error() {
    let mut c = Connection::open_in_memory().unwrap();
    migrations::run_up_to(&mut c, 1).unwrap();
    c.execute_batch("ALTER TABLE clients ADD COLUMN allowed_scopes TEXT")
        .unwrap();
    let e = migrations::run(&mut c).unwrap_err();
    assert!(
        matches!(e, StoreError::MigrationFailed { version: 2, .. }),
        "{e:?}"
    );
    assert!(
        e.to_string().contains("migration 2"),
        "the Display says what failed, not \"database I/O error\": {e}"
    );
}

// ── two new binaries starting together ──────────────────────────────────

#[test]
fn two_binaries_opening_one_old_database_together_both_succeed() {
    // Before RFC 112 the version was read and then acted on without a lock, so
    // both decided to apply the same migration and the loser failed with
    // `duplicate column name`. The read is now inside BEGIN IMMEDIATE, and each
    // migration re-reads under its own immediate transaction. The database is
    // already in WAL mode so the contention is the runner's, not the journal-mode
    // pragma's (which contends on its own, and is not this RFC's).
    for round in 0..30 {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("race.sqlite");
        {
            let mut c = Connection::open(&path).unwrap();
            let _: String = c
                .query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))
                .unwrap();
            migrations::run_up_to(&mut c, 1).unwrap();
        }
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(6));
        let handles: Vec<_> = (0..6)
            .map(|_| {
                let (p, b) = (path.clone(), barrier.clone());
                std::thread::spawn(move || {
                    b.wait();
                    Database::open(&p, MasterKey::generate()).map(|_| ())
                })
            })
            .collect();
        for h in handles {
            h.join()
                .unwrap()
                .unwrap_or_else(|e| panic!("round {round}: {e:?}"));
        }
        assert_eq!(
            read_stored_version(&Connection::open(&path).unwrap()).unwrap(),
            StoredVersion::At(i64::from(MAX_SCHEMA_VERSION)),
            "round {round}"
        );
    }
}
