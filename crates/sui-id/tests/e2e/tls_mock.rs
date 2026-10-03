//! RFC 134 D3: a self-signed TLS mock upstream for federation e2e tests,
//! now that discovery and its endpoints must be `https`.
//!
//! Implements `axum::serve::Listener` directly rather than terminating
//! TLS by hand: a custom `Listener` is the one seam axum exposes for
//! this, and `axum::serve` already knows how to drive the resulting
//! `AsyncRead + AsyncWrite` stream the same way it drives a plain TCP
//! one.

use std::sync::Arc;

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
                // A client that connects and drops before the handshake
                // completes (or any other handshake failure) must not
                // take the whole mock server down.
                Err(_) => continue,
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.tcp.local_addr()
    }
}

/// Bind a self-signed HTTPS listener on `127.0.0.1`, hand `build_app` its
/// base URL (`https://127.0.0.1:port`) so routes can echo it back into a
/// discovery document before anything is actually listening, then spawn
/// the server in the background. Returns the same base URL. Callers that
/// drive requests at it need a client that accepts the self-signed cert —
/// see [`insecure_test_client`].
pub(super) async fn serve_https(build_app: impl FnOnce(String) -> axum::Router) -> String {
    // Normally installed once from `main()` (`install_rustls_crypto_provider`,
    // never reached by `test_app()`'s lighter setup); needed here because
    // this is the first thing in the e2e suite to perform a real TLS
    // handshake, server or client side. Idempotent.
    sui_id::startup::install_rustls_crypto_provider();
    let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()])
        .expect("generate self-signed cert");
    let key_der = rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());

    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert.cert.der().clone()], key_der.into())
        .expect("build rustls ServerConfig");
    // The real federation egress client is `.http1_only()` (RFC 134 D1);
    // match that in ALPN so there is no h2-vs-h1 mismatch to debug.
    server_config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = tcp.local_addr().expect("local_addr");
    let base_url = format!("https://{addr}");
    let app = build_app(base_url.clone());

    tokio::spawn(async move {
        let listener = TlsListener { tcp, acceptor };
        axum::serve(listener, app).await.expect("tls mock upstream");
    });

    base_url
}

/// A `reqwest::Client` with the same policy as the real federation egress
/// client, except it accepts any TLS certificate — the self-signed one
/// [`serve_https`] generates fresh per test has no CA anything else would
/// trust. Test-only; never used by production code (RFC 134 D1's "one
/// client" is `crate::egress::build_federation_client`, untouched).
pub(super) fn insecure_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .http1_only()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("build insecure test client")
}
