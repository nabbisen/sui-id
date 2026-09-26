//! RFC 112 stage 2 — the operator is told, once, in a line they can act on.
//!
//! These run the **real binary** (`CARGO_BIN_EXE_sui-id`) against a database the
//! binary itself created and migrated, then stamped: the exact stderr, the exit
//! code, the `tracing` event and the exemptions are what is asserted, because the
//! failure this stage fixes was that the cause sat on line four of
//! `Error: opening database`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
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
    fn logs(&self) -> PathBuf {
        self.dir.path().join("logs")
    }
    fn config(&self, log_file: bool) -> PathBuf {
        let sample = Command::new(BIN)
            .arg("--print-sample-config")
            .output()
            .expect("sample");
        let mut text = String::from_utf8(sample.stdout).expect("utf8");
        text = text
            .replace("./sui-id.sqlite", &self.db().display().to_string())
            .replace(
                "./sui-id.key",
                &self.dir.path().join("t.key").display().to_string(),
            )
            .replace("127.0.0.1:8801", "127.0.0.1:18811");
        if log_file {
            text = text.replace(
                "access_log = false",
                &format!("access_log = false\nfile = \"{}\"", self.logs().display()),
            );
        }
        let path = self
            .dir
            .path()
            .join(if log_file { "cfg-log.toml" } else { "cfg.toml" });
        std::fs::write(&path, text).expect("config");
        path
    }
    /// Let the binary create and migrate the database (the command fails with
    /// "user not found", which is the point: the open succeeded).
    fn create(&self) {
        let out = run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            &self.config(false),
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
    fn tables(&self) -> usize {
        let c = rusqlite::Connection::open(self.db()).expect("raw");
        c.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name <> 'sui_meta'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .expect("count") as usize
    }
    fn stored(&self) -> String {
        rusqlite::Connection::open(self.db())
            .expect("raw")
            .query_row(
                "SELECT value FROM sui_meta WHERE key='schema_version'",
                [],
                |r| r.get(0),
            )
            .expect("version")
    }
}

fn run(args: &[&str], config: &Path) -> Output {
    Command::new(BIN)
        .args(args)
        .arg(config)
        .output()
        .expect("run")
}

/// Run `serve` and wait for it to exit on its own (a refusal exits at once); kill
/// and fail if it does not.
fn serve(config: &Path) -> Output {
    let mut child = Command::new(BIN)
        .arg("--config")
        .arg(config)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let start = Instant::now();
    loop {
        if child.try_wait().expect("wait").is_some() {
            return child.wait_with_output().expect("output");
        }
        if start.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let out = child.wait_with_output().expect("output");
            panic!(
                "serve did not exit: stdout={}",
                String::from_utf8_lossy(&out.stdout)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

const RELEASE: &str = env!("CARGO_PKG_VERSION");

#[test]
fn a_cli_subcommand_against_a_too_new_database_prints_one_line_and_exits_65() {
    let h = Home::new();
    h.create();
    h.stamp("UPDATE sui_meta SET value='44' WHERE key='schema_version'");
    let cfg = h.config(false);
    let out = run(
        &["admin", "unlock-user", "--username", "nobody", "--config"],
        &cfg,
    );
    assert_eq!(out.status.code(), Some(i32::from(EXIT_DATABASE_REFUSED)));
    let expected = format!(
        "{}\n",
        too_new_line(
            &h.db(),
            44,
            sui_id_store::migrations::MAX_SCHEMA_VERSION,
            Some(RELEASE)
        )
    );
    assert_eq!(stderr(&out), expected, "first and alone, and nothing else");
    assert!(out.stdout.is_empty());
    assert!(!stderr(&out).contains("Error:") && !stderr(&out).contains("Caused by"));
    assert_eq!(h.stored(), "44", "nothing was changed");
}

#[test]
fn a_cli_subcommand_against_a_garbled_stamp_prints_one_line_and_exits_65() {
    let h = Home::new();
    h.create();
    h.stamp("UPDATE sui_meta SET value='garbage' WHERE key='schema_version'");
    let tables = h.tables();
    let out = run(
        &["admin", "unlock-user", "--username", "nobody", "--config"],
        &h.config(false),
    );
    assert_eq!(out.status.code(), Some(i32::from(EXIT_DATABASE_REFUSED)));
    let detail = "the recorded value \"garbage\" is not a non-negative integer";
    assert_eq!(
        stderr(&out),
        format!("{}\n", invalid_line(&h.db(), tables, detail))
    );
    assert_eq!(
        h.stored(),
        "garbage",
        "the stamp is not overwritten with `1` any more"
    );
}

#[test]
fn both_refusal_variants_and_every_opener_share_exit_code_65() {
    // All seven CLI openers, for each variant; `serve` is below.
    let h = Home::new();
    h.create();
    let cfg = h.config(false);
    for (label, sql) in [
        (
            "too new",
            "UPDATE sui_meta SET value='99' WHERE key='schema_version'",
        ),
        (
            "unreadable",
            "UPDATE sui_meta SET value='' WHERE key='schema_version'",
        ),
        (
            "row deleted, database populated",
            "DELETE FROM sui_meta WHERE key='schema_version'",
        ),
    ] {
        h.stamp(sql);
        for args in [
            &["admin", "unlock-user", "--username", "x", "--config"][..],
            &[
                "admin",
                "reset-mfa",
                "--username",
                "x",
                "--reason",
                "r",
                "--config",
            ][..],
            &["admin", "rotate-metrics-token", "--config"][..],
            &[
                "admin",
                "issue-recovery-link",
                "--username",
                "x",
                "--reason",
                "r",
                "--config",
            ][..],
            &["admin", "rotate-key", "--config"][..],
            &["admin", "issue-registration-token", "--config"][..],
            &["setup", "--admin-username", "root", "--config"][..],
        ] {
            let out = run(args, &cfg);
            assert_eq!(
                out.status.code(),
                Some(65),
                "{label} / {args:?}: {}",
                stderr(&out)
            );
            assert!(
                stderr(&out).starts_with("sui-id: refusing to run:"),
                "{label}: {}",
                stderr(&out)
            );
            assert_eq!(
                stderr(&out).lines().count(),
                1,
                "{label} / {args:?}: nothing before or after the line"
            );
        }
        // Put the true stamp back for the next variant.
        h.stamp("INSERT OR REPLACE INTO sui_meta(key, value) VALUES('schema_version', '43')");
    }
}

#[test]
fn serve_against_a_too_new_database_exits_65_with_the_line_first_and_alone() {
    let h = Home::new();
    h.create();
    h.stamp("UPDATE sui_meta SET value='44' WHERE key='schema_version'");
    let out = serve(&h.config(false));
    assert_eq!(out.status.code(), Some(65));
    let expected = format!(
        "{}\n",
        too_new_line(
            &h.db(),
            44,
            sui_id_store::migrations::MAX_SCHEMA_VERSION,
            Some(RELEASE)
        )
    );
    assert_eq!(stderr(&out), expected);
    assert_eq!(h.stored(), "44");
}

#[test]
fn serve_against_a_garbled_stamp_exits_65_too() {
    let h = Home::new();
    h.create();
    h.stamp("UPDATE sui_meta SET value='+43' WHERE key='schema_version'");
    let out = serve(&h.config(false));
    assert_eq!(out.status.code(), Some(65));
    assert!(
        stderr(&out).starts_with("sui-id: refusing to run:"),
        "{}",
        stderr(&out)
    );
    assert_eq!(stderr(&out).lines().count(), 1);
    assert_eq!(h.stored(), "+43");
}

#[test]
fn serve_logs_one_tracing_event_with_found_supported_and_the_path() {
    let h = Home::new();
    h.create();
    h.stamp("UPDATE sui_meta SET value='44' WHERE key='schema_version'");
    let out = serve(&h.config(true));
    assert_eq!(out.status.code(), Some(65));
    // The line is first on stderr; with a log file configured, tracing also
    // writes to stderr, after it.
    assert!(
        stderr(&out).starts_with("sui-id: refusing to run:"),
        "{}",
        stderr(&out)
    );
    let mut logged = String::new();
    for entry in std::fs::read_dir(h.logs()).expect("log dir").flatten() {
        logged.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
    }
    let events: Vec<&str> = logged
        .lines()
        .filter(|l| l.contains("refusing to use the database"))
        .collect();
    assert_eq!(events.len(), 1, "exactly one event: {logged}");
    let e = events[0];
    assert!(e.contains("ERROR"), "{e}");
    assert!(e.contains("found=44") && e.contains("supported=43"), "{e}");
    assert!(e.contains(&h.db().display().to_string()), "{e}");
    assert!(e.contains("schema_too_new"), "{e}");
}

#[test]
fn a_failure_that_is_not_a_refusal_keeps_exit_1_and_its_shape() {
    let h = Home::new();
    h.create();
    let out = run(
        &["admin", "unlock-user", "--username", "nobody", "--config"],
        &h.config(false),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).starts_with("Error: looking up user"),
        "{}",
        stderr(&out)
    );
    assert!(stderr(&out).contains("Caused by"), "{}", stderr(&out));
}

#[test]
fn success_help_and_version_are_still_zero_and_need_no_database() {
    for arg in ["--version", "--help", "--print-sample-config"] {
        let out = Command::new(BIN).arg(arg).output().expect("run");
        assert_eq!(out.status.code(), Some(0), "{arg}");
    }
}

#[test]
fn backup_of_a_database_the_binary_cannot_open_still_works() {
    // D1's route back starts with a backup, so it must not be refused: it is a
    // file-level snapshot and never calls `Database::open`.
    let h = Home::new();
    h.create();
    h.stamp("UPDATE sui_meta SET value='44' WHERE key='schema_version'");
    let to = h.dir.path().join("out.tar");
    let out = Command::new(BIN)
        .args(["backup", "--to"])
        .arg(&to)
        .arg("--config")
        .arg(h.config(false))
        .output()
        .expect("backup");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(to.exists());
    // ...and verify reads the archive without opening any database of its own.
    let out = Command::new(BIN)
        .args(["verify-backup", "--from"])
        .arg(&to)
        .output()
        .expect("verify");
    assert_eq!(
        out.status.code(),
        Some(1),
        "an archive of a too-new database does not verify on this build"
    );
    assert!(stderr(&out).contains("newer"), "{}", stderr(&out));
}
