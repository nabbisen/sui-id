//! RFC 096-B1 stage 6c — the claim's extended match set (RFC 096
//! `:597-599`), the browser binding specifically, the `:603-604`
//! error-response path, and F05's no-side-effect property, all driven
//! against the real router rather than asserted at the unit level alone.

use super::common::*;
use super::federation_fail_closed::{add_federation_provider_config, linked_user, mock_upstream};
use axum::body::Body;
use axum::http::{Method, Request, header};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use sui_id::{AppState, build_router};
use tower::ServiceExt;

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn base_state() -> AppState {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    state
}

struct Started {
    fed_state_cookie: String,
    browser_binding_cookie: String,
    state_param: String,
}

async fn start(state: &AppState, slot: &super::federation_fail_closed::NonceSlot) -> Started {
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
    let fed_state_cookie =
        extract_set_cookie(resp.headers(), "sui_id_fed_state").expect("state cookie");
    let browser_binding_cookie =
        extract_set_cookie(resp.headers(), "sui_id_fed_browser_binding").expect("binding cookie");
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
    let nonce_param = location
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("nonce="))
        .expect("nonce parameter");
    let nonce_param = percent_encoding::percent_decode_str(nonce_param)
        .decode_utf8()
        .expect("nonce is valid UTF-8")
        .into_owned();
    *slot.lock().expect("lock") = Some(nonce_param);
    Started {
        fed_state_cookie,
        browser_binding_cookie,
        state_param,
    }
}

/// Drives `/callback` with an explicit, caller-supplied `Cookie:` header
/// instead of the started attempt's own real browser-binding cookie —
/// every other test's `callback()` round-trips the correct one; this is
/// the one helper deliberately letting a test substitute a wrong or
/// absent value.
async fn callback_with_cookie_header(
    state: &AppState,
    state_param: &str,
    cookie_header: &str,
) -> axum::http::Response<Body> {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/federated/callback?code=the-code&state={state_param}"
                ))
                .header(header::COOKIE, cookie_header)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback")
}

fn location_of(resp: &axum::http::Response<Body>) -> String {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

/// `state_param`, as extracted from the upstream-redirect URL, is still
/// percent-encoded (`federated_start` built that URL with
/// `utf8_percent_encode`) -- every other test in this file only ever
/// re-embeds it in a *second* URL's query string, which `axum`'s own
/// `Query` extractor decodes once on the way in, recovering the original
/// value server-side without any test code having to do it. This helper
/// hashes the value directly, bypassing that second URL round-trip, so it
/// must decode it itself first to match what the server actually hashed.
async fn attempt_status(state: &AppState, state_param: &str) -> String {
    let decoded = percent_encoding::percent_decode_str(state_param)
        .decode_utf8()
        .expect("state is valid UTF-8")
        .into_owned();
    state
        .db
        .with_read(move |read| {
            Ok(read
                .prepare(
                    "SELECT status FROM federation_login_attempt \
                     WHERE state_sha256 = ?1",
                )?
                .query_row([sha2::Sha256::digest(decoded.as_bytes()).to_vec()], |r| {
                    r.get::<_, String>(0)
                })?)
        })
        .await
        .expect("attempt row for this state")
}

use sha2::Digest;

/// A `federation_provider` row with an explicit `provision_mode`, and
/// deliberately no `federation_link` row -- `linked_user` always creates
/// both the provider row and a link; this is the unlinked case the
/// no-side-effect test needs, duplicated from the same idiom
/// `tests/e2e/r096_b1_stage5.rs` already established for exactly this.
async fn seed_unlinked_provider(state: &AppState, issuer: &str) {
    exec(
        state,
        format!(
            "INSERT INTO federation_provider \
             (id, slug, display_name, issuer, client_id, provision_mode, enabled, \
              created_at, updated_at) \
             VALUES ('{}', 'up', 'Up', '{issuer}', 'client', 'link_only', 1, \
             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;
}

async fn row_counts(state: &AppState) -> (i64, i64, i64) {
    state
        .db
        .with_read(|read| {
            let users: i64 = read
                .prepare("SELECT COUNT(*) FROM users")?
                .query_row([], |r| r.get(0))?;
            let links: i64 = read
                .prepare("SELECT COUNT(*) FROM federation_link")?
                .query_row([], |r| r.get(0))?;
            let sessions: i64 = read
                .prepare("SELECT COUNT(*) FROM sessions")?
                .query_row([], |r| r.get(0))?;
            Ok((users, links, sessions))
        })
        .await
        .expect("row counts")
}

/// A mock upstream whose `/token` route only counts hits -- the request
/// body is irrelevant, since the point of each test using this is that
/// the route must never be reached at all.
async fn mock_upstream_counting_token_hits() -> (
    String,
    super::federation_fail_closed::NonceSlot,
    Arc<AtomicUsize>,
) {
    let slot = nonce_slot_for_counting();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_route = hits.clone();
    let issuer = super::tls_mock::serve_https(move |issuer| {
        let hits = hits_for_route.clone();
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
                            "jwks_uri": format!("{issuer}/jwks"),
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
                "/jwks",
                axum::routing::get(|| async { axum::Json(serde_json::json!({ "keys": [] })) }),
            )
            .route(
                "/token",
                axum::routing::post(move || {
                    let hits = hits.clone();
                    async move {
                        hits.fetch_add(1, Ordering::SeqCst);
                        axum::Json(serde_json::json!({ "access_token": "at" }))
                    }
                }),
            )
    })
    .await;
    (issuer, slot, hits)
}

/// A standalone nonce slot: this mock's own `/token` never reads it (it
/// never runs, in every test that uses this mock), but `start()` still
/// needs one to fill after `/start`.
fn nonce_slot_for_counting() -> super::federation_fail_closed::NonceSlot {
    std::sync::Arc::new(std::sync::Mutex::new(None))
}

// ── The browser binding (RFC 096 `:598`) ────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_browser_binding_cookie_is_refused_as_binding_mismatch() {
    let (issuer, nonce_slot_v) = mock_upstream().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;
    let started = start(&state, &nonce_slot_v).await;

    let resp = callback_with_cookie_header(
        &state,
        &started.state_param,
        &format!(
            "sui_id_fed_state={}; sui_id_fed_browser_binding=not-the-real-value",
            started.fed_state_cookie
        ),
    )
    .await;

    assert_eq!(
        location_of(&resp),
        "/admin/login?fed_error=binding_mismatch"
    );
    // The claim never succeeded: the row is exactly as `/start` left it.
    assert_eq!(
        attempt_status(&state, &started.state_param).await,
        "pending"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_absent_browser_binding_cookie_is_refused_the_same_way_as_a_wrong_one() {
    let (issuer, nonce_slot_v) = mock_upstream().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;
    let started = start(&state, &nonce_slot_v).await;

    // No `sui_id_fed_browser_binding` in the Cookie header at all.
    let resp = callback_with_cookie_header(
        &state,
        &started.state_param,
        &format!("sui_id_fed_state={}", started.fed_state_cookie),
    )
    .await;

    assert_eq!(
        location_of(&resp),
        "/admin/login?fed_error=binding_mismatch"
    );
    assert_eq!(
        attempt_status(&state, &started.state_param).await,
        "pending"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_real_browser_binding_cookie_lets_the_claim_succeed() {
    let (issuer, nonce_slot_v) = mock_upstream().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    let user_id = linked_user(&state, &issuer).await;
    let started = start(&state, &nonce_slot_v).await;

    let resp = callback_with_cookie_header(
        &state,
        &started.state_param,
        &format!(
            "sui_id_fed_state={}; sui_id_fed_browser_binding={}",
            started.fed_state_cookie, started.browser_binding_cookie
        ),
    )
    .await;

    assert_eq!(location_of(&resp), "/admin");
    let _ = user_id;
    // RFC 096-B1 stage 6c: the claim runs before token exchange, leaving
    // the row `exchanging` -- not `pending`, which would mean the claim
    // never ran at all. RFC 094 `:548`, amended 2026-10-15 (stage 6d):
    // F01's own `exchanging -> completed` transition now runs inside
    // L04's own transaction, atomic with the session this successful,
    // no-local-MFA callback just created -- so by the time this redirect
    // is observed, the row has already moved past `exchanging` too.
    assert_eq!(
        attempt_status(&state, &started.state_param).await,
        "completed"
    );
}

/// RFC 096 `:597-599`'s own ordering, checked directly rather than only
/// through its redirect: a claim refused for a wrong browser binding must
/// mean the upstream's `/token` endpoint is *never called* -- the claim
/// now runs before the exchange, not after it. This is the oracle mutation
/// evidence item 9's first case is checked against: moving the claim back
/// to its pre-stage-6c position (after discovery and token exchange) would
/// make this specific assertion fail (hits == 1, not 0) while every
/// redirect-only test above kept passing unchanged.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_claim_refused_for_the_wrong_browser_binding_never_reaches_token_exchange() {
    let (issuer, nonce_slot_v, hits) = mock_upstream_counting_token_hits().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;
    let started = start(&state, &nonce_slot_v).await;

    let resp = callback_with_cookie_header(
        &state,
        &started.state_param,
        &format!(
            "sui_id_fed_state={}; sui_id_fed_browser_binding=not-the-real-value",
            started.fed_state_cookie
        ),
    )
    .await;

    assert_eq!(
        location_of(&resp),
        "/admin/login?fed_error=binding_mismatch"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "the upstream token endpoint must never be hit when the claim itself refuses"
    );
}

// ── The error-response path (RFC 096 `:603-604`) ────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_upstream_error_response_terminalizes_the_matching_attempt() {
    let (issuer, nonce_slot_v) = mock_upstream().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;
    let started = start(&state, &nonce_slot_v).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/federated/callback?error=access_denied&state={}",
                    started.state_param
                ))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback");

    assert_eq!(location_of(&resp), "/admin/login?fed_error=upstream");
    // Consumed without a token exchange, per `:603-604` -- `pending`, not
    // `exchanging`: this path never calls `claim` at all.
    assert_eq!(attempt_status(&state, &started.state_param).await, "failed");
    let rows = sui_id_store::repos::audit::recent(&state.db, 10)
        .await
        .expect("recent");
    assert!(
        rows.iter()
            .any(|r| r.action == "auth.federation.signin.upstream_failure"),
        "no upstream_failure event recorded: {rows:?}"
    );
}

/// `:603-604`'s own caution: an unauthenticated caller controls this
/// entire request, so an absent or unmatched `state` must not crash and
/// must not look any different from the matched case above -- otherwise
/// the response itself becomes an oracle for which state digests exist.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_upstream_error_response_with_no_matching_state_is_still_handled_uniformly() {
    let state = base_state().await;

    let with_garbage_state = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/callback?error=access_denied&state=not-a-real-state")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback");
    assert_eq!(
        location_of(&with_garbage_state),
        "/admin/login?fed_error=upstream"
    );

    let with_no_state = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/callback?error=access_denied")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback");
    assert_eq!(
        location_of(&with_no_state),
        "/admin/login?fed_error=upstream"
    );
}

// ── F05's no-side-effect property (dispatch item 6) ─────────────────────────

/// `link_required` (RFC 096 `:699-701`) is a post-claim denial that needs
/// no forged token: a correctly-signed response, from a provider with no
/// matching `federation_link` row and `provision_mode = 'link_only'`. F05
/// must terminalize the attempt and emit its event, and touch *nothing*
/// else -- no user, link or session row created, changed or removed.
/// Asserted by counting rows before and after, not by inspecting what the
/// handler happens to call.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_post_claim_denial_creates_or_changes_no_link_user_or_session_row() {
    let (issuer, nonce_slot_v) = mock_upstream().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    seed_unlinked_provider(&state, &issuer).await;
    let before = row_counts(&state).await;

    let started = start(&state, &nonce_slot_v).await;
    let resp = callback_with_cookie_header(
        &state,
        &started.state_param,
        &format!(
            "sui_id_fed_state={}; sui_id_fed_browser_binding={}",
            started.fed_state_cookie, started.browser_binding_cookie
        ),
    )
    .await;
    assert_eq!(location_of(&resp), "/admin/login?fed_error=link_required");
    assert_eq!(attempt_status(&state, &started.state_param).await, "failed");

    let after = row_counts(&state).await;
    assert_eq!(
        before, after,
        "no link/user/session row may change on this path"
    );
}
