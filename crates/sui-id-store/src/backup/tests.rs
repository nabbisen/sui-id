#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;

// (moved from backup.rs by RFC 075)
mod tests_inner {
    use super::tar::{read_tar, write_tar_entry, write_tar_terminator};
    use super::*;
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const ENTRY_MANIFEST: &str = "MANIFEST.json";
    const ENTRY_DB: &str = "sui-id.sqlite";
    const ENTRY_KEY: &str = "sui-id.key";

    /// The three values the store API takes, standing in for the binary's
    /// `Config` that these tests built before the move into this crate.
    struct Target {
        db: PathBuf,
        key: PathBuf,
        issuer: String,
    }

    fn target(db: PathBuf, key: PathBuf, issuer: &str) -> Target {
        Target {
            db,
            key,
            issuer: issuer.to_owned(),
        }
    }

    fn backup(t: &Target, dest: &Path, opts: &BackupOptions) -> Result<(), BackupError> {
        run_backup(&t.db, &t.key, &t.issuer, dest, opts)
    }

    fn restore(t: &Target, src: &Path, opts: &RestoreOptions) -> Result<(), BackupError> {
        run_restore(&t.db, &t.key, src, opts)
    }

    fn fake_files(dir: &Path) -> (PathBuf, PathBuf) {
        let db = dir.join("sui-id.sqlite");
        let key = dir.join("sui-id.key");
        // For the round-trip test we don't need a real SQLite file; the
        // tar pipe doesn't care. The end-to-end backup() function does
        // need a real SQLite file, exercised separately.
        std::fs::write(&db, b"sqlite-fake-bytes").unwrap();
        std::fs::write(&key, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").unwrap();
        (db, key)
    }

    /// K28 (backup-into-store equivalence record): the writer refuses a
    /// name that does not fit the 100-byte ustar name field. Every name
    /// the public path writes is a constant, so this check is reachable
    /// only here.
    #[test]
    fn k28_tar_writer_refuses_name_of_100_bytes_or_more() {
        let mut buf = Vec::new();
        let name = "n".repeat(100);
        let err = write_tar_entry(&mut buf, &name, b"x").unwrap_err();
        assert!(
            format!("{err:#}").contains("tar entry name too long"),
            "{err:#}"
        );
        assert!(buf.is_empty(), "nothing written for a refused entry");
        write_tar_entry(&mut buf, &"n".repeat(99), b"x").expect("99 bytes fits");
    }

    #[test]
    fn tar_round_trip_two_entries() {
        let tmp = TempDir::new().expect("tempdir");
        let dest = tmp.path().join("out.tar");
        {
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&dest)
                .unwrap();
            write_tar_entry(&mut f, "a", b"hello").unwrap();
            write_tar_entry(&mut f, "b", b"world!!!").unwrap();
            write_tar_terminator(&mut f).unwrap();
        }
        let mut bytes = Vec::new();
        File::open(&dest).unwrap().read_to_end(&mut bytes).unwrap();
        let entries = read_tar(&bytes).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "a");
        assert_eq!(entries[0].1, b"hello");
        assert_eq!(entries[1].0, "b");
        assert_eq!(entries[1].1, b"world!!!");
    }

    #[test]
    fn restore_refuses_to_overwrite_without_force() {
        let tmp = TempDir::new().expect("tempdir");
        let (db, key) = fake_files(tmp.path());
        let cfg = target(db.clone(), key.clone(), "https://x");
        let backup_path = tmp.path().join("backup.tar");
        // Build a backup tar by hand — bypass run_backup since fake_files
        // didn't create a real SQLite file.
        {
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&backup_path)
                .unwrap();
            write_tar_entry(&mut f, ENTRY_DB, b"db-bytes").unwrap();
            write_tar_entry(&mut f, ENTRY_KEY, b"key-bytes").unwrap();
            write_tar_terminator(&mut f).unwrap();
        }
        // db & key already exist, so restore must refuse.
        let r = restore(
            &cfg,
            &backup_path,
            &RestoreOptions {
                force: false,
                passphrase: None,
            },
        );
        assert!(r.is_err(), "expected refusal to overwrite without --force");
        // With --force, it succeeds.
        restore(
            &cfg,
            &backup_path,
            &RestoreOptions {
                force: true,
                passphrase: None,
            },
        )
        .expect("force restore");
        assert_eq!(std::fs::read(&db).unwrap(), b"db-bytes");
        assert_eq!(std::fs::read(&key).unwrap(), b"key-bytes");
    }

    #[test]
    fn restore_creates_files_when_destinations_dont_exist() {
        let tmp = TempDir::new().expect("tempdir");
        let cfg = target(
            tmp.path().join("subdir").join("sui-id.sqlite"),
            tmp.path().join("subdir").join("sui-id.key"),
            "https://x",
        );
        let backup_path = tmp.path().join("backup.tar");
        {
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&backup_path)
                .unwrap();
            write_tar_entry(&mut f, ENTRY_DB, b"db-bytes").unwrap();
            write_tar_entry(&mut f, ENTRY_KEY, b"key-bytes").unwrap();
            write_tar_terminator(&mut f).unwrap();
        }
        restore(
            &cfg,
            &backup_path,
            &RestoreOptions {
                force: false,
                passphrase: None,
            },
        )
        .expect("restore");
        assert!(cfg.db.exists());
        assert!(cfg.key.exists());
    }

    #[test]
    fn run_backup_round_trip_via_real_sqlite() {
        let tmp = TempDir::new().expect("tempdir");
        let db = tmp.path().join("source.sqlite");
        let key = tmp.path().join("source.key");
        // Real SQLite file.
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(
                "CREATE TABLE t (k TEXT PRIMARY KEY); INSERT INTO t VALUES ('hello');",
            )
            .unwrap();
        }
        std::fs::write(&key, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").unwrap();
        let cfg = target(db.clone(), key.clone(), "https://x");
        let dest = tmp.path().join("backup.tar");
        backup(&cfg, &dest, &BackupOptions::default()).expect("backup");
        assert!(dest.exists());
        // Verify mode 0600.
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        // Restore into a fresh location and check the SQLite file is queryable.
        let cfg2 = target(
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
            &cfg.issuer,
        );
        restore(
            &cfg2,
            &dest,
            &RestoreOptions {
                force: false,
                passphrase: None,
            },
        )
        .expect("restore");
        let conn = rusqlite::Connection::open(&cfg2.db).unwrap();
        let v: String = conn
            .query_row("SELECT k FROM t LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "hello");
        // Key file restored byte-for-byte.
        let restored_key = std::fs::read(&cfg2.key).unwrap();
        assert_eq!(
            restored_key,
            b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        );
    }

    // ---------- v0.13.0 additions: encryption, manifest, verify ----------

    fn make_real_sqlite_db(dir: &Path) -> (PathBuf, PathBuf) {
        let db = dir.join("sui-id.sqlite");
        let key = dir.join("sui-id.key");
        // Real SQLite with a sui_meta row so the manifest can read
        // schema_version. Mimics the post-migration state.
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE sui_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL); \
             INSERT INTO sui_meta(key, value) VALUES('schema_version', '5'); \
             CREATE TABLE t(k TEXT); INSERT INTO t VALUES('hello');",
        )
        .unwrap();
        std::fs::write(&key, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").unwrap();
        (db, key)
    }

    fn fake_cfg(_dir: &Path, db: PathBuf, key: PathBuf) -> Target {
        target(db, key, "https://idp.test")
    }

    #[test]
    fn manifest_present_in_plain_backup() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar");
        backup(&cfg, &dest, &BackupOptions::default()).unwrap();

        let bytes = std::fs::read(&dest).unwrap();
        let entries = read_tar(&bytes).unwrap();
        let m_bytes = entries
            .iter()
            .find(|(n, _)| n == ENTRY_MANIFEST)
            .map(|(_, b)| b)
            .expect("MANIFEST.json present");
        let m: Manifest = serde_json::from_slice(m_bytes).unwrap();
        assert_eq!(m.format_version, FORMAT_VERSION);
        assert_eq!(m.schema_version, 5);
        assert_eq!(m.issuer, "https://idp.test");
        assert!(!m.created_at.is_empty());
    }

    #[test]
    fn encrypted_backup_round_trips_with_correct_passphrase() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar.enc");
        backup(
            &cfg,
            &dest,
            &BackupOptions {
                passphrase: Some("hunter2-correct-horse".into()),
            },
        )
        .unwrap();

        // Restore into a fresh location.
        let cfg2 = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        restore(
            &cfg2,
            &dest,
            &RestoreOptions {
                force: false,
                passphrase: Some("hunter2-correct-horse".into()),
            },
        )
        .unwrap();
        let conn = rusqlite::Connection::open(&cfg2.db).unwrap();
        let v: String = conn.query_row("SELECT k FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(v, "hello");
    }

    #[test]
    fn encrypted_backup_rejects_wrong_passphrase() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar.enc");
        backup(
            &cfg,
            &dest,
            &BackupOptions {
                passphrase: Some("right-pass".into()),
            },
        )
        .unwrap();

        let cfg2 = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        let r = restore(
            &cfg2,
            &dest,
            &RestoreOptions {
                force: false,
                passphrase: Some("wrong-pass".into()),
            },
        );
        assert!(r.is_err());
        // Failure should not have written the destination files.
        assert!(!cfg2.db.exists());
        assert!(!cfg2.key.exists());
    }

    #[test]
    fn restore_of_encrypted_without_passphrase_errors() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar.enc");
        backup(
            &cfg,
            &dest,
            &BackupOptions {
                passphrase: Some("p".into()),
            },
        )
        .unwrap();

        let cfg2 = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        let r = restore(
            &cfg2,
            &dest,
            &RestoreOptions {
                force: false,
                passphrase: None,
            },
        );
        let msg = format!("{}", r.unwrap_err());
        assert!(
            msg.contains("encrypted"),
            "error should mention encryption; got: {msg}"
        );
    }

    #[test]
    fn restore_of_plain_with_passphrase_errors() {
        // A plain tarball + --decrypt is almost certainly an
        // operator misuse. Refuse rather than silently ignore.
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar");
        backup(&cfg, &dest, &BackupOptions::default()).unwrap();

        let cfg2 = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        let r = restore(
            &cfg2,
            &dest,
            &RestoreOptions {
                force: false,
                passphrase: Some("anything".into()),
            },
        );
        assert!(r.is_err());
    }

    #[test]
    fn verify_reports_manifest_and_runs_integrity_check() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar");
        backup(&cfg, &dest, &BackupOptions::default()).unwrap();
        let report = run_verify(&dest, None).expect("verify");
        assert!(!report.encrypted);
        assert_eq!(report.manifest.schema_version, 5);
        assert_eq!(report.manifest.format_version, FORMAT_VERSION);
        assert!(report.key_present);
        assert!(report.db_bytes > 0);
    }

    #[test]
    fn verify_works_on_encrypted_backup_with_passphrase() {
        let tmp = TempDir::new().unwrap();
        let (db, key) = make_real_sqlite_db(tmp.path());
        let cfg = fake_cfg(tmp.path(), db, key);
        let dest = tmp.path().join("out.tar.enc");
        backup(
            &cfg,
            &dest,
            &BackupOptions {
                passphrase: Some("p".into()),
            },
        )
        .unwrap();
        let report = run_verify(&dest, Some("p")).expect("verify");
        assert!(report.encrypted);
        assert_eq!(report.manifest.schema_version, 5);
    }

    #[test]
    fn restore_refuses_backup_with_too_new_schema_version() {
        // Hand-craft a manifest with a future schema_version.
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("out.tar");
        let manifest = Manifest {
            format_version: FORMAT_VERSION,
            sui_id_version: "future".into(),
            schema_version: 9999,
            created_at: "2099-01-01T00:00:00Z".into(),
            hostname: "x".into(),
            issuer: "x".into(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();

        // A real SQLite file so restore doesn't trip on integrity.
        let inner_db = tmp.path().join("inner.sqlite");
        let conn = rusqlite::Connection::open(&inner_db).unwrap();
        conn.execute_batch("CREATE TABLE t(k TEXT)").unwrap();
        let db_bytes = std::fs::read(&inner_db).unwrap();

        let mut tar_buf = Vec::new();
        write_tar_entry(&mut tar_buf, ENTRY_MANIFEST, &manifest_bytes).unwrap();
        write_tar_entry(&mut tar_buf, ENTRY_DB, &db_bytes).unwrap();
        write_tar_entry(&mut tar_buf, ENTRY_KEY, b"key").unwrap();
        write_tar_terminator(&mut tar_buf).unwrap();
        std::fs::write(&dest, &tar_buf).unwrap();

        let cfg = fake_cfg(
            tmp.path(),
            tmp.path().join("restored.sqlite"),
            tmp.path().join("restored.key"),
        );
        let r = restore(
            &cfg,
            &dest,
            &RestoreOptions {
                force: false,
                passphrase: None,
            },
        );
        let msg = format!("{}", r.unwrap_err());
        assert!(msg.contains("schema_version"), "got: {msg}");
    }
}
