//! RFC 142 stage 5 — the automatic pre-migration backup (§8 of
//! `rfcs/handoffs/142-.../stage5-the-backup-before-the-migration-2026-11-08.md`).
//!
//! Subprocess tests (the real binary) cover what only the full CLI dispatch
//! can prove: the exact operator-facing lines, the directory's file set
//! before and after, and that a database built to a real intermediate
//! version through `migrations::run_up_to` (reachable here only via
//! `sui-id-store`'s `test-support` feature) actually reaches head. In-
//! process calls to `sui_id::database::open` cover the two cases that must
//! not need a filled disk or a filesystem-wide fault: the free-space
//! refusal (injected through `set_free_space_override_for_test`) and the
//! snapshot-write failure (a directory this process cannot write to).

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use sui_id::database::{PreMigrationBackup, set_free_space_override_for_test};
use sui_id_store::backup::FreeSpaceCheck;
use sui_id_store::crypto::MasterKey;

const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

struct Home {
    dir: tempfile::TempDir,
}

impl Home {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }
    fn db(&self) -> PathBuf {
        self.dir.path().join("t.sqlite")
    }
    fn key(&self) -> PathBuf {
        self.dir.path().join("t.key")
    }
    fn config(&self) -> PathBuf {
        let sample = Command::new(BIN)
            .arg("--print-sample-config")
            .output()
            .expect("sample");
        let text = String::from_utf8(sample.stdout)
            .expect("utf8")
            .replace("./sui-id.sqlite", &self.db().display().to_string())
            .replace("./sui-id.key", &self.key().display().to_string())
            .replace("127.0.0.1:8801", "127.0.0.1:18813");
        let path = self.dir.path().join("cfg.toml");
        std::fs::write(&path, text).expect("config");
        path
    }
    /// Build the database to `version` through the real migration runner
    /// (reachable here only because `sui-id-store`'s `test-support` feature
    /// is enabled for this crate's dev-dependency), then seed one row per
    /// seeded table so there is something whose count the snapshot must
    /// preserve. Rollback-journal mode (the default); nothing sets WAL.
    fn build_to_version(&self, version: i32) {
        let mut conn = rusqlite::Connection::open(self.db()).expect("raw");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        sui_id_store::migrations::run_up_to(&mut conn, version).expect("run_up_to");
        conn.execute_batch(
            "INSERT INTO users(id, username, is_admin, is_disabled, is_deleted, created_at, \
                               updated_at, user_uuid, failed_login_count) \
             VALUES('u1','u1',0,0,0,datetime('now'),datetime('now'),'11111111-1111-1111-1111-111111111111',0); \
             INSERT INTO credentials(user_id, password_hash, updated_at) \
             VALUES('u1','hash',datetime('now'));",
        )
        .expect("seed a row");
    }
    /// Let the binary create and fully migrate a fresh database (fails with
    /// "user not found", which is the point: open and migrate succeeded).
    fn create(&self) {
        let out = run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &self.config(),
        );
        assert_eq!(
            out.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(self.db().exists());
    }
    fn stored(&self) -> i64 {
        rusqlite::Connection::open(self.db())
            .expect("raw")
            .query_row(
                "SELECT value FROM sui_meta WHERE key='schema_version'",
                [],
                |r| r.get::<_, String>(0),
            )
            .expect("version")
            .parse()
            .expect("parse version")
    }
    fn listing(&self) -> BTreeSet<String> {
        std::fs::read_dir(self.dir.path())
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect()
    }
    fn snapshot_file(&self) -> Option<PathBuf> {
        std::fs::read_dir(self.dir.path())
            .expect("read dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("pre-upgrade-schema"))
            })
    }
}

fn run(args: &[&str], config: &Path) -> Output {
    Command::new(BIN)
        .args(args)
        .arg(config)
        .output()
        .expect("run")
}

fn unlock_nobody(h: &Home, extra: &[&str]) -> Output {
    let mut args = vec!["admin", "unlock-user", "--username", "nobody"];
    args.extend_from_slice(extra);
    args.push("--config");
    run(&args, &h.config())
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

const MAX: i32 = sui_id_store::migrations::MAX_SCHEMA_VERSION;

// ───────────────────────── §8.1: it happens ───────────────────────────────

#[test]
fn it_happens_exactly_one_valid_snapshot_at_the_pre_migration_version_mode_0600() {
    let h = Home::new();
    h.build_to_version(44);
    let before_user_count: i64 = rusqlite::Connection::open(h.db())
        .expect("raw")
        .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
        .expect("count");
    assert_eq!(before_user_count, 1, "sanity: one seeded row");

    let out = unlock_nobody(&h, &[]);

    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("looking up user") || stderr(&out).contains("not found"),
        "migration must have succeeded for the command to reach the user lookup: {}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains(&format!("before migrating schema 44 → {MAX}")),
        "{}",
        stderr(&out)
    );

    let snapshot = h.snapshot_file().expect("exactly one snapshot file");
    let file_name = snapshot.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        file_name.starts_with(&format!("t.sqlite.pre-upgrade-schema-44-to-{MAX}-")),
        "{file_name}"
    );
    assert!(file_name.ends_with(".sqlite"), "{file_name}");

    let mode = std::fs::metadata(&snapshot).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the snapshot must be mode 0600");

    let snap_conn = rusqlite::Connection::open(&snapshot).expect("open snapshot");
    let snap_version: i64 = snap_conn
        .query_row(
            "SELECT value FROM sui_meta WHERE key='schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .expect("snapshot version")
        .parse()
        .expect("parse");
    assert_eq!(
        snap_version, 44,
        "the snapshot is at the pre-migration version"
    );
    let snap_user_count: i64 = snap_conn
        .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
        .expect("count");
    assert_eq!(
        snap_user_count, before_user_count,
        "row counts preserved in the snapshot"
    );

    assert_eq!(h.stored(), i64::from(MAX), "the live database reached head");
}

// ───────────────────────── §8.2: skipped when nothing is pending ─────────

#[test]
fn skipped_at_head() {
    let h = Home::new();
    h.create();
    let before = h.listing();
    let out = unlock_nobody(&h, &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(h.snapshot_file().is_none());
    assert_eq!(
        h.listing(),
        before,
        "no file beyond what the first create() made"
    );
}

#[test]
fn skipped_no_file() {
    let h = Home::new();
    let _ = h.config();
    let before = h.listing();
    let out = unlock_nobody(&h, &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(h.snapshot_file().is_none());
    // A fresh start creates the database file *and* generates the key file
    // (neither existed yet) -- just no pre-migration snapshot beside them.
    let mut expected = before.clone();
    expected.insert("t.sqlite".to_string());
    expected.insert("t.key".to_string());
    assert_eq!(
        h.listing(),
        expected,
        "only the new database and key files appear"
    );
}

#[test]
fn skipped_fresh_empty_file() {
    let h = Home::new();
    let _ = h.config();
    std::fs::write(h.db(), []).expect("empty file");
    let before = h.listing();
    let out = unlock_nobody(&h, &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(h.snapshot_file().is_none());
    let mut expected = before.clone();
    expected.insert("t.key".to_string());
    assert_eq!(h.listing(), expected, "only the new key file appears");
}

// ───────────────────────── §8.3: --no-backup ──────────────────────────────

#[test]
fn no_backup_flag_skips_the_snapshot_and_says_so() {
    let h = Home::new();
    h.build_to_version(44);
    let out = unlock_nobody(&h, &["--no-backup"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains(&format!(
            "sui-id: --no-backup: migrating schema 44 → {MAX} without a backup."
        )),
        "{}",
        stderr(&out)
    );
    assert!(h.snapshot_file().is_none(), "no snapshot with --no-backup");
    assert_eq!(h.stored(), i64::from(MAX), "the migration still ran");
}

// ───────────────────────── §8.4/§8.6/§8.7: in-process, open() directly ───

fn seeded_at(dir: &Path, version: i32) -> PathBuf {
    let path = dir.join("t.sqlite");
    let mut conn = rusqlite::Connection::open(&path).expect("raw");
    conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
    sui_id_store::migrations::run_up_to(&mut conn, version).expect("run_up_to");
    drop(conn);
    path
}

#[test]
fn free_space_short_refuses_with_exit_1_shape_no_snapshot_database_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = seeded_at(dir.path(), 44);
    let before_bytes = std::fs::read(&path).expect("read before");
    let before_listing: BTreeSet<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();

    set_free_space_override_for_test(Some(FreeSpaceCheck {
        needed_bytes: 999_999_999,
        available_bytes: Some(1),
    }));
    let result = sui_id::database::open(&path, MasterKey::generate(), PreMigrationBackup::Take);
    set_free_space_override_for_test(None);

    let err = match result {
        Ok(_) => panic!("must refuse"),
        Err(e) => e,
    };
    assert!(
        err.downcast_ref::<sui_id::database::DatabaseRefused>()
            .is_none(),
        "this is not a schema refusal"
    );
    let refused = err
        .downcast_ref::<sui_id::database::PreMigrationBackupRefused>()
        .expect("PreMigrationBackupRefused");
    assert!(
        refused.line().starts_with("sui-id: refusing to migrate:"),
        "{}",
        refused.line()
    );
    assert!(
        refused
            .line()
            .contains("free beside it (room for the backup and for the migration after it)"),
        "{}",
        refused.line()
    );
    assert!(
        refused.line().contains("Nothing was changed"),
        "{}",
        refused.line()
    );

    let after_listing: BTreeSet<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(after_listing, before_listing, "no snapshot file created");
    assert_eq!(
        std::fs::read(&path).expect("read after"),
        before_bytes,
        "database untouched"
    );
}

#[test]
fn snapshot_write_failure_refuses_removes_the_partial_file_database_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = seeded_at(dir.path(), 44);
    let before_bytes = std::fs::read(&path).expect("read before");

    // Make the directory beside the database unwritable, so creating the
    // snapshot file fails with a permission error -- no disk-filling, no
    // seam, just an ordinary fs fault.
    let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
    perms.set_mode(0o500);
    std::fs::set_permissions(dir.path(), perms).expect("chmod 0500");

    // A privileged user (root, CAP_DAC_OVERRIDE) is not stopped by 0500, so
    // this fault cannot be produced there. Probe it rather than assume it,
    // and skip with a visible note instead of failing on the environment.
    let probe = dir.path().join("probe");
    if std::fs::File::create(&probe).is_ok() {
        let _ = std::fs::remove_file(&probe);
        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(dir.path(), perms).expect("restore perms");
        eprintln!(
            "skipped: 0500 does not stop this user (running privileged), so the fault cannot be produced"
        );
        return;
    }

    let result = sui_id::database::open(&path, MasterKey::generate(), PreMigrationBackup::Take);

    // Restore permissions before any assertion can panic and leak a
    // read-only temp dir past this test.
    let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
    perms.set_mode(0o700);
    std::fs::set_permissions(dir.path(), perms).expect("restore perms");

    let err = match result {
        Ok(_) => panic!("must refuse"),
        Err(e) => e,
    };
    let refused = err
        .downcast_ref::<sui_id::database::PreMigrationBackupRefused>()
        .expect("PreMigrationBackupRefused");
    assert!(
        refused.line().starts_with("sui-id: refusing to migrate:"),
        "{}",
        refused.line()
    );
    assert!(
        refused.line().contains("could not write"),
        "{}",
        refused.line()
    );

    let remaining: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("pre-upgrade-schema"))
        .collect();
    assert!(
        remaining.is_empty(),
        "no partial snapshot file left behind: {remaining:?}"
    );
    assert_eq!(
        std::fs::read(&path).expect("read after"),
        before_bytes,
        "database untouched"
    );
}

#[test]
fn an_existing_file_of_the_same_name_refuses_and_the_existing_file_is_untouched() {
    use std::io::Write as _;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = seeded_at(dir.path(), 44);
    let max = sui_id_store::migrations::MAX_SCHEMA_VERSION;
    // A pre-existing file with the exact name `open()` will compute. The
    // name has one-second resolution and `open()` reads the clock itself, so
    // one predicted name is not enough: under CI load the second can tick
    // between this read and `open()`'s, and `open()` then picks a free name
    // and succeeds (CI run 43574983415). So every second from now through
    // the next two minutes is taken. `open()` reads the clock after `now`,
    // so it lands in this window unless the gap exceeds two minutes.
    let now = chrono::Utc::now();
    let collisions: Vec<PathBuf> = (0..=120)
        .map(|s| {
            sui_id_store::backup::pre_migration_snapshot_path_at(
                &path,
                44,
                max,
                now + chrono::Duration::seconds(s),
            )
        })
        .collect();
    for c in &collisions {
        let mut f = std::fs::File::create(c).expect("pre-create collision");
        f.write_all(b"not a database").expect("write");
    }

    let result = sui_id::database::open(&path, MasterKey::generate(), PreMigrationBackup::Take);

    assert!(
        result.is_err(),
        "must refuse: create_new must not overwrite"
    );
    for c in &collisions {
        assert_eq!(
            std::fs::read(c).expect("read collision after"),
            b"not a database",
            "the pre-existing file must be byte-identical afterwards: {}",
            c.display()
        );
    }
    let snapshots = std::fs::read_dir(dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("pre-upgrade-schema")
        })
        .count();
    assert_eq!(
        snapshots,
        collisions.len(),
        "no snapshot was written beside them"
    );
}

// ───────────────────────── §8.8: preview predicts the start ──────────────

/// The enough-space case: `preview-upgrade`'s reported MB figures must be
/// the same `check_free_space` numbers `database::open` would act on for
/// the same file, both read from the real filesystem (not a synthetic
/// value) -- the two code paths must agree because they call the same
/// function, not because a test constructed them to.
#[test]
fn preview_upgrade_predicts_the_real_free_space_numbers() {
    let h = Home::new();
    h.build_to_version(44);
    let cfg = h.config();

    let expected = sui_id_store::backup::check_free_space(&h.db());
    assert!(
        !expected.is_short(),
        "sanity: a temp dir has room for a tiny database"
    );
    // The fixture is far below a megabyte, so the operator line says so
    // rather than "0 MB"; the figures still come from the same reading.
    assert!(
        expected.needed_bytes < 1024 * 1024,
        "sanity: a tiny fixture"
    );

    let out = Command::new(BIN)
        .args(["preview-upgrade", "--config"])
        .arg(&cfg)
        .output()
        .expect("run preview-upgrade");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        stdout.contains("a backup of under 1 MB beside the database (under 1 MB free needed,"),
        "preview-upgrade's predicted size must match the real check_free_space reading: {stdout}"
    );
    assert!(
        stdout.contains("Before migrating, this sui-id writes a backup"),
        "{stdout}"
    );
    assert!(!stdout.contains("not enough space"), "{stdout}");
}

// ───────────────────────── §8.7: a WAL-only row is in the snapshot ────────

#[test]
fn a_wal_only_row_is_in_the_snapshot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = seeded_at(dir.path(), 44);

    let writer = rusqlite::Connection::open(&path).expect("writer");
    writer
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .expect("wal mode");
    writer
        .execute(
            "INSERT INTO users(id, username, is_admin, is_disabled, is_deleted, created_at, \
                               updated_at, user_uuid, failed_login_count) \
             VALUES('u2','u2',0,0,0,datetime('now'),datetime('now'),'22222222-2222-2222-2222-222222222222',0)",
            [],
        )
        .expect("a WAL-only row");

    let result = sui_id::database::open(&path, MasterKey::generate(), PreMigrationBackup::Take);
    drop(writer);
    result.expect("open must succeed");

    let snapshot = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("pre-upgrade-schema"))
        })
        .expect("snapshot");
    let count: i64 = rusqlite::Connection::open(&snapshot)
        .expect("open snapshot")
        .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 1, "the WAL-only row must be in the snapshot");
}
