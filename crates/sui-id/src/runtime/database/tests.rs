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

use crate::key_adapter::KeyFileOps;
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

    let (db, origin, _lock) =
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

    let (db, origin, _lock) =
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

// ── RFC 100 `0.94.0` P7 §1: `open_checked` holds the database lock
// shared for as long as its caller keeps the returned guard alive --
// proved against the real lock file, not `Simulated`, since the
// property under test is exactly what the three-element return value
// itself gives a caller the means to do. ────────────────────────────

#[tokio::test]
async fn open_checked_holds_the_lock_shared_so_a_rotation_refuses_while_it_is_held() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t.sqlite");
    let key_file = dir.path().join("t.key"); // never created: generates one

    let (_db, _origin, db_lock) =
        crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
            .await
            .expect("open");

    let paths = crate::rotate::preconditions::RotationPaths::for_key_file(&key_file);
    let result = crate::rotate::preconditions::take_rotation_lock(
        &crate::key_adapter::Real,
        &paths,
        "rotate.lock",
    );
    assert!(
        result.is_err(),
        "a rotation must refuse while open_checked's own caller has the database open"
    );
    let err = result.err().expect("checked above");
    assert!(err.to_string().contains("sui-id is running"), "{err:#}");

    // Two ordinary `open_checked` callers (two CLI commands, or a
    // command beside a running server) coexist: both locks are shared.
    let (_db2, _origin2, db_lock2) =
        crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
            .await
            .expect("a second open_checked caller coexists with the first");

    drop(db_lock);
    drop(db_lock2);
    crate::rotate::preconditions::take_rotation_lock(
        &crate::key_adapter::Real,
        &paths,
        "rotate.lock",
    )
    .expect("released: the rotation now proceeds");
}

/// The other direction of the same property: `open_checked` (through
/// recovery's own shared, non-blocking take) refuses while a rotation
/// holds the lock exclusively -- the dispatch's own "recovery still
/// refuses while a rotation holds the lock exclusive," exercised
/// through the real, whole `open_checked` path, not `recover` alone.
#[tokio::test]
async fn open_checked_refuses_while_a_rotation_holds_the_lock_exclusively() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("t.sqlite");
    let key_file = dir.path().join("t.key");

    let paths = crate::rotate::preconditions::RotationPaths::for_key_file(&key_file);
    let rotation_lock = crate::rotate::preconditions::take_rotation_lock(
        &crate::key_adapter::Real,
        &paths,
        "rotate.lock",
    )
    .expect("models a live rotation holding the lock exclusively");

    let err = crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
        .await
        .map(|_| ())
        .expect_err("an ordinary open must refuse while a rotation is running");
    assert!(
        err.to_string().contains("a key rotation is running"),
        "{err:#}"
    );
    assert!(!db_path.exists(), "nothing was created");
    assert!(!key_file.exists(), "nothing was created");

    drop(rotation_lock);
    let (_db, _origin, _lock) =
        crate::database::open_checked(&db_path, &key_file, PreMigrationBackup::Declined)
            .await
            .expect("released: the open now proceeds");
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

    let (db, _origin, _lock) =
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

// ── RFC 100 `0.94.0` P1 §4/§5: the service is not ready on a pending row ──

/// A row 9 fixture: a published, `Prepared` journal; K08 already
/// committed (reseal_master_key_for_rotation, directly -- the live
/// rotation does not call it yet); and the active key file already
/// swapped to the new key, as if an earlier run had activated it.
/// `open_checked` is the only thing left owed: K05, then the clean.
async fn seed_row9(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf, MasterKey) {
    let old_key = MasterKey::generate();
    let new_key = MasterKey::generate();
    let db_path = dir.join("t.sqlite");
    let key_path = dir.join("t.key");
    write_key_file_for_test(&key_path, &old_key);
    let db = sui_id_store::Database::open(&db_path, {
        let mut bytes = *old_key.as_bytes();
        let copy = MasterKey::from_bytes(bytes);
        zeroize::Zeroize::zeroize(&mut bytes);
        copy
    })
    .expect("open");
    sui_id_store::master_key_check::ensure_sentinel_written(&db, db.key())
        .await
        .expect("sentinel");
    drop(db);

    let paths = crate::rotate::preconditions::RotationPaths::for_key_file(&key_path);
    let adapter = crate::key_adapter::Real;
    crate::rotate::journal::lock_workspace(&adapter, &paths.workspace).expect("lock");
    let body = crate::rotate::journal::sample_body(&key_path, &paths.workspace, &old_key, &new_key);
    let rotation_id = body.rotation_id.clone();
    let mut envelope =
        crate::rotate::journal::write_intent(&adapter, &paths.workspace, body, &old_key, &new_key)
            .expect("write_intent");
    write_key_file_for_test(&paths.next_temp, &new_key);
    envelope = crate::rotate::journal::advance(
        &adapter,
        &paths.workspace,
        &envelope,
        crate::rotate::journal::JournalState::NextTempVerified,
        &old_key,
        &new_key,
    )
    .expect("advance");
    adapter
        .publish_no_replace("test.next.publish", &paths.next_temp, &paths.next)
        .expect("publish next");
    envelope = crate::rotate::journal::advance(
        &adapter,
        &paths.workspace,
        &envelope,
        crate::rotate::journal::JournalState::NextPublished,
        &old_key,
        &new_key,
    )
    .expect("advance");
    write_key_file_for_test(&paths.backup_temp, &old_key);
    envelope = crate::rotate::journal::advance(
        &adapter,
        &paths.workspace,
        &envelope,
        crate::rotate::journal::JournalState::BackupTempVerified,
        &old_key,
        &new_key,
    )
    .expect("advance");
    adapter
        .publish_no_replace("test.backup.publish", &paths.backup_temp, &paths.backup)
        .expect("publish backup");
    envelope = crate::rotate::journal::advance(
        &adapter,
        &paths.workspace,
        &envelope,
        crate::rotate::journal::JournalState::BackupPublished,
        &old_key,
        &new_key,
    )
    .expect("advance");
    crate::rotate::journal::advance(
        &adapter,
        &paths.workspace,
        &envelope,
        crate::rotate::journal::JournalState::Prepared,
        &old_key,
        &new_key,
    )
    .expect("advance to Prepared");

    // Simulate K08: a real, open database, resealed through the real
    // command.
    let db = sui_id_store::Database::open(&db_path, {
        let mut bytes = *old_key.as_bytes();
        let copy = MasterKey::from_bytes(bytes);
        zeroize::Zeroize::zeroize(&mut bytes);
        copy
    })
    .expect("re-open with the old key");
    sui_id_store::commands::reseal_master_key_for_rotation(
        &db,
        &{
            let mut bytes = *new_key.as_bytes();
            let copy = MasterKey::from_bytes(bytes);
            zeroize::Zeroize::zeroize(&mut bytes);
            copy
        },
        sui_id_store::master_key_rotation::RotationRecord {
            rotation_id,
            old_fingerprint: sui_id_store::crypto::fingerprint(&old_key),
            backup_filename: paths
                .backup
                .file_name()
                .and_then(|n| n.to_str())
                .expect("utf8")
                .to_owned(),
            disposition:
                sui_id_store::master_key_rotation::RotationDisposition::RemoveAfterVerified {
                    verification_reference: "test".into(),
                },
        },
    )
    .await
    .expect("simulate K08");
    drop(db);

    // Row 9: activation already happened, in an earlier run.
    write_key_file_for_test(&key_path, &new_key);

    (db_path, key_path, new_key)
}

#[tokio::test]
async fn open_checked_completes_a_pending_rotation_and_cleans_the_workspace_before_returning() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (db_path, key_path, new_key) = seed_row9(dir.path()).await;
    let paths = crate::rotate::preconditions::RotationPaths::for_key_file(&key_path);

    let (db, _origin, _lock) =
        crate::database::open_checked(&db_path, &key_path, PreMigrationBackup::Declined)
            .await
            .expect("open_checked must complete the rotation and succeed");

    // By the time `open_checked` returned `Ok`, K05 had already run:
    // the row is `complete`.
    let row = db
        .with_conn(sui_id_store::master_key_rotation::read_within_tx)
        .await
        .expect("with_conn")
        .expect("a row exists");
    assert_eq!(
        row.state,
        sui_id_store::master_key_rotation::RotationState::Complete
    );
    assert!(row.completed_at.is_some());

    // And the leftover workspace was cleaned, the same run -- but RFC
    // 100 `0.94.0` P2 §3 point 11: a *completed* rotation's backup
    // stays, under its bound disposition, until P3. (Rows 2-7's own
    // cleanup, for an *abandoned* rotation, still removes it --
    // `rotate::recovery::tests` covers that side.)
    assert!(!paths.workspace.exists());
    assert!(!paths.next.exists());
    assert!(
        paths.backup.exists(),
        "a completed rotation's backup must survive cleanup, under its disposition"
    );

    // The returned database is the real thing: it opens with the new
    // key, which is what `open_checked`'s own caller gets to use.
    assert_eq!(
        sui_id_store::crypto::fingerprint(db.key()),
        sui_id_store::crypto::fingerprint(&new_key)
    );
}
