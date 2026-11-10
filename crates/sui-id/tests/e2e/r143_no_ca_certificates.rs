//! RFC 143 stage 2 part A — the real binary, with no CA certificates on
//! the (simulated) host, must start, keep serving, and warn once; it
//! must never panic for want of certificates.
//!
//! `SSL_CERT_FILE`/`SSL_CERT_DIR` (honoured by `rustls-native-certs`
//! 0.8.5, which `rustls-platform-verifier` falls back to on Linux) are
//! pointed at an empty file and an empty directory respectively, so
//! this process — and only this process, env vars set on the child, not
//! this test binary's own — sees a host with no CA certificates at all,
//! the exact condition stage 1's smoke test found crashing every build.

#![allow(dead_code)]

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_sui-id");

struct Home {
    dir: tempfile::TempDir,
    /// This run's own port. It is not a shared constant: both tests here
    /// start a server, and `cargo test` runs them in parallel. With one fixed
    /// port, a test's connection could reach the *other* test's server, count
    /// it as its own, and kill its own child before it had logged a line. That
    /// gave empty output in Level B run 42825365703. The OS hands out an unused
    /// port, which is then released for the child to bind.
    port: u16,
}

impl Home {
    fn new() -> Self {
        let port = std::net::TcpListener::bind(("127.0.0.1", 0))
            .expect("bind an ephemeral port")
            .local_addr()
            .expect("local addr")
            .port();
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
            port,
        }
    }

    fn config(&self) -> PathBuf {
        let sample = Command::new(BIN)
            .arg("--print-sample-config")
            .output()
            .expect("sample");
        let text = String::from_utf8(sample.stdout)
            .expect("utf8")
            .replace(
                "./sui-id.sqlite",
                &self.dir.path().join("t.sqlite").display().to_string(),
            )
            .replace(
                "./sui-id.key",
                &self.dir.path().join("t.key").display().to_string(),
            )
            .replace("127.0.0.1:8801", &format!("127.0.0.1:{}", self.port));
        let path = self.dir.path().join("cfg.toml");
        std::fs::write(&path, text).expect("config");
        path
    }

    /// An existing, empty file — a CA bundle with nothing in it.
    fn empty_ca_file(&self) -> PathBuf {
        let path = self.dir.path().join("empty-ca.crt");
        std::fs::write(&path, b"").expect("empty ca file");
        path
    }

    /// An existing, empty directory — a CA directory with nothing in it.
    fn empty_ca_dir(&self) -> PathBuf {
        let path = self.dir.path().join("empty-ca-dir");
        std::fs::create_dir(&path).expect("empty ca dir");
        path
    }
}

/// Starts the real binary with no CA certificates reachable, waits for it
/// to either accept a TCP connection on its configured port or exit, then
/// kills it and returns everything captured.
struct Run {
    accepted_connection: bool,
    exited_early: bool,
    output: std::process::Output,
}

fn run_with_empty_ca_bundle(home: &Home) -> Run {
    let mut child = Command::new(BIN)
        .arg("--config")
        .arg(home.config())
        .env("SSL_CERT_FILE", home.empty_ca_file())
        .env("SSL_CERT_DIR", home.empty_ca_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    let start = Instant::now();
    let mut accepted_connection = false;
    let mut exited_early = false;
    loop {
        if std::net::TcpStream::connect(("127.0.0.1", home.port)).is_ok() {
            accepted_connection = true;
            break;
        }
        if child.try_wait().expect("try_wait").is_some() {
            exited_early = true;
            break;
        }
        if start.elapsed() > Duration::from_secs(20) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_string(&mut stdout);
    }
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_string(&mut stderr);
    }
    let status = child.wait().expect("wait");
    Run {
        accepted_connection,
        exited_early,
        output: std::process::Output {
            status,
            stdout: stdout.into_bytes(),
            stderr: stderr.into_bytes(),
        },
    }
}

fn combined(run: &Run) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&run.output.stdout),
        String::from_utf8_lossy(&run.output.stderr)
    )
}

#[test]
fn starts_and_serves_with_no_ca_certificates_at_all() {
    let home = Home::new();
    let run = run_with_empty_ca_bundle(&home);
    let text = combined(&run);

    assert!(
        run.accepted_connection,
        "the server never accepted a connection on its configured port \
         (exited early: {}); output:\n{text}",
        run.exited_early
    );
    assert!(
        !run.exited_early,
        "the process exited before this test could even connect; output:\n{text}"
    );
    assert!(
        !text.contains("panicked at"),
        "no client construction may panic for want of CA certificates; output:\n{text}"
    );
}

#[test]
fn warns_once_in_operator_words_naming_every_affected_feature() {
    let home = Home::new();
    let run = run_with_empty_ca_bundle(&home);
    let text = combined(&run);

    let line = text
        .lines()
        .find(|l| l.contains("no CA certificates were found on this host"))
        .unwrap_or_else(|| panic!("no such warning line in output:\n{text}"));
    assert!(line.contains("WARN"), "wrong level: {line}");
    for feature in ["HIBP", "SMTP", "LDAPS", "federation"] {
        assert!(
            line.contains(feature),
            "warning does not name {feature}: {line}"
        );
    }
    assert!(
        line.contains("ca-certificates"),
        "warning does not say how to fix it: {line}"
    );
    assert!(
        line.contains("no password will be breach-checked"),
        "warning does not say plainly what HIBP's fail-open does: {line}"
    );

    // Logged once: the exact phrase appears in exactly one line, not once
    // per request this test's own TCP-connect probing might have caused.
    let count = text
        .lines()
        .filter(|l| l.contains("no CA certificates were found on this host"))
        .count();
    assert_eq!(count, 1, "expected exactly one warning line:\n{text}");
}
