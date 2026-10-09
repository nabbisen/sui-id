//! RFC 096-A stage 9: RFC 096 `:74-79`'s well-known derivation vectors,
//! exercised end to end against the real `fetch_discovery`
//! (`handlers/federation.rs`, which 096-A may not modify but may
//! exercise over real TLS). Stage 8's own package argued the derivation
//! is structurally correct because `fetch_discovery` only ever appends
//! `/.well-known/openid-configuration` after the issuer's own path and
//! never inserts it before one -- true, and, until this file, unproven:
//! nothing fetched discovery from a *path* issuer at all.
//!
//! Each test's mock server answers at exactly one path. That is the
//! proof shape this file relies on throughout: the implementation lives
//! in a file this RFC may not touch, so there is no line to mutate here
//! -- the mock's own narrowness is what makes "the request landed
//! exactly where the RFC says it must" demonstrable at all. A request
//! to any other path 404s, and a 404 fails `fetch_discovery` before
//! `ValidatedDiscovery::validate` ever runs, surfacing as no state
//! cookie (the generic `Err(e)` branch in `federated_start`, not the
//! `is_invalid()` one `DISCOVERY_ORIGIN_REJECTED` uses).

use super::common::{complete_setup_and_login, extract_set_cookie, test_app};
use super::tls_mock::{federation_test_client, serve_https};
use axum::body::Body;
use axum::http::{Method, Request};
use std::sync::Arc;
use sui_id::AppState;
use sui_id::build_router;
use tower::ServiceExt;

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

/// Seeds a `federation_provider` row naming `issuer`, slug `up`, with the
/// issuer's own origin as the sole allowed one (empty `allowed_origins`).
async fn seed_provider(state: &AppState, issuer: &str) {
    let provider = uuid::Uuid::new_v4();
    exec(
        state,
        format!(
            "INSERT INTO federation_provider \
             (id, slug, display_name, issuer, client_id, enabled, allowed_origins, \
              created_at, updated_at) \
             VALUES ('{provider}', 'up', 'Up', '{issuer}', 'client', 1, '', \
             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');"
        ),
    )
    .await;
}

/// A full, valid discovery document naming `issuer` as both its own
/// `issuer` field and the base of its two endpoints -- satisfies every
/// unconditional rule `ValidatedDiscovery::validate` checks (stage 8), so
/// a successful fetch here is attributable to the well-known path being
/// right, not to some unrelated validation failure masking it.
fn discovery_json(issuer: &str) -> serde_json::Value {
    serde_json::json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "authorization_response_iss_parameter_supported": true,
        "token_endpoint_auth_methods_supported": ["client_secret_basic"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "subject_types_supported": ["public"],
    })
}

async fn start(state: &AppState) -> axum::response::Response {
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

/// Succeeded discovery is observable as the signed `sui_id_fed_state`
/// cookie `federated_start` only ever sets after `ValidatedDiscovery::
/// validate` returns `Ok` -- the same signal `federation_fail_closed.rs`'s
/// `federated_signin` relies on, used bare here since these vectors stop
/// at discovery and never need the token exchange that follows it.
fn discovery_succeeded(resp: &axum::response::Response) -> bool {
    resp.status().is_redirection()
        && extract_set_cookie(resp.headers(), "sui_id_fed_state").is_some()
}

/// RFC 096 `:74-76`: a root issuer (`https://id.example`) derives
/// `https://id.example/.well-known/openid-configuration`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_root_issuer_derives_the_well_known_path_at_the_root() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = Arc::new(federation_test_client(sui_id::resolver::ValidatingResolver));

    let issuer = serve_https(move |base| {
        let doc = discovery_json(&base);
        axum::Router::new().route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || {
                let doc = doc.clone();
                async move { axum::Json(doc) }
            }),
        )
    })
    .await;
    seed_provider(&state, &issuer).await;

    let resp = start(&state).await;
    assert!(
        discovery_succeeded(&resp),
        "root-issuer well-known derivation must succeed: status {}",
        resp.status()
    );
}

/// RFC 096 `:76-78`: a path issuer (`https://id.example/tenant/a`)
/// derives `https://id.example/tenant/a/.well-known/openid-configuration`
/// -- the suffix appended *after* the issuer's own path, not inserted
/// before it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_path_issuer_derives_the_well_known_path_after_the_path() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = Arc::new(federation_test_client(sui_id::resolver::ValidatingResolver));

    let base = serve_https(move |base| {
        let issuer = format!("{base}/tenant/a");
        let doc = discovery_json(&issuer);
        axum::Router::new().nest(
            "/tenant/a",
            axum::Router::new().route(
                "/.well-known/openid-configuration",
                axum::routing::get(move || {
                    let doc = doc.clone();
                    async move { axum::Json(doc) }
                }),
            ),
        )
    })
    .await;
    let issuer = format!("{base}/tenant/a");
    seed_provider(&state, &issuer).await;

    let resp = start(&state).await;
    assert!(
        discovery_succeeded(&resp),
        "path-issuer well-known derivation must land at /tenant/a/.well-known/openid-configuration: status {}",
        resp.status()
    );
}

/// RFC 096 `:78-79`: for a path issuer, the RFC 8414-style
/// insert-before-path form (`https://id.example/.well-known/
/// openid-configuration/tenant/a`) is rejected -- proven here by serving
/// the document *only* at that form and nowhere else. If `fetch_discovery`
/// ever tried it, this test would see a successful sign-in; it must not.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_rfc_8414_insert_before_path_form_is_never_attempted_for_a_path_issuer() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = Arc::new(federation_test_client(sui_id::resolver::ValidatingResolver));

    let base = serve_https(move |base| {
        let issuer = format!("{base}/tenant/a");
        let doc = discovery_json(&issuer);
        axum::Router::new().route(
            "/.well-known/openid-configuration/tenant/a",
            axum::routing::get(move || {
                let doc = doc.clone();
                async move { axum::Json(doc) }
            }),
        )
    })
    .await;
    let issuer = format!("{base}/tenant/a");
    seed_provider(&state, &issuer).await;

    let resp = start(&state).await;
    assert!(
        !discovery_succeeded(&resp),
        "the insert-before-path form must never be requested, let alone accepted"
    );
}
