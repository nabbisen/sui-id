//! RFC 134 D3 and RFC 096-A harness: TLS mock upstreams for federation e2e tests.
//!
//! Two modes, both over a real rustls handshake:
//!
//! - [`serve_https`]: an `axum` router served over TLS. Exercises hyper and the
//!   full HTTP stack for real. Most federation cases use this.
//! - [`serve_raw_https`]: a fixture that completes the handshake and then writes
//!   **bytes the test chose**. An `axum` server cannot produce protocol-level
//!   abuse (a `Content-Length` that lies, a body that stops mid-stream), so this
//!   mode exists for those rows only.
//!
//! The client these mocks are reached with is
//! [`federation_test_client`]: the production constructor
//! (`sui_id::egress::build_federation_client_for_tests`), differing from
//! production in exactly two named parameters. Its resolver is the caller's
//! choice, and it trusts one extra root, [`mock_root`]. Verification stays on
//! for both the mock and the platform roots.
//!
//! Every mock in a process presents the same self-signed certificate, so one
//! extra root covers all of them.

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

/// Host name the raw fixture answers for. It is a name, not an address, so that
/// the request goes through the client's resolver (an IP literal would bypass it).
pub(super) const RAW_FIXTURE_HOST: &str = "fixture.test";

struct MockCert {
    cert_der: rustls::pki_types::CertificateDer<'static>,
    key_der: rustls::pki_types::PrivatePkcs8KeyDer<'static>,
}

/// One self-signed certificate per test process, valid for every mock's name.
fn mock_cert() -> &'static MockCert {
    static CERT: OnceLock<MockCert> = OnceLock::new();
    CERT.get_or_init(|| {
        let cert =
            rcgen::generate_simple_self_signed(vec!["127.0.0.1".into(), RAW_FIXTURE_HOST.into()])
                .expect("generate self-signed cert");
        MockCert {
            cert_der: cert.cert.der().clone(),
            key_der: rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()),
        }
    })
}

/// The one extra trusted root every test federation client is built with.
pub(super) fn mock_root() -> reqwest::Certificate {
    reqwest::Certificate::from_der(mock_cert().cert_der.as_ref()).expect("mock root certificate")
}

fn tls_acceptor() -> tokio_rustls::TlsAcceptor {
    // Normally installed once from `main()` (`install_rustls_crypto_provider`,
    // never reached by `test_app()`'s lighter setup); needed here because this is
    // the first thing in the e2e suite to perform a real TLS handshake, server or
    // client side. Idempotent.
    sui_id::startup::install_rustls_crypto_provider();
    let cert = mock_cert();
    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.cert_der.clone()],
            rustls::pki_types::PrivateKeyDer::from(cert.key_der.clone_key()),
        )
        .expect("build rustls ServerConfig");
    // The real federation egress client is `.http1_only()` (RFC 134 D1); match that
    // in ALPN so there is no h2-vs-h1 mismatch to debug.
    server_config.alpn_protocols = vec![b"http/1.1".to_vec()];
    tokio_rustls::TlsAcceptor::from(Arc::new(server_config))
}

struct TlsListener {
    tcp: tokio::net::TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
}

impl axum::serve::Listener for TlsListener {
    type Io = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let Ok((stream, addr)) = self.tcp.accept().await else {
                continue;
            };
            match self.acceptor.accept(stream).await {
                Ok(tls) => return (tls, addr),
                // A client that connects and drops before the handshake completes
                // (or any other handshake failure) must not take the whole mock
                // server down.
                Err(_) => continue,
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.tcp.local_addr()
    }
}

/// Bind a self-signed HTTPS listener on `127.0.0.1`, hand `build_app` its base URL
/// (`https://127.0.0.1:port`) so routes can echo it back into a discovery document
/// before anything is actually listening, then spawn the server in the background.
/// Returns the same base URL. Callers reach it with [`federation_test_client`].
pub(super) async fn serve_https(build_app: impl FnOnce(String) -> axum::Router) -> String {
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = tcp.local_addr().expect("local_addr");
    let base_url = format!("https://{addr}");
    let app = build_app(base_url.clone());

    let acceptor = tls_acceptor();
    tokio::spawn(async move {
        let listener = TlsListener { tcp, acceptor };
        axum::serve(listener, app).await.expect("tls mock upstream");
    });

    base_url
}

/// What the raw fixture does after writing its response.
#[derive(Clone, Copy)]
pub(super) enum AfterWrite {
    /// Shut the connection down: the client sees end of stream.
    Close,
    /// Keep the connection open and send nothing more. A reader that waits for
    /// the declared length or for end of stream never returns; only a bound on
    /// the bytes it has already read can end it.
    HoldOpen,
}

/// A fixture that completes a real TLS handshake, reads one request head, and then
/// writes the bytes `build` returns and does [`AfterWrite`]. `build` receives the
/// fixture's own base URL, so a discovery document can name its own endpoints.
///
/// Returns the base URL, `https://fixture.test:port`, and the socket address to
/// hand to [`FixtureResolver`]. The name is what makes the request go through that
/// resolver; an IP literal would not.
pub(super) async fn serve_raw_https(
    build: impl FnOnce(&str) -> Vec<u8>,
    after: AfterWrite,
) -> (String, SocketAddr) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind raw fixture");
    let addr = tcp.local_addr().expect("local_addr");
    let base_url = format!("https://{RAW_FIXTURE_HOST}:{}", addr.port());
    let response = Arc::new(build(&base_url));
    let acceptor = tls_acceptor();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = tcp.accept().await else {
                continue;
            };
            let acceptor = acceptor.clone();
            let response = Arc::clone(&response);
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(stream).await else {
                    return;
                };
                // Read until the request head ends; a GET has no body to read.
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    match tls.read(&mut byte).await {
                        Ok(1) => head.push(byte[0]),
                        _ => return,
                    }
                }
                let _ = tls.write_all(&response).await;
                let _ = tls.flush().await;
                match after {
                    AfterWrite::Close => {
                        let _ = tls.shutdown().await;
                    }
                    AfterWrite::HoldOpen => {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    }
                }
            });
        }
    });

    (base_url, addr)
}

/// Resolves [`RAW_FIXTURE_HOST`] to the loopback port a raw fixture listens on, and
/// nothing else. This is the resolver difference a fixture test makes; production's
/// [`sui_id::resolver::ValidatingResolver`] refuses loopback by design (RFC 134 D2),
/// and the row that proves it still does is cited, not repeated, in the harness.
pub(super) struct FixtureResolver {
    pub(super) addr: SocketAddr,
}

impl reqwest::dns::Resolve for FixtureResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let answer = (name.as_str() == RAW_FIXTURE_HOST).then_some(self.addr);
        Box::pin(async move {
            match answer {
                Some(addr) => {
                    let addrs: reqwest::dns::Addrs = Box::new(std::iter::once(addr));
                    Ok(addrs)
                }
                None => Err("FixtureResolver answers only for the fixture host".into()),
            }
        })
    }
}

/// The federation client, built by the production constructor with the caller's
/// resolver and [`mock_root`] as its extra trusted root.
///
/// For tests that reach a [`serve_https`] mock by IP literal, pass the production
/// resolver: the IP literal never reaches DNS, so the client then differs from
/// production in its extra root alone.
pub(super) fn federation_test_client<R>(resolver: R) -> reqwest::Client
where
    R: reqwest::dns::Resolve + 'static,
{
    sui_id::egress::build_federation_client_for_tests(Arc::new(resolver), mock_root())
}
