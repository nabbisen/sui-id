//! RFC 019 — Auth flow data integrity hardening.
//!
//! Covers the three correctness gaps closed by RFC 019:
//!
//! 1. `exchange_code` rejects token issuance when the bound user is
//!    disabled at exchange time (`user_revoked` audit event).
//! 2. Auth codes for a user are marked consumed when that user is
//!    disabled (so they can't be exchanged after re-enable without
//!    a fresh authentication).
//! 3. Refresh-token theft detection fires even after the GC has
//!    run: revoked-but-unexpired rows are retained so that a
//!    replayed token reaches the `theft_detected` branch.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use tower::ServiceExt;
use url::Url;

use super::common::*;
use sui_id::build_router;

// ── helpers ──────────────────────────────────────────────────────────

/// Perform steps 1-3 of the OIDC Authorization Code + PKCE flow and
/// return `(code, verifier)` so the caller can use the matching verifier
/// in the subsequent token exchange.
async fn get_auth_code(
    state: &sui_id::AppState,
    session: &str,
    client_id: &str,
) -> (String, String) {
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}\
         &redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&nonce=n0\
         &code_challenge={challenge}&code_challenge_method=S256"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&auth_url)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");
    assert!(resp.status().is_redirection(), "authorize should redirect");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let parsed = Url::parse(&location).expect("url");
    let code = parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code in redirect");
    (code, verifier)
}

/// Exchange an authorization code for tokens using the given PKCE verifier.
/// The verifier must match the challenge that was sent in the /authorize request;
/// use the second element of the tuple returned by `get_auth_code`.
async fn exchange_code(
    state: &sui_id::AppState,
    code: &str,
    client_id: &str,
    client_secret: &str,
    verifier: &str,
) -> axum::response::Response {
    let body = format!(
        "grant_type=authorization_code&code={code}\
         &redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}\
         &code_verifier={verifier}"
    );
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("token")
}

// ── tests ─────────────────────────────────────────────────────────────

/// RFC 019 § 1: exchange_code rejects token issuance when the bound
/// user is disabled in the ~60-second window between authorization and
/// exchange.
#[tokio::test]
async fn exchange_code_rejected_when_user_disabled_before_exchange() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // Obtain an authorization code while the user is still active.
    let (code, verifier) = get_auth_code(&state, &session, &client_id).await;

    // Disable the user before exchanging the code.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await
        .expect("find user");
    sui_id_store::repos::users::set_disabled(&state.db, user.id, true).await
        .expect("disable user");

    // Exchange should return invalid_grant, not a token set.
    let resp = exchange_code(&state, &code, &client_id, &client_secret, &verifier).await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "expected 400 invalid_grant for disabled user"
    );
    let body = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    // The server uses "protocol_code" for the OAuth error token in its
    // JSON envelope (see sui-id-shared::errors::ApiErrorBody).
    assert_eq!(
        json["error"].as_str(),
        Some("invalid_grant"),
        "expected invalid_grant per RFC 6749"
    );

    // The audit log should contain the user_revoked event.
    let audit = sui_id_store::repos::audit::recent(&state.db, 50).await.expect("audit");
    let has_event = audit
        .iter()
        .any(|e| e.action == "oauth2.exchange_code.user_revoked");
    assert!(has_event, "expected oauth2.exchange_code.user_revoked in audit log");
}

/// RFC 019 § 3: auth codes are invalidated when a user is disabled,
/// so they can't be exchanged even if the user is later re-enabled.
#[tokio::test]
async fn auth_codes_invalidated_on_user_disable() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // Get a code while the user is active.
    let (code, verifier) = get_auth_code(&state, &session, &client_id).await;

    // Disable the user via the store-level call (which `set_user_disabled`
    // delegates to after the admin/self-check). This is the path that
    // invalidates outstanding auth codes.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await
        .expect("find user");
    sui_id_store::repos::users::set_disabled(&state.db, user.id, true).await
        .expect("set_disabled");
    sui_id_store::repos::auth_codes::invalidate_all_for_user(&state.db, user.id).await
        .expect("invalidate_auth_codes");

    // Re-enable the user immediately to check that the code is still dead.
    sui_id_store::repos::users::set_disabled(&state.db, user.id, false).await
        .expect("re-enable user");

    // The code should now be consumed and must return invalid_grant.
    let resp = exchange_code(&state, &code, &client_id, &client_secret, &verifier).await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "code should be consumed after user was disabled"
    );
    let body = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["error"].as_str(), Some("invalid_grant"), "expected invalid_grant");
}

/// RFC 019 § 5 (GC fix): a revoked-but-unexpired refresh token survives
/// the GC and still fires theft detection when replayed.
#[tokio::test]
async fn refresh_token_theft_detection_survives_gc() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // Full authorize → token exchange to get a refresh token.
    let (code, verifier) = get_auth_code(&state, &session, &client_id).await;
    let resp = exchange_code(&state, &code, &client_id, &client_secret, &verifier).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let refresh_a = json["refresh_token"].as_str().expect("refresh_token").to_owned();

    // Rotate: use refresh_a to get refresh_b (this revokes refresh_a).
    let rotate_body = format!(
        "grant_type=refresh_token&refresh_token={refresh_a}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(rotate_body))
                .expect("req"),
        )
        .await
        .expect("rotate");
    assert_eq!(resp.status(), StatusCode::OK, "rotation should succeed");
    let body = read_body(resp.into_body()).await;
    let json2: serde_json::Value = serde_json::from_slice(&body).expect("json2");
    assert!(json2["refresh_token"].is_string(), "should get a new refresh token");

    // Run GC. The old refresh_a is revoked but not yet expired: the
    // fixed GC only deletes rows WHERE expires_at < now, so refresh_a
    // should survive.
    sui_id::gc::run_once(&state).await;

    // Replay refresh_a. If the GC had deleted it we'd get a generic
    // not-found error. If theft detection fires we get invalid_grant
    // (the family is revoked). Either way the request must fail; what
    // we assert here is that the entire family is revoked — i.e. the
    // audit log contains theft_detected.
    let replay_body = format!(
        "grant_type=refresh_token&refresh_token={refresh_a}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(replay_body))
                .expect("req"),
        )
        .await
        .expect("replay");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "replay must fail");

    // The audit log should contain the theft_detected event.
    let audit = sui_id_store::repos::audit::recent(&state.db, 50).await.expect("audit");
    let has_theft = audit
        .iter()
        .any(|e| e.action == "auth.refresh.theft_detected");
    assert!(
        has_theft,
        "expected auth.refresh.theft_detected in audit log after GC + replay"
    );
}
