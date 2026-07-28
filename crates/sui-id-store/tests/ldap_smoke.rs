//! RFC 093 G09 — rustls crypto-provider smoke test for the LDAP TLS path.
//!
//! `ldap3`'s `tls-rustls-ring` feature builds a `rustls::ClientConfig` via
//! the bare `ClientConfig::builder()`. In this workspace that call is
//! ambiguous — `sui-id-store`'s `rustls` dev-dependency compiles in both
//! the `ring` and `aws_lc_rs` provider backends (matching the real `sui-id`
//! binary, where reqwest's rustls integration pulls in `aws_lc_rs` and
//! ldap3's `tls-rustls-ring` pulls in `ring`) — so it panics unless a
//! process-level `CryptoProvider` was explicitly installed first. That is
//! the "provider panic" RFC 093 names as the reason this gate exists.
//! Production installs the provider exactly once at startup
//! (`crates/sui-id/src/runtime/startup.rs::install_rustls_crypto_provider`),
//! before any LDAPS or federation connection can occur. These two tests
//! prove both directions of that precondition against a local loopback
//! fixture. No public LDAP service is contacted, and no real directory
//! credentials are used.
//!
//! ```text
//! cargo test -p sui-id-store --features ldap --test ldap_smoke --locked \
//!   -- --exact rustls_provider_and_tls_connector_reach_fixture
//! cargo test -p sui-id-store --features ldap --test ldap_smoke --locked \
//!   -- --exact rejects_missing_crypto_provider
//! ```
//!
//! `CryptoProvider::install_default()` is a one-shot, unresettable, process-
//! global operation, and a plain `cargo test --workspace --all-features`
//! (as G04/G06 run) executes every test in this binary in one process. The
//! negative case therefore cannot simply run in-process like the positive
//! one — if it did, whichever test happened to install a provider first
//! would leak that into the other. It re-execs itself in an isolated child
//! process instead, so it observes a guaranteed-clean process regardless of
//! what else ran in the parent.

#![cfg(feature = "ldap")]
// panic!()/expect()/unwrap() on setup and assertion failures is normal
// integration-test style here; a failure is exactly the panic we want.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use ldap3::{LdapConnAsync, LdapConnSettings};
use std::time::Duration;
use tokio::net::TcpListener;

/// Bind a loopback listener that accepts one connection and holds it open
/// without speaking any protocol. A TLS client connecting to it gets past
/// TCP connect and into a real handshake attempt, which is exactly the
/// boundary these tests need to reach (or panic before reaching, for the
/// negative case).
async fn spawn_dummy_listener() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback listener");
    let addr = listener.local_addr().expect("local_addr");
    let handle = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            // Hold the connection open briefly without speaking TLS (or
            // anything else). The client's handshake fails on its own
            // once it gets a non-TLS response, or times out waiting.
            tokio::time::sleep(Duration::from_millis(500)).await;
            drop(stream);
        }
    });
    (format!("ldaps://{addr}"), handle)
}

/// Positive case: with the provider installed, the connector reaches the
/// TLS/provider path — `ClientConfig::builder()` succeeds — and only fails
/// once it attempts the handshake against our non-TLS-speaking fixture.
/// A connection-level rejection or timeout here is expected and acceptable;
/// a panic, or any sign of a plaintext downgrade, is not.
#[tokio::test]
async fn rustls_provider_and_tls_connector_reach_fixture() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (url, _listener) = spawn_dummy_listener().await;
    let settings = LdapConnSettings::new().set_conn_timeout(Duration::from_secs(3));

    let attempt = tokio::time::timeout(
        Duration::from_secs(5),
        LdapConnAsync::with_settings(settings, &url),
    )
    .await;

    match attempt {
        Ok(Ok(_)) => {
            panic!("connection unexpectedly succeeded against a fixture that never speaks TLS")
        }
        Ok(Err(e)) => {
            // Reaching a typed ldap3 error (rather than a panic) proves the
            // provider path executed. Fail closed on anything that looks
            // like a plaintext downgrade instead of a TLS-layer failure.
            let msg = e.to_string();
            assert!(
                !msg.to_lowercase().contains("plaintext"),
                "unexpected plaintext downgrade: {msg}"
            );
        }
        Err(_) => {
            // Timed out mid-handshake. A missing provider panics
            // immediately rather than hanging, so a timeout here also
            // proves the provider path was reached and is an acceptable
            // outcome against a fixture that never completes a handshake.
        }
    }
}

/// Environment variable marking the isolated child-process worker for
/// [`rejects_missing_crypto_provider`]. Its presence, not its value,
/// matters.
const SUBPROCESS_WORKER_ENV: &str = "LDAP_SMOKE_REJECTS_PROVIDER_WORKER";

/// Negative case, outer half: re-exec this exact test in a fresh child
/// process so no other test in this binary can have already installed a
/// `CryptoProvider` before it runs. The child's own test body (below)
/// observes the panic in-process via `JoinError::is_panic()` and asserts
/// on it directly — a caught task panic does not fail the child's test,
/// it *is* the child's test passing — so the outer half just needs the
/// child to run cleanly and its stderr to show the expected panic
/// actually fired, not (wrongly) a non-zero child exit status.
#[tokio::test]
async fn rejects_missing_crypto_provider() {
    if std::env::var_os(SUBPROCESS_WORKER_ENV).is_some() {
        rejects_missing_crypto_provider_worker().await;
        return;
    }

    let exe = std::env::current_exe().expect("current_exe");
    let output = std::process::Command::new(exe)
        .arg("--exact")
        .arg("rejects_missing_crypto_provider")
        .arg("--nocapture")
        .env(SUBPROCESS_WORKER_ENV, "1")
        .output()
        .expect("spawn isolated subprocess");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "subprocess worker failed (expected it to pass by observing the \
         provider panic internally):\nstdout:\n{stdout}\nstderr:\n{stderr}",
    );
    assert!(
        stderr.contains("no process-level CryptoProvider")
            || stderr
                .contains("Could not automatically determine the process-level CryptoProvider"),
        "subprocess passed, but stderr does not show the expected missing-\
         provider panic — it may have passed for the wrong reason:\n{stderr}",
    );
}

/// Negative case, inner half: runs only inside the isolated child process
/// (see [`SUBPROCESS_WORKER_ENV`]). Attempts the identical connector
/// construction as the positive test, without installing a provider first.
/// The connector construction runs inside a spawned task specifically so
/// its panic is caught by Tokio and observable via `JoinError::is_panic()`
/// here, rather than unwinding into (and being separately caught by) the
/// outer `#[tokio::test]` harness — asserting on the classified `JoinError`
/// is the actual test; a plain top-level panic would only prove *some*
/// panic happened, not this one.
async fn rejects_missing_crypto_provider_worker() {
    let (url, _listener) = spawn_dummy_listener().await;

    let task = tokio::spawn(async move {
        let settings = LdapConnSettings::new().set_conn_timeout(Duration::from_secs(3));
        let _ = LdapConnAsync::with_settings(settings, &url).await;
    });

    match tokio::time::timeout(Duration::from_secs(5), task).await {
        Ok(Err(join_err)) => {
            assert!(
                join_err.is_panic(),
                "expected a panic from the missing crypto provider, got: {join_err}"
            );
        }
        Ok(Ok(())) => {
            panic!("connection unexpectedly completed without a crypto provider installed")
        }
        Err(_) => {
            panic!("connection attempt hung instead of panicking on the missing crypto provider")
        }
    }
}
