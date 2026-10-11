//! RFC 096-B1 stage 6a, item 2 — RFC 096 `:600-601`: *"The callback
//! cookie is expired on every terminal path."* Both federation cookies,
//! `sui_id_fed_state` and `sui_id_fed_browser_binding`, on four terminal
//! paths of genuinely different shapes: an early refusal before
//! `fed_state` is even unsealed, a mid-exchange failure (the token POST
//! itself failing), the `link_required` decision, and the success path.

use super::common::*;
use super::federation_fail_closed::{add_federation_provider_config, linked_user, mock_upstream};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;
use tower::ServiceExt;

const STATE_COOKIE_NAME: &str = "sui_id_fed_state";
const BROWSER_BINDING_COOKIE_NAME: &str = "sui_id_fed_browser_binding";

/// Both federation cookies, as sent in `/start`'s own response — present
/// on every real callback request, which is why every test here drives a
/// real `/start` first rather than hand-assembling a `Cookie:` header.
/// `CookieJar::remove` only emits a removal `Set-Cookie` for a cookie the
/// jar actually extracted from the incoming request (`cookie` crate's
/// own `original_cookies` check) — a request that never carried the
/// cookie in the first place has nothing to expire, so skipping `/start`
/// would make this assertion vacuous, not merely unrealistic.
struct StartCookies {
    state_cookie: String,
    browser_binding_cookie: String,
    state_param: String,
}

/// `nonce_slot` is `None` for the two tests whose mock never reaches
/// `/token` through `federation_fail_closed::mock_upstream`'s own nonce-
/// gated handler (the upstream-error path returns before token exchange;
/// the mid-exchange-failure mock is this file's own, simpler server) --
/// `Some` for the two that drive `federation_fail_closed::mock_upstream`
/// all the way through, which needs the real nonce fed back to it the
/// same way `federation_fail_closed::start` does.
async fn start(
    state: &sui_id::AppState,
    nonce_slot: Option<&super::federation_fail_closed::NonceSlot>,
) -> StartCookies {
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
    assert!(resp.status().is_redirection(), "start: {}", resp.status());
    let state_cookie = extract_set_cookie(resp.headers(), STATE_COOKIE_NAME).expect("state cookie");
    let browser_binding_cookie = extract_set_cookie(resp.headers(), BROWSER_BINDING_COOKIE_NAME)
        .expect("browser binding cookie");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("upstream redirect")
        .to_owned();
    let state_param = location
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("state="))
        .expect("state parameter")
        .to_owned();
    if let Some(slot) = nonce_slot {
        let nonce_param = location
            .split(['?', '&'])
            .find_map(|kv| kv.strip_prefix("nonce="))
            .expect("nonce parameter");
        let nonce_param = percent_encoding::percent_decode_str(nonce_param)
            .decode_utf8()
            .expect("nonce is valid UTF-8")
            .into_owned();
        *slot.lock().expect("lock") = Some(nonce_param);
    }
    StartCookies {
        state_cookie,
        browser_binding_cookie,
        state_param,
    }
}

async fn callback_with_cookies(
    state: &sui_id::AppState,
    cookies: &StartCookies,
    query_suffix: &str,
) -> axum::http::Response<Body> {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/federated/callback?state={}{query_suffix}",
                    cookies.state_param
                ))
                .header(
                    header::COOKIE,
                    format!(
                        "{STATE_COOKIE_NAME}={}; {BROWSER_BINDING_COOKIE_NAME}={}",
                        cookies.state_cookie, cookies.browser_binding_cookie
                    ),
                )
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback")
}

/// Stage 6a review C1, carried into stage 6b: `Max-Age=0` alone proves
/// the cookie is expired, but not *which* cookie a browser treats this as
/// -- that is `Path` (plus `Domain`, unused here), not the name or value.
/// A removal with no `Path` at all relies on the browser's default path
/// (derived from the request path) coincidentally matching the cookie's
/// own explicit `Path=/auth/federated`; asserting the explicit attribute
/// is what actually pins the requirement RFC 096-B1 F04 depends on,
/// rather than today's accidental agreement between the two.
fn assert_cookie_expired(headers: &axum::http::HeaderMap, name: &str) {
    let set_cookie = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with(&format!("{name}=")))
        .unwrap_or_else(|| panic!("no Set-Cookie for {name} at all"));
    assert!(
        set_cookie.contains("Max-Age=0"),
        "{name} not expired: {set_cookie}"
    );
    assert!(
        set_cookie.contains("Path=/auth/federated"),
        "{name} removal carries no explicit Path=/auth/federated: {set_cookie}"
    );
}

fn assert_both_federation_cookies_expired(headers: &axum::http::HeaderMap) {
    assert_cookie_expired(headers, STATE_COOKIE_NAME);
    assert_cookie_expired(headers, BROWSER_BINDING_COOKIE_NAME);
}

/// `federation_fail_closed::linked_user` seeds the `federation_provider`
/// row *and* a link in one call; the `link_required` test needs the
/// provider row without the link, since an existing link is a different
/// decision entirely (`ExistingLink`, not `LinkRequired`).
async fn seed_provider_row_only(state: &sui_id::AppState, issuer: &str) {
    state
        .db
        .with_conn({
            let issuer = issuer.to_owned();
            move |c| {
                Ok(c.execute_batch(&format!(
                    "INSERT INTO federation_provider \
                     (id, slug, display_name, issuer, client_id, enabled, created_at, updated_at) \
                     VALUES ('{}', 'up', 'Up', '{issuer}', 'client', 1, \
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    uuid::Uuid::new_v4()
                ))?)
            }
        })
        .await
        .expect("seed provider row");
}

/// Shape 1: an early refusal, before `fed_state` is even unsealed —
/// `federated_callback`'s very first check (`q.error`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn both_cookies_expire_on_the_upstream_error_path() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    // Never reaches `/token` (the error check is `federated_callback`'s
    // very first line), but `start` is given the slot anyway so a future
    // change to that ordering fails loudly rather than silently.
    let cookies = start(&state, Some(&nonce_slot)).await;
    let resp = callback_with_cookies(&state, &cookies, "&error=access_denied").await;

    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/login?fed_error=upstream");
    assert_both_federation_cookies_expired(resp.headers());
}

/// Shape 2: a mid-exchange failure — the token endpoint itself rejects
/// the POST. Discovery must succeed to reach this (it is what names the
/// token endpoint), so this mock is its own, minimal, discovery-only
/// server with a `/token` route that always answers 500 — no JWKS, no
/// signing, since the flow never gets that far.
async fn mock_upstream_with_failing_token_endpoint() -> String {
    super::tls_mock::serve_https(move |issuer| {
        axum::Router::new()
            .route(
                "/.well-known/openid-configuration",
                axum::routing::get(move || {
                    let issuer = issuer.clone();
                    async move {
                        axum::Json(serde_json::json!({
                            "issuer": issuer,
                            "authorization_endpoint": format!("{issuer}/authorize"),
                            "token_endpoint": format!("{issuer}/token"),
                            "response_types_supported": ["code"],
                            "code_challenge_methods_supported": ["S256"],
                            "authorization_response_iss_parameter_supported": true,
                            "token_endpoint_auth_methods_supported": ["client_secret_basic"],
                            "id_token_signing_alg_values_supported": ["RS256"],
                            "subject_types_supported": ["public"],
                        }))
                    }
                }),
            )
            .route(
                "/token",
                axum::routing::post(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
            )
    })
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn both_cookies_expire_on_a_mid_exchange_failure() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let issuer = mock_upstream_with_failing_token_endpoint().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let cookies = start(&state, None).await;
    let resp = callback_with_cookies(&state, &cookies, "&code=the-code").await;

    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/login?fed_error=token_exchange");
    assert_both_federation_cookies_expired(resp.headers());
}

/// Shape 3: the `link_required` decision — an unlinked identity under
/// `link_only`, reached only after the entire new validation pipeline
/// and the legacy decode both succeed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn both_cookies_expire_on_the_link_required_path() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    seed_provider_row_only(&state, &issuer).await;
    // Deliberately no link -- this is the unknown-identity path.

    let cookies = start(&state, Some(&nonce_slot)).await;
    let resp = callback_with_cookies(&state, &cookies, "&code=the-code").await;

    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/login?fed_error=link_required");
    assert_both_federation_cookies_expired(resp.headers());
}

/// Shape 4: the success path — the one the dispatch singles out, since
/// before this stage it removed only `STATE_COOKIE`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn both_cookies_expire_on_the_success_path() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let cookies = start(&state, Some(&nonce_slot)).await;
    let resp = callback_with_cookies(&state, &cookies, "&code=the-code").await;

    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin");
    assert!(
        extract_set_cookie(resp.headers(), "sui_id_session").is_some(),
        "the harness reaches a session"
    );
    assert_both_federation_cookies_expired(resp.headers());
}
