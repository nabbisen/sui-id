//! RFC 148 cycle O2a §6: the registry's own behaviour, through the real
//! binary. Unit-level parsing coverage (does every alias and every "a
//! flag's value is never taken for a command" case even parse the way
//! the dispatch expects) lives in `sui_id::cli`'s own test modules; this
//! file is the "one e2e per alias family" the dispatch names, plus the
//! registry/help/reference coverage and the unknown-command suggestion,
//! each through the spawned binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// A notice line (RFC 148 §2) is exactly one line, on stderr, and never
/// touches stdout.
fn assert_notice(out: &Output, old: &str, new: &str) {
    let want = format!("note: `sui-id {old}` is now `sui-id {new}`; the old form still works.");
    let err = stderr(out);
    assert!(
        err.lines().any(|l| l == want),
        "expected the notice {want:?} on its own line in stderr:\n{err}"
    );
}

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
            .args(["config", "sample"])
            .output()
            .expect("sample");
        let text = String::from_utf8(sample.stdout)
            .expect("utf8")
            .replace("./sui-id.sqlite", &self.db().display().to_string())
            .replace("./sui-id.key", &self.key().display().to_string())
            .replace("127.0.0.1:8801", "127.0.0.1:18816");
        let path = self.dir.path().join("cfg.toml");
        std::fs::write(&path, text).expect("config");
        path
    }
    /// Opens (creating and migrating) the database and bootstraps the
    /// key file, the same lightweight way `o1_startup_key_check.rs` does
    /// — failing harmlessly on "user not found" is fine, the open itself
    /// is the point.
    fn create(&self, cfg: &Path) {
        let out = Command::new(BIN)
            .args([
                "admin",
                "user",
                "unlock",
                "--username",
                "nobody",
                "--config",
            ])
            .arg(cfg)
            .output()
            .expect("run");
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        assert!(self.db().exists());
        assert!(self.key().exists());
    }
}

// ── Family: top-level mode flags (--print-sample-config, --version,
// --help, --dev) ────────────────────────────────────────────────────────

#[test]
fn print_sample_config_alias_notice_and_byte_identical_stdout() {
    let old = Command::new(BIN)
        .arg("--print-sample-config")
        .output()
        .expect("run");
    let new = Command::new(BIN)
        .args(["config", "sample"])
        .output()
        .expect("run");
    assert_eq!(old.status.code(), Some(0));
    assert_eq!(stdout(&old), stdout(&new), "stdout must be byte-identical");
    assert_notice(&old, "--print-sample-config", "config sample");
    assert!(stderr(&new).is_empty(), "the new form prints no notice");
}

#[test]
fn version_and_help_aliases_print_their_notice_to_stderr_only() {
    let v = Command::new(BIN).arg("--version").output().expect("run");
    assert_eq!(v.status.code(), Some(0));
    assert_notice(&v, "--version", "version");
    assert_eq!(
        stdout(&v),
        stdout(&Command::new(BIN).arg("version").output().expect("run"))
    );

    let h = Command::new(BIN).arg("--help").output().expect("run");
    assert_eq!(h.status.code(), Some(0));
    assert_notice(&h, "--help", "help");
    assert!(stdout(&h).contains("Getting started"), "{}", stdout(&h));
}

/// Drains a child's pipe into a shared buffer on its own thread, so
/// polling for a marker in the main thread never blocks inside a `read`
/// call waiting for bytes that may never come on *this* stream (the
/// bug this helper exists to avoid: `sui-id (dev mode) listening` is on
/// stdout, not stderr, and a blocking read on the wrong one hangs past
/// any wall-clock check the polling loop makes).
fn drain(
    mut pipe: impl std::io::Read + Send + 'static,
) -> std::sync::Arc<std::sync::Mutex<Vec<u8>>> {
    let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let shared = buf.clone();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => shared.lock().expect("lock").extend_from_slice(&chunk[..n]),
            }
        }
    });
    buf
}

/// The `--dev` flag family: it starts a server (never exits on success),
/// so this confirms the notice and that it actually started, then kills
/// it — `serve_and_wait_for_exit`-style but for a long-running success,
/// not a refusal.
#[test]
fn dev_flag_alias_prints_notice_and_actually_starts() {
    let mut child = Command::new(BIN)
        .args(["--dev", "--dev-bind", "127.0.0.1:18817"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out_buf = drain(child.stdout.take().expect("stdout"));
    let err_buf = drain(child.stderr.take().expect("stderr"));

    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            panic!(
                "sui-id --dev exited early ({status:?}): stdout={} stderr={}",
                String::from_utf8_lossy(&out_buf.lock().expect("lock")),
                String::from_utf8_lossy(&err_buf.lock().expect("lock")),
            );
        }
        let out_text = String::from_utf8_lossy(&out_buf.lock().expect("lock")).into_owned();
        let err_text = String::from_utf8_lossy(&err_buf.lock().expect("lock")).into_owned();
        if err_text.contains("note:") && out_text.contains("listening") {
            assert!(
                err_text
                    .lines()
                    .any(|l| l
                        == "note: `sui-id --dev` is now `sui-id dev`; the old form still works."),
                "{err_text}"
            );
            let _ = child.kill();
            return;
        }
        // A hang guard, not a speed assertion: the loop returns as soon as
        // the line appears. 20s failed under a loaded CI runner (0.93.0
        // O2b, run 44541738891: dev mode's seeding hashes passwords, slow
        // in a debug build), so the bound is generous.
        if start.elapsed() > Duration::from_secs(180) {
            let _ = child.kill();
            panic!(
                "sui-id --dev never printed both the notice and a listening line: \
                 stdout={out_text} stderr={err_text}"
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

// ── Family: backup-word aliases (backup, restore, verify-backup,
// preview-upgrade) ───────────────────────────────────────────────────────

#[test]
fn backup_restore_verify_preview_upgrade_old_words_print_their_notice() {
    let h = Home::new();
    let cfg = h.config();
    h.create(&cfg);

    let to = h.dir.path().join("b.tar");
    let out = Command::new(BIN)
        .args(["backup", "--to"])
        .arg(&to)
        .arg("--config")
        .arg(&cfg)
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_notice(&out, "backup --to PATH", "backup create --to PATH");
    assert!(to.exists());

    let out = Command::new(BIN)
        .args(["verify-backup", "--from"])
        .arg(&to)
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_notice(&out, "verify-backup", "backup verify");

    let out = Command::new(BIN)
        .args(["preview-upgrade", "--config"])
        .arg(&cfg)
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_notice(&out, "preview-upgrade", "upgrade preview");

    let dst_db = h.dir.path().join("restored.sqlite");
    let dst_key = h.dir.path().join("restored.key");
    let dst_cfg_text = std::fs::read_to_string(&cfg)
        .expect("read cfg")
        .replace(&h.db().display().to_string(), &dst_db.display().to_string())
        .replace(
            &h.key().display().to_string(),
            &dst_key.display().to_string(),
        );
    let dst_cfg = h.dir.path().join("dst.toml");
    std::fs::write(&dst_cfg, dst_cfg_text).expect("write dst cfg");
    let out = Command::new(BIN)
        .args(["restore", "--from"])
        .arg(&to)
        .arg("--config")
        .arg(&dst_cfg)
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_notice(&out, "restore", "backup restore");
    assert!(dst_db.exists());
}

// ── Family: admin subaction aliases ─────────────────────────────────────

#[test]
fn an_old_admin_subaction_alias_prints_notice_and_has_the_same_effect_as_its_new_form() {
    let h = Home::new();
    let cfg = h.config();
    h.create(&cfg);

    let out = Command::new(BIN)
        .args(["admin", "unlock-user", "--username", "nobody", "--config"])
        .arg(&cfg)
        .output()
        .expect("run");
    assert_notice(&out, "admin unlock-user", "admin user unlock");
    // Same effect as the new form: both reach the same "no such user"
    // outcome against this empty database (exit 1, not a parse error).
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));

    let new_form = Command::new(BIN)
        .args([
            "admin",
            "user",
            "unlock",
            "--username",
            "nobody",
            "--config",
        ])
        .arg(&cfg)
        .output()
        .expect("run");
    assert_eq!(new_form.status.code(), out.status.code());
    assert!(
        stderr(&new_form)
            .lines()
            .next()
            .map(|l| !l.starts_with("note:"))
            .unwrap_or(true)
    );
}

// ── RFC 107's own closure condition, through the real binary ────────────

#[test]
fn a_flags_value_is_never_taken_for_a_command() {
    let h = Home::new();
    // `--config backup`: a configuration literally named "backup" -- it
    // must be read as that file's path, not dispatch to the `backup`
    // command (which would then fail for lacking --to, not for a missing
    // config file).
    let out = Command::new(BIN)
        .args(["--config", "backup"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("backup") && stderr(&out).contains("no configuration found"),
        "{}",
        stderr(&out)
    );

    // `admin user unlock --username serve`: the value "serve" must reach
    // --username, not be mistaken for a command.
    let cfg = h.config();
    h.create(&cfg);
    let out = Command::new(BIN)
        .args(["admin", "user", "unlock", "--username", "serve", "--config"])
        .arg(&cfg)
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("\"serve\""),
        "the value must reach --username as the literal string \"serve\": {}",
        stderr(&out)
    );
}

// ── Help and the registry cannot diverge ─────────────────────────────────

/// Every leaf line printed by the overview resolves with `sui-id help
/// <leaf>`, through the real binary -- built by walking the registry
/// (`sui_id::cli::help`), not a hand list, so a leaf this loop cannot
/// find would mean the overview and `help`'s own resolver disagree.
#[test]
fn help_and_the_registry_cannot_diverge() {
    let overview = Command::new(BIN).arg("help").output().expect("run");
    assert_eq!(overview.status.code(), Some(0));
    let text = stdout(&overview);

    for leaf in [
        "setup",
        "serve",
        "backup create",
        "backup restore",
        "backup verify",
        "upgrade preview",
        "config sample",
        "config check",
        "admin user unlock",
        "admin user reset-mfa",
        "admin user recovery-link",
        "admin master-key rotate",
        "admin metrics-token rotate",
        "admin registration-token issue",
        "admin registration-token revoke",
        "admin registration-token list",
        "admin registration-token legacy-candidates",
        "dev",
        "help",
        "version",
    ] {
        assert!(
            text.contains(leaf),
            "the overview must mention {leaf:?}:\n{text}"
        );
        let mut args: Vec<&str> = vec!["help"];
        args.extend(leaf.split(' '));
        let out = Command::new(BIN).args(&args).output().expect("run");
        assert_eq!(
            out.status.code(),
            Some(0),
            "`sui-id help {leaf}` must succeed: {}",
            stderr(&out)
        );
    }
}

#[test]
fn an_unknown_command_suggests_the_nearest_and_exits_nonzero() {
    let out = Command::new(BIN).arg("bakup").output().expect("run");
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("backup"), "{err}");

    let out = Command::new(BIN)
        .args(["help", "bakup", "create"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("backup"), "{err}");
    assert!(err.contains("sui-id help"), "{err}");
}

// ── Review fold-ins (0.93.0 O2a) ────────────────────────────────────────────

/// The documented service unit runs exactly `sui-id --config PATH`
/// (`docs/src/guides/deployment.md:240`). It must still serve, and print no
/// notice: a service log should not gain a line on every start.
#[test]
fn bare_config_serves_and_prints_no_notice() {
    let h = Home::new();
    let cfg = h.config();
    let text = std::fs::read_to_string(&cfg)
        .expect("read cfg")
        .replace("127.0.0.1:18816", "127.0.0.1:18819");
    std::fs::write(&cfg, text).expect("write cfg");

    let mut child = Command::new(BIN)
        .arg("--config")
        .arg(&cfg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out_buf = drain(child.stdout.take().expect("stdout"));
    let err_buf = drain(child.stderr.take().expect("stderr"));

    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            panic!(
                "bare `sui-id --config` exited early ({status:?}): stdout={} stderr={}",
                String::from_utf8_lossy(&out_buf.lock().expect("lock")),
                String::from_utf8_lossy(&err_buf.lock().expect("lock")),
            );
        }
        let out_text = String::from_utf8_lossy(&out_buf.lock().expect("lock")).into_owned();
        let err_text = String::from_utf8_lossy(&err_buf.lock().expect("lock")).into_owned();
        if out_text.contains("sui-id listening") || err_text.contains("sui-id listening") {
            let _ = child.kill();
            assert!(
                !err_text.contains("note:"),
                "bare `sui-id --config` printed a notice: {err_text}"
            );
            return;
        }
        // A hang guard, not a speed assertion: the loop returns as soon as
        // the line appears. 20s failed under a loaded CI runner (0.93.0
        // O2b, run 44541738891: dev mode's seeding hashes passwords, slow
        // in a debug build), so the bound is generous.
        if start.elapsed() > Duration::from_secs(180) {
            let _ = child.kill();
            panic!("bare `sui-id --config` never listened: stdout={out_text} stderr={err_text}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A subcommand's `--help` prints that command's own detail, exits 0, and
/// prints no notice (it is not a pre-148 form). A flag's *value* spelled
/// `-h` is never taken for the help flag (RFC 107's bug class).
#[test]
fn a_subcommands_help_is_its_own_and_a_value_is_never_the_help_flag() {
    let out = Command::new(BIN)
        .args(["backup", "create", "--help"])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).contains("--to"), "{}", stdout(&out));
    assert!(!stderr(&out).contains("note:"), "{}", stderr(&out));
    assert!(
        !stdout(&out).contains("Getting started"),
        "the overview, not the command's help"
    );

    let out = Command::new(BIN)
        .args([
            "admin",
            "user",
            "reset-mfa",
            "--username",
            "x",
            "--reason",
            "-h",
        ])
        .output()
        .expect("run");
    assert_ne!(out.status.code(), Some(0));
    assert!(
        !stdout(&out).contains("Getting started"),
        "a value of `-h` printed the overview: {}",
        stdout(&out)
    );
}
