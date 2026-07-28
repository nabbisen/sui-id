//! RFC 093 G09 follow-up (review finding B2): `wasm-smtp-tokio`'s
//! implicit-TLS transport hits the identical rustls `ClientConfig::builder()`
//! panic as `ldap3`'s TLS path (see
//! `crates/sui-id-store/tests/ldap_smoke.rs` for the full mechanism). The
//! `serve_dev` entry point builds a real `SmtpMailSender` without going
//! through `runtime::startup::prepare()`, so it was missed by the original
//! fix, which only ran there. The fix now installs the provider in `main()`
//! before subcommand dispatch, covering every entry point including
//! `serve_dev`; this test proves the same provider-install precondition
//! this crate depends on for that fix to work, using the exact call site
//! `SmtpMailSender::send` uses (`mail.rs:197`).
//!
//! Unlike `ldap_smoke.rs`, this is a positive-only check: installing a
//! provider is a harmless, idempotent, additive operation for any other
//! test that happens to share this process, so it needs none of
//! `ldap_smoke.rs`'s subprocess isolation. It does not attempt a
//! `SmtpMailSender::send` end-to-end (that requires a live `smtp_config` DB
//! row and is exercised by the existing outbox/e2e coverage); it isolates
//! the one thing this finding was about — the transport construction that
//! panics without a provider.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use tokio::net::TcpListener;
use wasm_smtp_tokio::TokioTlsTransport;

/// With the provider installed (matching `runtime::startup::
/// install_rustls_crypto_provider`'s choice of `aws_lc_rs`), the implicit-TLS
/// transport reaches the TLS/provider path — `ClientConfig::builder()`
/// succeeds — and only fails once it attempts a handshake against a fixture
/// that never speaks TLS. A connection-level error or timeout here is
/// expected and acceptable; a panic is not.
#[tokio::test]
async fn smtp_implicit_tls_reaches_provider_path_without_panicking() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback listener");
    let addr = listener.local_addr().expect("local_addr");
    let accept = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            drop(stream);
        }
    });

    let attempt = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        TokioTlsTransport::connect_implicit_tls(
            &addr.ip().to_string(),
            addr.port(),
            &addr.ip().to_string(),
        ),
    )
    .await;

    match attempt {
        Ok(Ok(_)) => panic!("connection unexpectedly succeeded against a non-TLS fixture"),
        Ok(Err(_)) => {
            // A typed IO/TLS error (rather than a panic) proves the provider
            // path executed.
        }
        Err(_) => {
            // Timed out mid-handshake. A missing provider panics
            // immediately rather than hanging, so this also proves the
            // provider path was reached.
        }
    }

    accept.abort();
}
