//! RFC 100 stage 1 (cycle O's O1): the end-to-end refusal, through the
//! real binary — §6's own explicit ask, distinct from the in-process
//! `resolve_checked`/`open_checked` coverage in `sui-id`'s own
//! `runtime::keyring`/`runtime::database` test modules.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use sui_id::database::EXIT_DATABASE_REFUSED;
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
        let mut text = String::from_utf8(sample.stdout).expect("utf8");
        text = text
            .replace("./sui-id.sqlite", &self.db().display().to_string())
            .replace("./sui-id.key", &self.key().display().to_string())
            .replace("127.0.0.1:8801", "127.0.0.1:18815");
        let path = self.dir.path().join("cfg.toml");
        std::fs::write(&path, text).expect("config");
        path
    }
    /// Sets the instance up the same lightweight way `key_rotation.rs`'s
    /// two harnesses do: a CLI subcommand that opens (creating and
    /// migrating) the database and bootstraps the key file, failing
    /// harmlessly on "user not found" — the open itself is the point,
    /// not this command's own result.
    fn create(&self, cfg: &Path) {
        let out = self.run(
            &["admin", "unlock-user", "--username", "nobody", "--config"],
            cfg,
        );
        assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
        assert!(self.db().exists());
        assert!(self.key().exists());
    }
    fn run(&self, args: &[&str], cfg: &Path) -> Output {
        Command::new(BIN).args(args).arg(cfg).output().expect("run")
    }
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Run `serve` (no subcommand: the real server entry point) and wait for
/// it to exit on its own — a refusal exits at once, before binding any
/// listener; kill and fail if it does not, so a regression that makes
/// the server actually start (and hang waiting for connections) fails
/// this test instead of hanging the suite.
fn serve_and_wait_for_exit(config: &Path) -> Output {
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
                "serve did not exit on its own -- it must refuse before binding: stdout={} stderr={}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr),
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn sha256_hex(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    let bytes = std::fs::read(path).expect("read for hashing");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest.as_slice() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[test]
fn a_wrong_key_at_startup_exits_nonzero_names_both_fingerprints_and_leaves_the_database_untouched()
{
    let h = Home::new();
    let cfg = h.config();
    // "set up an instance": `admin unlock-user` opens the database the
    // same way `serve` does, through `open_checked` -- it migrates and
    // writes the sentinel under the original key, so there is no need
    // to actually run `serve` itself (which blocks forever on a
    // successful start, serving HTTP) just to establish that state.
    h.create(&cfg);

    let before_hash = sha256_hex(&h.db());
    let right_key_body = std::fs::read_to_string(h.key()).expect("read the right key");
    let right_key = MasterKey::from_base64(right_key_body.trim()).expect("parse");
    let right_fp = sui_id_store::crypto::fingerprint(&right_key);

    // "stop it, replace sui-id.key with another key, start it again."
    let wrong_key = MasterKey::generate();
    let wrong_fp = sui_id_store::crypto::fingerprint(&wrong_key);
    std::fs::write(h.key(), format!("{}\n", wrong_key.to_base64())).expect("replace key");

    let second = serve_and_wait_for_exit(&cfg);
    assert_eq!(
        second.status.code(),
        Some(i32::from(EXIT_DATABASE_REFUSED)),
        "a wrong key must exit {EXIT_DATABASE_REFUSED}, not start: stdout={} stderr={}",
        String::from_utf8_lossy(&second.stdout),
        stderr(&second)
    );
    let err_line = stderr(&second);
    assert!(
        err_line.contains(&wrong_fp),
        "the refusal must name the key actually in the file: {err_line}"
    );
    assert!(
        err_line.contains(&right_fp),
        "the refusal must name the sentinel's own fingerprint: {err_line}"
    );
    assert!(second.stdout.is_empty());

    let after_hash = sha256_hex(&h.db());
    assert_eq!(
        before_hash, after_hash,
        "a refused start must leave the database file byte-identical"
    );
}
