//! RFC 142 stage 4 — `sui-id preview-upgrade` writes nothing, and says so
//! truthfully (§6 of `rfcs/handoffs/142-.../stage4-preview-the-upgrade-2026-11-07.md`).
//!
//! The pending/at-head/fresh/no-file wording is unit-tested directly against
//! `render_preview_upgrade` (`crates/sui-id/src/cli/tests.rs`) because that
//! function is pure; what belongs here is what only the real binary, against
//! a real file, can prove: the refusal line and exit code are identical to
//! every other refused-database path, the database needs no master key, and
//! — the point of this file — the database and its directory are byte-for-
//! byte and file-for-file identical before and after, including the one case
//! that could tempt a plain read-only open into creating a WAL sibling: a
//! cleanly-stopped WAL database, and a WAL database with a live writer still
//! holding the file open.

use std::path::PathBuf;
use std::process::{Command, Output};
use sui_id::database::{EXIT_DATABASE_REFUSED, invalid_line, too_new_line};

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
            .replace("127.0.0.1:8801", "127.0.0.1:18812");
        let path = self.dir.path().join("cfg.toml");
        std::fs::write(&path, text).expect("config");
        path
    }
    /// Let the binary create and fully migrate the database (the command
    /// fails with "user not found", which is the point: the open and the
    /// migration both succeeded).
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
    fn stamp(&self, sql: &str) {
        rusqlite::Connection::open(self.db())
            .expect("raw")
            .execute_batch(sql)
            .expect("stamp");
    }
    /// The exact set of file names directly inside the home directory.
    fn listing(&self) -> std::collections::BTreeSet<String> {
        std::fs::read_dir(self.dir.path())
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect()
    }
}

fn run(args: &[&str], config: &std::path::Path) -> Output {
    Command::new(BIN)
        .args(args)
        .arg(config)
        .output()
        .expect("run")
}

fn preview(config: &std::path::Path) -> Output {
    Command::new(BIN)
        .args(["preview-upgrade", "--config"])
        .arg(config)
        .output()
        .expect("run preview-upgrade")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

const RELEASE: &str = env!("CARGO_PKG_VERSION");

fn too_new_version() -> i64 {
    i64::from(sui_id_store::migrations::MAX_SCHEMA_VERSION) + 1
}

#[test]
fn at_head_exits_0_with_the_already_at_head_line_and_changes_nothing() {
    let h = Home::new();
    h.create();
    let cfg = h.config();
    let before = std::fs::read(h.db()).expect("read before");

    let out = preview(&cfg);

    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).contains(&format!(
            "The database is already at schema version {}. Starting this sui-id runs no \
             migrations. Nothing was changed.",
            sui_id_store::migrations::MAX_SCHEMA_VERSION
        )),
        "{}",
        stdout(&out)
    );
    assert_eq!(
        std::fs::read(h.db()).expect("read after"),
        before,
        "the database must be byte-for-byte unchanged"
    );
}

#[test]
fn no_database_reports_it_and_the_path_still_does_not_exist_afterwards() {
    let h = Home::new();
    let cfg = h.config();
    assert!(!h.db().exists());

    let out = preview(&cfg);

    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).contains(&format!("No database exists at {} yet.", h.db().display())),
        "{}",
        stdout(&out)
    );
    assert!(
        stdout(&out).contains("creates its schema and runs all"),
        "{}",
        stdout(&out)
    );
    assert!(
        !h.db().exists(),
        "a preview must not bring the database into existence"
    );
}

#[test]
fn an_empty_file_is_reported_as_fresh_and_is_left_exactly_as_empty() {
    let h = Home::new();
    std::fs::write(h.db(), []).expect("empty file");
    let cfg = h.config();

    let out = preview(&cfg);

    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).contains(&format!("The database at {} is empty.", h.db().display())),
        "{}",
        stdout(&out)
    );
    assert_eq!(
        std::fs::read(h.db()).expect("read after").len(),
        0,
        "a preview must not write to an empty file"
    );
}

#[test]
fn a_too_new_database_prints_exactly_databases_checks_line_and_exits_65() {
    let h = Home::new();
    h.create();
    let too_new = too_new_version();
    h.stamp(&format!(
        "UPDATE sui_meta SET value='{too_new}' WHERE key='schema_version'"
    ));
    let cfg = h.config();
    let before = std::fs::read(h.db()).expect("read before");

    let out = preview(&cfg);

    assert_eq!(out.status.code(), Some(i32::from(EXIT_DATABASE_REFUSED)));
    let expected = format!(
        "{}\n",
        too_new_line(
            &h.db(),
            too_new,
            sui_id_store::migrations::MAX_SCHEMA_VERSION,
            Some(RELEASE)
        )
    );
    assert_eq!(
        stderr(&out),
        expected,
        "the same line database::check produces"
    );
    assert!(out.stdout.is_empty());
    assert_eq!(
        std::fs::read(h.db()).expect("read after"),
        before,
        "a refusal must not change the database bytes"
    );
}

#[test]
fn an_invalid_database_prints_exactly_databases_checks_line_and_exits_65() {
    let h = Home::new();
    h.create();
    h.stamp("UPDATE sui_meta SET value='garbage' WHERE key='schema_version'");
    let cfg = h.config();

    let out = preview(&cfg);

    assert_eq!(out.status.code(), Some(i32::from(EXIT_DATABASE_REFUSED)));
    let detail = "the recorded value \"garbage\" is not a non-negative integer";
    let tables = {
        let c = rusqlite::Connection::open(h.db()).expect("raw");
        c.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' AND name <> 'sui_meta'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .expect("count") as usize
    };
    assert_eq!(
        stderr(&out),
        format!("{}\n", invalid_line(&h.db(), tables, detail))
    );
}

#[test]
fn no_key_file_the_preview_still_succeeds() {
    let h = Home::new();
    h.create();
    assert!(
        h.key().exists(),
        "sanity: create() does generate a key file"
    );
    std::fs::remove_file(h.key()).expect("remove key file");
    let cfg = h.config();

    let out = preview(&cfg);

    assert_eq!(
        out.status.code(),
        Some(0),
        "a preview must not need the master key: {}",
        stderr(&out)
    );
    assert!(
        !h.key().exists(),
        "the preview must not recreate the key file either"
    );
}

#[test]
fn writes_nothing_rollback_journal_mode() {
    let h = Home::new();
    h.create();
    h.stamp("PRAGMA journal_mode=DELETE");
    let cfg = h.config();
    let before_bytes = std::fs::read(h.db()).expect("read before");
    let before_listing = h.listing();

    let out = preview(&cfg);

    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(std::fs::read(h.db()).expect("read after"), before_bytes);
    assert_eq!(h.listing(), before_listing, "no sibling file may appear");
}

#[test]
fn writes_nothing_cleanly_closed_wal_mode() {
    let h = Home::new();
    h.create();
    // `Database::open` runs in WAL mode; after the one-shot CLI process that
    // `create()` ran exits, its connection has closed. A clean stop leaves
    // no `-shm`/`-wal` sibling -- assert that precondition so a regression
    // in the fixture itself does not masquerade as this test passing.
    let before_listing = h.listing();
    assert!(
        !before_listing
            .iter()
            .any(|n| n.ends_with("-wal") || n.ends_with("-shm")),
        "precondition: a cleanly stopped WAL database has no sibling files yet: {before_listing:?}"
    );
    let cfg = h.config();
    let before_bytes = std::fs::read(h.db()).expect("read before");

    let out = preview(&cfg);

    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(std::fs::read(h.db()).expect("read after"), before_bytes);
    assert_eq!(
        h.listing(),
        before_listing,
        "immutable=1 must create no -shm/-wal sibling"
    );
}

#[test]
fn writes_nothing_wal_with_a_live_writer_and_reads_its_committed_frames() {
    let h = Home::new();
    h.create();
    let cfg = h.config();

    // A writer connection stays open across the preview, with a committed
    // change sitting only in the WAL (autocheckpoint off, never dropped
    // until after the assertions): the preview must read it, not a stale
    // base-file snapshot, and must create nothing new doing so.
    let writer = rusqlite::Connection::open(h.db()).expect("writer");
    writer
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .expect("wal mode, no autocheckpoint");
    writer
        .execute_batch(
            "UPDATE sui_meta SET value='44' WHERE key='schema_version';\n\
             UPDATE sui_meta SET value='9.9.9-live-wal' WHERE key='last_migrated_by';",
        )
        .expect("commit a change that stays in the wal");

    let before_listing = h.listing();
    assert!(
        before_listing.iter().any(|n| n.ends_with("-wal")),
        "precondition: the live writer must have a -wal sibling: {before_listing:?}"
    );

    let out = preview(&cfg);
    // The listing must be taken before the writer closes: closing it
    // checkpoints and removes the WAL siblings itself, which would hide
    // whatever the preview did or did not create while they still existed.
    let after_listing = h.listing();
    drop(writer);

    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out)
            .contains("Schema version now:      44 (last migrated by sui-id 9.9.9-live-wal)"),
        "the preview must read the committed WAL frame, not the stale base file: {}",
        stdout(&out)
    );
    assert_eq!(
        after_listing, before_listing,
        "the preview must create no sibling file beyond what the live writer already holds"
    );
}
