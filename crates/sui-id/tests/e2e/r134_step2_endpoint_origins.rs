//! RFC 134 step 2 (D3) — discovery is untrusted input.
//!
//! `ValidatedDiscovery`'s own unit tests (`crates/sui-id/src/http/
//! discovery.rs`) cover every specific rule (each endpoint independently,
//! `http://` rejected, port equivalence, the empty-set default) against
//! the pure validation function — no network needed, since `validate()`
//! performs none. What a unit test on that function *cannot* prove is
//! that the real HTTP handlers actually reach it before making any
//! request with the rejected endpoint. That is what this file proves,
//! against the real router, over real (self-signed) TLS.

use super::common::{complete_setup_and_login, extract_set_cookie, test_app};
use super::federation_fail_closed::federated_signin;
use super::tls_mock::{insecure_test_client, serve_https};
use axum::body::Body;
use axum::http::{Method, Request, header};
use base64ct::Encoding;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use sui_id::AppState;
use sui_id::build_router;
use tower::ServiceExt;

const DISCOVERY_ORIGIN_REJECTED: &str = "/admin/login?fed_error=discovery_origin";

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

/// Seed a `federation_provider` row naming `issuer`, with the slug
/// `federated_signin` expects ("up"). `allowed_origins` is the raw,
/// space-separated column value — empty means "the issuer's origin
/// alone".
async fn seed_provider(state: &AppState, issuer: &str, allowed_origins: &str) {
    let provider = uuid::Uuid::new_v4();
    exec(
        state,
        format!(
            "INSERT INTO federation_provider \
             (id, slug, display_name, issuer, client_id, enabled, allowed_origins, \
              created_at, updated_at) \
             VALUES ('{provider}', 'up', 'Up', '{issuer}', 'client', 1, '{allowed_origins}', \
             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');"
        ),
    )
    .await;
}

/// **The central test.** A discovery document whose `token_endpoint`
/// names an origin outside the allowed set (here: outside the default
/// empty-set, which is the issuer's own origin alone) must result in no
/// request being made to it — not merely a failed sign-in, which could
/// also be true after the secret was already POSTed to the wrong place.
///
/// `ValidatedDiscovery::validate` checks all three endpoints
/// unconditionally, so a bad `token_endpoint` fails the whole document
/// even though `/start` itself only reads `authorization_endpoint` — the
/// rejection happens at `/start`, before any upstream redirect and
/// before a state cookie exists, which is earlier than this property
/// strictly requires but is the stronger, correct behavior: a provider
/// whose discovery document is wrong in any way is not used at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_endpoint_outside_the_allowed_set_receives_no_request() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = Arc::new(insecure_test_client());

    let token_hits = Arc::new(AtomicUsize::new(0));
    let hits_for_route = token_hits.clone();
    let rogue_token_endpoint = serve_https(move |_base| {
        axum::Router::new().route(
            "/token",
            axum::routing::post(move || {
                let hits = hits_for_route.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    axum::Json(serde_json::json!({ "access_token": "at" }))
                }
            }),
        )
    })
    .await;

    // The discovery server IS the issuer (so authorization_endpoint is
    // in-set); its token_endpoint points at the separate, out-of-set
    // rogue server above.
    let discovery_issuer = serve_https(move |base| {
        let token_endpoint = format!("{rogue_token_endpoint}/token");
        axum::Router::new().route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || {
                let base = base.clone();
                let token_endpoint = token_endpoint.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "authorization_endpoint": format!("{base}/authorize"),
                        "token_endpoint": token_endpoint,
                    }))
                }
            }),
        )
    })
    .await;
    seed_provider(&state, &discovery_issuer, "").await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/up/start")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("start");

    assert_eq!(
        token_hits.load(Ordering::SeqCst),
        0,
        "the client_secret must never have been POSTed to the rogue token_endpoint"
    );
    assert!(resp.status().is_redirection(), "status {}", resp.status());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, DISCOVERY_ORIGIN_REJECTED);
    assert!(
        extract_set_cookie(resp.headers(), "sui_id_fed_state").is_none(),
        "no state cookie -- the upstream redirect never happened either"
    );
}

/// `authorization_endpoint` outside the allowed set is rejected at
/// `/start`, before the state cookie that would carry the user to the
/// upstream is even issued.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authorization_endpoint_outside_the_allowed_set_is_rejected_at_start() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = Arc::new(insecure_test_client());

    let discovery_issuer = serve_https(move |_base| {
        axum::Router::new().route(
            "/.well-known/openid-configuration",
            axum::routing::get(|| async move {
                axum::Json(serde_json::json!({
                    "authorization_endpoint": "https://evil.example.com/authorize",
                    "token_endpoint": "https://evil.example.com/token",
                }))
            }),
        )
    })
    .await;
    seed_provider(&state, &discovery_issuer, "").await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/up/start")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("start");

    assert!(resp.status().is_redirection(), "status {}", resp.status());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, DISCOVERY_ORIGIN_REJECTED);
    assert!(
        extract_set_cookie(resp.headers(), "sui_id_fed_state").is_none(),
        "no state cookie must be issued for a rejected discovery document"
    );
}

/// A provider configured with a non-empty `allowed_origins` that includes
/// a genuine second origin: the discovery document's endpoints split
/// across both are accepted, and sign-in succeeds normally.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_configured_origin_is_accepted_end_to_end() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = Arc::new(insecure_test_client());

    let token_hits = Arc::new(AtomicUsize::new(0));
    let hits_for_route = token_hits.clone();
    let claims = serde_json::json!({ "sub": "fed-sub-second-origin" });
    let id_token = format!(
        "{}.{}.sig",
        base64ct::Base64UrlUnpadded::encode_string(br#"{"alg":"none"}"#),
        base64ct::Base64UrlUnpadded::encode_string(claims.to_string().as_bytes())
    );
    let second_origin = serve_https(move |_base| {
        axum::Router::new().route(
            "/token",
            axum::routing::post(move || {
                let hits = hits_for_route.clone();
                let id_token = id_token.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    axum::Json(serde_json::json!({ "access_token": "at", "id_token": id_token }))
                }
            }),
        )
    })
    .await;

    let second_origin_for_discovery = second_origin.clone();
    let discovery_issuer = serve_https(move |base| {
        let token_endpoint = format!("{second_origin_for_discovery}/token");
        axum::Router::new().route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || {
                let base = base.clone();
                let token_endpoint = token_endpoint.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "authorization_endpoint": format!("{base}/authorize"),
                        "token_endpoint": token_endpoint,
                    }))
                }
            }),
        )
    })
    .await;
    let allowed = format!("{discovery_issuer} {second_origin}");
    seed_provider(&state, &discovery_issuer, &allowed).await;

    let outcome = federated_signin(&state).await;

    assert_eq!(
        token_hits.load(Ordering::SeqCst),
        1,
        "the real token_endpoint, on the second configured origin, was used"
    );
    // No federation_link row was seeded for this sub, so this is an
    // unknown upstream identity under the default link_only provision
    // mode -- the link flow, not a session. The property this test is
    // for is that discovery validation *passed* (unlike the central
    // test's rejection), proven by reaching the real token_endpoint and
    // not landing on a fed_error redirect.
    assert_eq!(outcome.location, "/auth/federated/link");
    assert!(outcome.session_cookie.is_none());
}
