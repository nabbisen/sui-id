use super::*;
use std::path::PathBuf;

fn p() -> PathBuf {
    PathBuf::from("/var/lib/sui-id/sui-id.sqlite")
}

#[test]
fn a_too_new_line_names_the_path_both_versions_the_route_and_what_not_to_do() {
    let line = too_new_line(&p(), 44, 43, Some("0.79.0"));
    assert!(line.starts_with("sui-id: refusing to run:"), "{line}");
    assert!(line.contains("/var/lib/sui-id/sui-id.sqlite"), "the path");
    assert!(line.contains("schema version 44"), "the found version");
    assert!(line.contains("understands up to 43"), "the ceiling");
    assert!(line.contains(env!("CARGO_PKG_VERSION")), "this binary");
    assert!(line.contains("last migrated by sui-id 0.79.0"), "the stamp");
    assert!(
        line.contains("Run sui-id 0.79.0 or newer"),
        "the route forward"
    );
    assert!(line.contains("sui-id restore"), "the route back");
    assert!(line.contains("--force"), "the route back is runnable");
    assert!(line.contains("Nothing was changed"), "{line}");
    assert!(
        line.contains("Do not edit or delete the database"),
        "{line}"
    );
    assert!(line.contains("do not run migrations by hand"), "{line}");
    // The words the deployment guide's grep looks for.
    assert!(line.contains("schema") && line.contains("migrat"), "{line}");
}

#[test]
fn without_a_stamp_the_line_says_a_newer_sui_id() {
    let line = too_new_line(&p(), 44, 43, None);
    assert!(line.contains("Run a newer sui-id,"), "{line}");
    assert!(!line.contains("last migrated by"), "{line}");
}

#[test]
fn an_invalid_line_names_the_path_the_detail_and_the_table_count() {
    let line = invalid_line(
        &p(),
        24,
        "the recorded value \"garbage\" is not a non-negative integer",
    );
    assert!(line.starts_with("sui-id: refusing to run:"), "{line}");
    assert!(line.contains("/var/lib/sui-id/sui-id.sqlite"), "{line}");
    assert!(line.contains("\"garbage\""), "{line}");
    assert!(line.contains("24 application table(s)"), "{line}");
    assert!(line.contains("no migrations were run"), "{line}");
    assert!(line.contains("Nothing was changed"), "{line}");
    assert!(line.contains("sui-id restore"), "{line}");
    assert!(line.contains("schema") && line.contains("migrat"), "{line}");
}

#[test]
fn a_line_is_one_line_whatever_a_path_or_a_stored_value_contained() {
    let path = PathBuf::from("/tmp/a\nb\u{1b}[31m/db.sqlite");
    for line in [
        too_new_line(&path, 44, 43, Some("0.79.0\nX")),
        invalid_line(&path, 1, "a\r\nb"),
    ] {
        assert!(
            !line.contains('\n') && !line.contains('\r') && !line.contains('\u{1b}'),
            "{line:?}"
        );
    }
}

#[test]
fn only_the_two_schema_refusals_are_a_refusal() {
    let too_new = StoreError::SchemaTooNew {
        found: 44,
        supported: 43,
    };
    let invalid = StoreError::SchemaVersionInvalid {
        tables: 3,
        detail: "x".into(),
    };
    assert!(refusal(&p(), &too_new).is_some());
    assert!(refusal(&p(), &invalid).is_some());
    for other in [
        StoreError::NotFound,
        StoreError::Conflict,
        StoreError::Integrity("x".into()),
        StoreError::MigrationFailed {
            version: 2,
            source: rusqlite::Error::InvalidQuery,
        },
    ] {
        assert!(refusal(&p(), &other).is_none(), "{other:?}");
    }
}

#[test]
fn both_refusal_variants_share_one_exit_code() {
    // D5: a code covering one variant covers nothing.
    assert_eq!(EXIT_DATABASE_REFUSED, 65);
}

#[test]
fn a_refusal_survives_anyhow_context_and_downcasts() {
    let refused: anyhow::Error = refusal(
        &p(),
        &StoreError::SchemaTooNew {
            found: 44,
            supported: 43,
        },
    )
    .unwrap()
    .into();
    let wrapped = refused.context("some caller's wrapper");
    assert!(wrapped.downcast_ref::<DatabaseRefused>().is_some());
}

// ── RFC 100 stage 1: `open_checked` ─────────────────────────────────────

use rusqlite::OptionalExtension;
use sui_id_store::crypto::MasterKey;

fn write_key_file_for_test(path: &std::path::Path, key: &MasterKey) {
    std::fs::write(path, format!("{}\n", key.to_base64())).expect("write key file");
}

/// A sealed signing-key row under `key`, migrated to `MAX_SCHEMA_VERSION`
/// (today's build). With no sentinel row written yet, this is exactly
/// an upgrade's shape — sealed data, no sentinel — without needing to
/// fake an older stored schema version: the sentinel table exists
/// (migration 52 already ran) but holds nothing until something writes
/// to it.
fn seed_sealed_signing_key(db_path: &std::path::Path, key: &MasterKey) {
    let mut conn = rusqlite::Connection::open(db_path).expect("open for seeding");
    sui_id_store::migrations::run(&mut conn).expect("migrate to head");
    let sealed = sui_id_store::crypto::seal(
        key,
        b"private-key-material-for-a-test",
        b"sui-id/signing_key/v1",
    )
    .expect("seal");
    conn.execute(
        "INSERT INTO signing_keys (id, algorithm, private_key_enc, public_key, is_active, created_at) \
         VALUES ('k1', 'Ed25519', ?1, X'00', 1, '2026-01-01T00:00:00Z')",
        rusqlite::params![sealed],
    )
    .expect("insert sealed row");
}

#[tokio::test]
async fn a_wrong_key_refuses_before_readiness_and_touches_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t.sqlite");
    let key_file = dir.path().join("t.key");
    let right_key = MasterKey::generate();
    seed_sealed_signing_key(&db_path, &right_key);
    let bytes_before = std::fs::read(&db_path).expect("read before");

    let wrong_key = MasterKey::generate();
    write_key_file_for_test(&key_file, &wrong_key);

    let err = crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
        .await
        .map(|_| ())
        .expect_err("a wrong key must refuse");
    assert!(err.downcast_ref::<DatabaseRefused>().is_some());

    let bytes_after = std::fs::read(&db_path).expect("read after");
    assert_eq!(
        bytes_before, bytes_after,
        "a refusal must not touch the database file at all, not even its schema version"
    );
}

#[tokio::test]
async fn a_missing_key_file_over_a_sealed_database_refuses_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t.sqlite");
    let key_file = dir.path().join("t.key"); // never created
    let key = MasterKey::generate();
    seed_sealed_signing_key(&db_path, &key);

    let err = crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
        .await
        .map(|_| ())
        .expect_err("a missing key over a sealed database must refuse");
    assert!(err.downcast_ref::<DatabaseRefused>().is_some());
    assert!(
        !key_file.exists(),
        "refusing to generate means exactly that: no key file appears"
    );
}

#[tokio::test]
async fn a_missing_key_file_over_a_never_sealed_database_generates_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t.sqlite");
    let key_file = dir.path().join("t.key"); // never created

    let (db, origin) =
        crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
            .await
            .expect("a never-sealed database with no key must generate one");
    assert_eq!(origin, crate::keyring::KeyOrigin::GeneratedNew);
    assert!(key_file.exists());
    let _ = db;
}

#[tokio::test]
async fn upgrade_sealed_rows_the_key_opens_verifies_then_writes_the_sentinel() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t.sqlite");
    let key_file = dir.path().join("t.key");
    let key = MasterKey::generate();
    // Migrated to head, with a sealed signing-key row and no sentinel
    // row yet (nothing has written one) — exactly an upgrade's shape:
    // sealed data, no sentinel.
    seed_sealed_signing_key(&db_path, &key);
    write_key_file_for_test(&key_file, &key);

    let (db, origin) =
        crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
            .await
            .expect("the right key must verify against the one sealed row and proceed");
    assert_eq!(origin, crate::keyring::KeyOrigin::KeyFile);
    let row = db
        .with_conn(|conn| Ok(sui_id_store::master_key_check::read_sentinel(conn)?))
        .await
        .expect("with_conn")
        .expect("the sentinel must exist after a verified upgrade");
    assert_eq!(row.key_fingerprint, sui_id_store::crypto::fingerprint(&key));
}

#[tokio::test]
async fn upgrade_sealed_rows_the_key_does_not_open_refuses() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t.sqlite");
    let key_file = dir.path().join("t.key");
    let right_key = MasterKey::generate();
    seed_sealed_signing_key(&db_path, &right_key);
    let wrong_key = MasterKey::generate();
    write_key_file_for_test(&key_file, &wrong_key);

    let err = crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
        .await
        .map(|_| ())
        .expect_err("the wrong key over an unverified upgrade must refuse");
    assert!(err.downcast_ref::<DatabaseRefused>().is_some());
}

#[tokio::test]
async fn migrating_up_from_0_92_0_creates_both_tables_and_writes_the_sentinel_under_the_existing_key()
 {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t.sqlite");
    let key_file = dir.path().join("t.key");
    let key = MasterKey::generate();
    // A database exactly at 0.92.0's schema ceiling (51): migrate up to
    // it, not to head, then seed the sealed row that 0.92.0 itself could
    // have written.
    {
        let mut conn = rusqlite::Connection::open(&db_path).expect("open");
        sui_id_store::migrations::run_up_to(&mut conn, 51).expect("migrate to 0.92.0's ceiling");
        let sealed = sui_id_store::crypto::seal(
            &key,
            b"private-key-material-for-a-test",
            b"sui-id/signing_key/v1",
        )
        .expect("seal");
        conn.execute(
            "INSERT INTO signing_keys (id, algorithm, private_key_enc, public_key, is_active, created_at) \
             VALUES ('k1', 'Ed25519', ?1, X'00', 1, '2026-01-01T00:00:00Z')",
            rusqlite::params![sealed],
        )
        .expect("insert sealed row");
    }
    write_key_file_for_test(&key_file, &key);

    let (db, _origin) =
        crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
            .await
            .expect("migrating up from 0.92.0 with the right key must succeed");
    let version = db.stored_schema_version().await.expect("stored version");
    assert_eq!(
        version,
        i64::from(sui_id_store::migrations::MAX_SCHEMA_VERSION)
    );
    let row = db
        .with_conn(|conn| Ok(sui_id_store::master_key_check::read_sentinel(conn)?))
        .await
        .expect("with_conn")
        .expect("the sentinel must exist after migrating up and verifying");
    assert_eq!(row.key_fingerprint, sui_id_store::crypto::fingerprint(&key));
    // `master_key_rotation` exists too, even though stage 1 writes
    // nothing to it yet.
    let rotation_table_exists = db
        .with_conn(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'master_key_rotation'",
                    [],
                    |_| Ok(()),
                )
                .optional()?
                .is_some())
        })
        .await
        .expect("with_conn");
    assert!(
        rotation_table_exists,
        "master_key_rotation must exist after migrating up"
    );
}
