//! RFC 096-A prerequisite: the hostile-provider harness, proven with two rows.
//!
//! The fixture is [`super::tls_mock::serve_raw_https`]: a real rustls handshake,
//! then bytes this test chose. The client is [`federation_test_client`], built by
//! the production constructor and differing in two named parameters: its resolver
//! ([`FixtureResolver`], which maps the fixture host to loopback) and one extra
//! trusted root (the mock's own certificate). Verification stays on.
//!
//! The third proof row, that the production client still refuses a name resolving
//! to loopback, is not repeated here. It is
//! `r134_step3_validating_resolver::the_real_egress_client_refuses_a_name_that_resolves_to_loopback`,
//! which passes against the production constructor unchanged.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

use super::common::*;
use super::tls_mock::{AfterWrite, FixtureResolver, federation_test_client, serve_raw_https};
use sui_id::build_router;

/// The fixture's discovery document, naming its own endpoints under `base`.
fn discovery_body(base: &str) -> String {
    serde_json::json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "authorization_response_iss_parameter_supported": true,
        "token_endpoint_auth_methods_supported": ["client_secret_basic"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "subject_types_supported": ["public"],
    })
    .to_string()
}

/// A well-formed response: status line, media type, and a `Content-Length` that
/// tells the truth about the body that follows.
fn honest_response(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

/// A response whose `Content-Length` claims ten mebibytes while the fixture sends
/// only `actual` bytes of body, then holds the connection open.
fn lying_length_response(claimed: usize, actual: usize) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {claimed}\r\n\r\n"
    )
    .into_bytes();
    bytes.extend(std::iter::repeat_n(b'x', actual));
    bytes
}

async fn seed_provider(state: &sui_id::AppState, issuer: &str) {
    let provider = uuid::Uuid::new_v4();
    state
        .db
        .with_conn({
            let sql = format!(
                "INSERT INTO federation_provider (id, slug, display_name, issuer, client_id, \
                 enabled, allowed_origins, created_at, updated_at) \
                 VALUES ('{provider}', 'up', 'Up', '{issuer}', 'client', 1, '', \
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');"
            );
            move |c| Ok(c.execute_batch(&sql)?)
        })
        .await
        .expect("seed provider");
}

async fn start_flow(state: &sui_id::AppState) -> axum::response::Response {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/up/start")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("start")
}

/// Row 1 — bytes arrive. A response the fixture fabricates reaches
/// `fetch_discovery` and is parsed into the endpoints it names, so the redirect
/// to the upstream authorization endpoint carries the fixture's own URL.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fabricated_bytes_reach_fetch_discovery() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let (base, addr) = serve_raw_https(
        |base| honest_response(&discovery_body(base)),
        AfterWrite::Close,
    )
    .await;
    state.http_client = Arc::new(federation_test_client(FixtureResolver { addr }));
    seed_provider(&state, &base).await;

    let resp = start_flow(&state).await;
    assert!(resp.status().is_redirection(), "status {}", resp.status());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        location.starts_with(&format!("{base}/authorize?")),
        "the redirect must go to the endpoint the fabricated document named: {location}"
    );
}

/// Control for row 2: the same path, with a document well inside the cap, succeeds.
/// Without this, a failure in row 2 could be the fixture rather than the cap.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_honest_document_inside_the_cap_is_accepted() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let (base, addr) = serve_raw_https(
        |base| honest_response(&discovery_body(base)),
        AfterWrite::Close,
    )
    .await;
    state.http_client = Arc::new(federation_test_client(FixtureResolver { addr }));
    seed_provider(&state, &base).await;

    let resp = start_flow(&state).await;
    assert!(resp.status().is_redirection(), "status {}", resp.status());
}

/// Row 2 — the bounds still apply on bytes read. The declared length is ten
/// mebibytes and the fixture sends a hundred kibibytes and then holds the
/// connection open. Only the production byte cap in `response_bounds` can end the
/// read: the declared length is never reached and end of stream never arrives. The
/// assertion is that the request fails **promptly** and does not redirect.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lying_content_length_is_capped_on_bytes_read_through_the_production_path() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let (base, addr) = serve_raw_https(
        |_| lying_length_response(10 * 1024 * 1024, 100 * 1024),
        AfterWrite::HoldOpen,
    )
    .await;
    state.http_client = Arc::new(federation_test_client(FixtureResolver { addr }));
    seed_provider(&state, &base).await;

    let started = Instant::now();
    let resp = start_flow(&state).await;
    let elapsed = started.elapsed();

    assert!(
        !resp.status().is_redirection(),
        "an oversized document must not produce a redirect: {}",
        resp.status()
    );
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    // The client's own read timeout is 5 s and its total deadline is 10 s. Returning
    // well before either proves the cap ended the read, not the timeout.
    assert!(
        elapsed < Duration::from_secs(4),
        "the cap must end the read on bytes received; took {elapsed:?}"
    );
}
