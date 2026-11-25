use super::*;
use crate::key_adapter::Real;
use crate::rotate::preconditions::RotationPaths;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use sui_id_store::Database;
use sui_id_store::crypto::{self, MasterKey};
use sui_id_store::master_key_check::ensure_sentinel_written;
use sui_id_store::master_key_rotation::{RotationDisposition, RotationRecord};

fn dup_key(key: &MasterKey) -> MasterKey {
    let mut bytes = *key.as_bytes();
    let copy = MasterKey::from_bytes(bytes);
    zeroize::Zeroize::zeroize(&mut bytes);
    copy
}

fn write_key_at(path: &std::path::Path, key: &MasterKey, mode: u32) {
    std::fs::write(path, format!("{}\n", key.to_base64())).expect("write key");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

fn remove_after_verified() -> RotationDisposition {
    RotationDisposition::RemoveAfterVerified {
        verification_reference: "verified-by-test".into(),
    }
}

/// A completed rotation, with its backup still kept at
/// `paths.backup`, holding `old_key`'s own bytes -- exactly the state
/// `admin master-key backup remove` is meant to act on. The active key
/// file already holds `new_key` (activation already happened).
struct Fixture {
    _dir: tempfile::TempDir,
    key_path: PathBuf,
    db_path: PathBuf,
    paths: RotationPaths,
    old_key: MasterKey,
    new_key: MasterKey,
    rotation_id: String,
}

impl Fixture {
    async fn completed(disposition: RotationDisposition) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let old_key = MasterKey::generate();
        let new_key = MasterKey::generate();
        let key_path = dir.path().join("t.key");
        let db_path = dir.path().join("t.sqlite");
        let paths = RotationPaths::for_key_file(&key_path);
        let rotation_id = "r-backup-test".to_owned();

        let db = Database::open(&db_path, dup_key(&old_key)).expect("open under old key");
        ensure_sentinel_written(&db, db.key())
            .await
            .expect("sentinel");
        sui_id_store::commands::reseal_master_key_for_rotation(
            &db,
            &new_key,
            RotationRecord {
                rotation_id: rotation_id.clone(),
                old_fingerprint: crypto::fingerprint(&old_key),
                backup_filename: paths
                    .backup
                    .file_name()
                    .and_then(|n| n.to_str())
                    .expect("utf8")
                    .to_owned(),
                disposition,
            },
        )
        .await
        .expect("K08");
        drop(db);

        write_key_at(&key_path, &new_key, 0o600);
        write_key_at(&paths.backup, &old_key, 0o600);

        let db_new = Database::open(&db_path, dup_key(&new_key)).expect("open under new key");
        sui_id_store::commands::complete_master_key_rotation(&db_new, rotation_id.clone())
            .await
            .expect("K05")
            .expect("something changed");
        drop(db_new);

        Self {
            _dir: dir,
            key_path,
            db_path,
            paths,
            old_key,
            new_key,
            rotation_id,
        }
    }

    fn db(&self) -> Database {
        Database::open(&self.db_path, dup_key(&self.new_key)).expect("open")
    }
}

#[tokio::test]
async fn remove_reports_nothing_to_remove_when_the_file_is_absent() {
    let f = Fixture::completed(remove_after_verified()).await;
    std::fs::remove_file(&f.paths.backup).expect("remove the backup ahead of the call");
    let db = f.db();

    let outcome = remove(&Real, &db, &f.key_path, None, "0:root".into())
        .await
        .expect("remove");
    assert!(matches!(outcome, RemovalOutcome::NothingToRemove));
}

#[tokio::test]
async fn remove_succeeds_for_remove_after_verified_and_unlinks_the_file() {
    let f = Fixture::completed(remove_after_verified()).await;
    let db = f.db();

    let outcome = remove(&Real, &db, &f.key_path, None, "1000:alice".into())
        .await
        .expect("remove");
    match outcome {
        RemovalOutcome::Removed {
            rotation_id,
            verification_reference,
        } => {
            assert_eq!(rotation_id, f.rotation_id);
            assert_eq!(verification_reference, "verified-by-test");
        }
        RemovalOutcome::NothingToRemove => panic!("expected Removed"),
    }
    assert!(!f.paths.backup.exists(), "the backup file is gone");

    let row = db
        .with_conn(sui_id_store::master_key_rotation::read_within_tx)
        .await
        .expect("read")
        .expect("row");
    assert!(row.backup_removed_at.is_some());
}

#[tokio::test]
async fn remove_requires_an_override_for_retain_until_and_changes_nothing() {
    let f = Fixture::completed(RotationDisposition::RetainUntil {
        review_at: chrono::Utc::now() + chrono::TimeDelta::days(90),
        custody_reference: "ops-team".into(),
    })
    .await;
    let db = f.db();

    let err = remove(&Real, &db, &f.key_path, None, "0:root".into())
        .await
        .expect_err("must refuse without an override");
    assert!(
        format!("{err:#}").contains("verification-reference"),
        "{err:#}"
    );
    assert!(f.paths.backup.exists(), "the backup file is untouched");
}

#[tokio::test]
async fn remove_succeeds_for_retain_until_with_an_override() {
    let f = Fixture::completed(RotationDisposition::RetainUntil {
        review_at: chrono::Utc::now() + chrono::TimeDelta::days(90),
        custody_reference: "ops-team".into(),
    })
    .await;
    let db = f.db();

    let outcome = remove(
        &Real,
        &db,
        &f.key_path,
        Some("checked by hand, 2026-11-25".into()),
        "0:root".into(),
    )
    .await
    .expect("remove");
    match outcome {
        RemovalOutcome::Removed {
            verification_reference,
            ..
        } => assert_eq!(verification_reference, "checked by hand, 2026-11-25"),
        RemovalOutcome::NothingToRemove => panic!("expected Removed"),
    }
    assert!(!f.paths.backup.exists());
}

#[tokio::test]
async fn remove_refuses_a_symlink_and_changes_nothing() {
    let f = Fixture::completed(remove_after_verified()).await;
    let real_target = f._dir.path().join("elsewhere.key");
    std::fs::rename(&f.paths.backup, &real_target).expect("move aside");
    std::os::unix::fs::symlink(&real_target, &f.paths.backup).expect("symlink");
    let db = f.db();

    let err = remove(&Real, &db, &f.key_path, None, "0:root".into())
        .await
        .expect_err("must refuse a symlink");
    assert!(format!("{err:#}").contains("symlink"), "{err:#}");
    assert!(f.paths.backup.exists());
    let row = db
        .with_conn(sui_id_store::master_key_rotation::read_within_tx)
        .await
        .expect("read")
        .expect("row");
    assert!(row.backup_removed_at.is_none());
}

#[tokio::test]
async fn remove_refuses_a_fingerprint_mismatch_and_changes_nothing() {
    let f = Fixture::completed(remove_after_verified()).await;
    write_key_at(&f.paths.backup, &MasterKey::generate(), 0o600);
    let db = f.db();

    let err = remove(&Real, &db, &f.key_path, None, "0:root".into())
        .await
        .expect_err("must refuse a fingerprint mismatch");
    assert!(format!("{err:#}").contains("fingerprint"), "{err:#}");
    assert!(f.paths.backup.exists());
}

#[tokio::test]
async fn remove_refuses_when_no_row_names_the_backup() {
    let f = Fixture::completed(remove_after_verified()).await;
    let db = f.db();
    // A backup with no row to explain it -- tampering, or a file left
    // by something else entirely. Simulated here by wiping the row
    // directly (the real file is untouched).
    db.with_conn(|conn| {
        conn.execute("DELETE FROM master_key_rotation WHERE id = 1", [])?;
        Ok::<_, sui_id_store::StoreError>(())
    })
    .await
    .expect("delete the row");

    let err = remove(&Real, &db, &f.key_path, None, "0:root".into())
        .await
        .expect_err("must refuse with no row");
    assert!(
        format!("{err:#}").contains("no master-key rotation row"),
        "{err:#}"
    );
    assert!(f.paths.backup.exists());
}

/// RFC 100 `0.94.0` P3 §3: a kept backup is never a startup key
/// candidate. `keyring` reads only the configured key file -- proved
/// here directly against a database sealed under the *backup's* key,
/// with the active key wrong: it must still refuse, never silently
/// open under the backup.
#[tokio::test]
async fn a_kept_backup_is_never_tried_as_a_startup_key_candidate() {
    let f = Fixture::completed(remove_after_verified()).await;
    // The configured key file now holds a key that opens nothing (not
    // the backup's old key, not the real active new key).
    write_key_at(&f.key_path, &MasterKey::generate(), 0o600);

    let resolved = crate::keyring::read_existing_impl(&Real, &f.key_path).expect("read");
    let resolved_key = resolved
        .expect("a key was read from the configured file")
        .key;
    // It is neither the backup's key nor the real active one -- proof
    // that `read_existing_impl` never fell back to scanning siblings
    // (the backup among them) for something that would open the
    // database instead.
    assert_ne!(
        crypto::fingerprint(&resolved_key),
        crypto::fingerprint(&f.old_key)
    );
    assert_ne!(
        crypto::fingerprint(&resolved_key),
        crypto::fingerprint(&f.new_key)
    );

    let conn =
        sui_id_store::migrations::open_preview_connection(&f.db_path).expect("preview connection");
    let inspection = sui_id_store::master_key_check::inspect(&conn).expect("inspect");
    let sui_id_store::master_key_check::Inspection::Sentinel(sentinel) = inspection else {
        panic!("expected a sentinel");
    };
    assert!(
        !sui_id_store::master_key_check::sentinel_opens(&resolved_key, &sentinel.sealed),
        "the wrong configured key must not open the sentinel"
    );
}

/// RFC 100 `0.94.0` P3 §3, the dispatch's own exact scenario: a
/// database sealed under what the kept backup *also* holds, with the
/// configured key file wrong. If `resolve_checked` ever fell back to
/// trying sibling files, this is the one case that would make it
/// *succeed* -- wrongly, since the backup is not the configured key.
/// It must still refuse.
#[tokio::test]
async fn resolve_checked_refuses_even_though_the_kept_backup_would_open_the_sentinel() {
    let dir = tempfile::tempdir().expect("tempdir");
    let sentinel_key = MasterKey::generate();
    let wrong_key = MasterKey::generate();
    let key_path = dir.path().join("t.key");
    let db_path = dir.path().join("t.sqlite");
    let paths = RotationPaths::for_key_file(&key_path);

    let db = Database::open(&db_path, dup_key(&sentinel_key)).expect("open");
    ensure_sentinel_written(&db, db.key())
        .await
        .expect("sentinel");
    drop(db);

    write_key_at(&key_path, &wrong_key, 0o600);
    write_key_at(&paths.backup, &sentinel_key, 0o600);

    let result = crate::keyring::resolve_checked_impl(&Real, &key_path, &db_path);
    let err = match result {
        Ok(_) => panic!("must refuse: the configured key is wrong, the backup is not a candidate"),
        Err(e) => e,
    };
    assert!(
        format!("{err:#}").contains("does not open this database"),
        "{err:#}"
    );
}

/// RFC 100 `0.94.0` P3 §3: the finding appears once `now` has passed
/// `review_at`, names the file, the date, and the custody reference,
/// and never fires for `RemoveAfterVerified` (which has no
/// `review_at` to be overdue against).
#[tokio::test]
async fn overdue_finding_line_fires_only_past_review_at_for_a_retain_until_backup() {
    let review_at = chrono::Utc::now() + chrono::TimeDelta::days(90);
    let f = Fixture::completed(RotationDisposition::RetainUntil {
        review_at,
        custody_reference: "ops-team".into(),
    })
    .await;

    assert!(
        overdue_finding_line_from_paths(
            &f.key_path,
            &f.db_path,
            review_at - chrono::TimeDelta::days(1)
        )
        .is_none(),
        "not yet overdue"
    );

    let line = overdue_finding_line_from_paths(
        &f.key_path,
        &f.db_path,
        review_at + chrono::TimeDelta::days(1),
    )
    .expect("overdue");
    assert!(
        line.contains(&f.paths.backup.display().to_string()),
        "{line}"
    );
    assert!(line.contains("ops-team"), "{line}");
    assert!(line.contains("backup remove"), "{line}");
    assert!(line.contains("backup renew"), "{line}");
}

#[tokio::test]
async fn overdue_finding_line_never_fires_for_remove_after_verified() {
    let f = Fixture::completed(remove_after_verified()).await;
    let far_future = chrono::Utc::now() + chrono::TimeDelta::days(3650);
    assert!(overdue_finding_line_from_paths(&f.key_path, &f.db_path, far_future).is_none());
}

/// Settled (removed) means the finding stops, even past `review_at`.
#[tokio::test]
async fn overdue_finding_line_stops_once_the_backup_is_removed() {
    let review_at = chrono::Utc::now() - chrono::TimeDelta::days(1);
    let f = Fixture::completed(RotationDisposition::RetainUntil {
        review_at,
        custody_reference: "ops-team".into(),
    })
    .await;
    let db = f.db();
    remove(
        &Real,
        &db,
        &f.key_path,
        Some("checked by hand".into()),
        "0:root".into(),
    )
    .await
    .expect("remove");

    assert!(
        overdue_finding_line_from_paths(&f.key_path, &f.db_path, chrono::Utc::now()).is_none(),
        "settled by removal, so no finding"
    );
}

/// Settled (renewed) means the finding stops until the new date.
#[tokio::test]
async fn overdue_finding_line_stops_once_renewed_and_resumes_after_the_new_date() {
    let review_at = chrono::Utc::now() - chrono::TimeDelta::days(1);
    let f = Fixture::completed(RotationDisposition::RetainUntil {
        review_at,
        custody_reference: "ops-team".into(),
    })
    .await;
    let db = f.db();
    let new_review_at = chrono::Utc::now() + chrono::TimeDelta::days(30);
    sui_id_store::commands::renew_master_key_backup(&db, new_review_at, "ops-team-2".into())
        .await
        .expect("renew");

    assert!(
        overdue_finding_line_from_paths(&f.key_path, &f.db_path, chrono::Utc::now()).is_none(),
        "settled by renewal, so no finding until the new date"
    );
    assert!(
        overdue_finding_line_from_paths(
            &f.key_path,
            &f.db_path,
            new_review_at + chrono::TimeDelta::days(1)
        )
        .is_some()
    );
}

/// `overdue_finding_line_from_db` is the one a server start actually
/// calls, with its own `Database` connection already open and staying
/// open throughout -- exactly the case `overdue_finding_line_from_paths`
/// cannot be reused for (its `open_preview_connection` call can take
/// the `immutable=1` fast path and never see a write this same
/// process's own already-open connection made moments before).
#[tokio::test]
async fn overdue_finding_line_from_db_sees_an_overdue_backup_while_the_database_stays_open() {
    let review_at = chrono::Utc::now() - chrono::TimeDelta::days(1);
    let f = Fixture::completed(RotationDisposition::RetainUntil {
        review_at,
        custody_reference: "ops-team".into(),
    })
    .await;
    let db = f.db();

    let line = overdue_finding_line_from_db(&f.key_path, &db, chrono::Utc::now())
        .await
        .expect("overdue, with the database still open");
    assert!(line.contains("ops-team"), "{line}");

    // Still open after the call -- the same handle a server start
    // keeps using for the rest of its life.
    db.with_conn(sui_id_store::master_key_rotation::read_within_tx)
        .await
        .expect("the database is still usable");
}
