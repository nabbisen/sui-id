use super::*;

#[tokio::test]
async fn build_accepts_https_url() {
    let w = build("https://idp.example/").await.expect("build");
    let _ = w; // we just want it to construct without panicking.
}

#[tokio::test]
async fn build_accepts_https_with_port() {
    let w = build("https://idp.example:8443/").await.expect("build");
    let _ = w;
}

#[tokio::test]
async fn build_accepts_localhost_http() {
    let w = build("http://localhost:8080/")
        .await
        .expect("localhost http");
    let _ = w;
}

#[tokio::test]
async fn build_accepts_127_0_0_1_http() {
    // webauthn-rs requires a hostname for rp_id, not a raw IP address.
    // 127.0.0.1 passes our transport check (it's a loopback address)
    // but is rejected at the WebauthnBuilder level with an Err — which
    // is the correct behaviour: operators should use `localhost`, not the
    // numeric address, for local dev.  We test that we reach the builder
    // stage (i.e., our transport guard doesn't reject it) by checking that
    // the error, if any, is *not* a ConfigError.
    let r = build("http://127.0.0.1:8801/").await;
    match r {
        Ok(_) => {} // webauthn-rs accepted it — fine
        Err(CoreError::ConfigError(_)) => {
            panic!("127.0.0.1 http must not be rejected by our transport guard (RFC 011)")
        }
        Err(_) => {} // webauthn-rs rejected the IP as rp_id — expected
    }
}

// RFC 011: http on a non-localhost host must be rejected at startup.
#[tokio::test]
async fn build_rejects_http_on_public_host() {
    let r = build("http://idp.example/").await;
    assert!(
        r.is_err(),
        "http on a non-localhost host must be rejected (RFC 011)"
    );
    let err = r.unwrap_err();
    assert!(
        matches!(err, CoreError::ConfigError(_)),
        "expected ConfigError, got: {err}"
    );
}

#[tokio::test]
async fn build_rejects_url_without_host() {
    // file:// has no host — webauthn-rs (and our wrapper) reject this.
    let r = build("file:///etc/passwd").await;
    assert!(r.is_err());
}
