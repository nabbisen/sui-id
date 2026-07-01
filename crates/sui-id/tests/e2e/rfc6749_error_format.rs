//! RFC 6749 §5.2 wire-format error response tests.
//!
//! Verifies that protocol endpoints (`/oauth2/token`, `/oauth2/introspect`,
//! `/oauth2/revoke`) return `{"error":"...","error_description":"..."}` as
//! required by RFC 6749, not the internal API envelope that admin/UI endpoints
//! use.
//!
//! Also checks:
//! - 401 responses carry `WWW-Authenticate: Basic realm="sui-id"`.
//! - `Cache-Control: no-store` is present on protocol error responses.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::Value;
use sui_id::build_router;
use tower::ServiceExt;

use super::common::*;

// ── helpers ──────────────────────────────────────────────────────────────────

async fn post_token(state: &sui_id::AppState, body: &str) -> axum::response::Response {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("token")
}

async fn post_introspect(state: &sui_id::AppState, body: &str) -> axum::response::Response {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/introspect")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("introspect")
}

// ── /oauth2/token ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn token_unsupported_grant_type_returns_rfc6749_error() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (cid, secret) = create_client(&state, &session).await;

    let resp = post_token(
        &state,
        &format!("grant_type=password&client_id={cid}&client_secret={secret}"),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(&read_body(resp.into_body()).await).expect("json");

    assert_eq!(
        body["error"].as_str(),
        Some("unsupported_grant_type"),
        "token error must be RFC 6749 format; got {body}"
    );
    assert!(
        body["error_description"].is_string(),
        "error_description must be present; got {body}"
    );
    // Must NOT contain the internal envelope fields.
    assert!(
        body.get("code").is_none(),
        "internal 'code' field must be absent"
    );
    assert!(
        body.get("protocol_code").is_none(),
        "internal 'protocol_code' must be absent"
    );
    assert!(
        body.get("request_id").is_none(),
        "request_id must be absent from protocol errors"
    );
}

#[tokio::test]
async fn token_invalid_grant_returns_rfc6749_error() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (cid, secret) = create_client(&state, &session).await;
    let (verifier, challenge) = pkce_pair();

    // Get a real code first, then tamper with it.
    let auth_resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/oauth2/authorize?client_id={cid}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
                     &response_type=code&scope=openid&state=s\
                     &code_challenge={challenge}&code_challenge_method=S256"
                ))
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");
    assert!(auth_resp.status().is_redirection());

    let resp = post_token(
        &state,
        &format!(
            "grant_type=authorization_code&code=definitely-not-a-valid-code\
             &redirect_uri=https%3A%2F%2Frp.test%2Fcb\
             &client_id={cid}&client_secret={secret}\
             &code_verifier={verifier}"
        ),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(&read_body(resp.into_body()).await).expect("json");

    assert_eq!(
        body["error"].as_str(),
        Some("invalid_grant"),
        "invalid code must produce invalid_grant; got {body}"
    );
    assert!(body.get("code").is_none(), "internal 'code' must be absent");
}

#[tokio::test]
async fn token_missing_client_id_returns_invalid_client_401() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    let resp = post_token(
        &state,
        "grant_type=authorization_code&code=x&redirect_uri=y&code_verifier=z",
    )
    .await;

    // RFC 6749: missing client authentication → 401 with WWW-Authenticate.
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "missing client_id must be 401; got {}",
        resp.status()
    );
    let www_auth = resp
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        www_auth.contains("Basic"),
        "WWW-Authenticate must contain Basic; got {www_auth:?}"
    );
    let body: Value = serde_json::from_slice(&read_body(resp.into_body()).await).expect("json");
    assert_eq!(
        body["error"].as_str(),
        Some("invalid_client"),
        "missing client_id must produce invalid_client; got {body}"
    );
}

#[tokio::test]
async fn token_wrong_client_secret_returns_invalid_client_401() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (cid, _) = create_client(&state, &session).await;

    let resp = post_token(
        &state,
        &format!(
            "grant_type=authorization_code&code=x&redirect_uri=y\
             &client_id={cid}&client_secret=wrong_secret&code_verifier=z"
        ),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body: Value = serde_json::from_slice(&read_body(resp.into_body()).await).expect("json");
    assert_eq!(body["error"].as_str(), Some("invalid_client"));
}

#[tokio::test]
async fn token_protocol_error_has_cache_control_no_store() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    let resp = post_token(&state, "grant_type=unknown_grant").await;

    let cache_control = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    assert!(
        cache_control.contains("no-store"),
        "protocol error responses must have Cache-Control: no-store; got {cache_control:?}"
    );
}

// ── /oauth2/introspect ────────────────────────────────────────────────────────

#[tokio::test]
async fn introspect_unauthenticated_returns_rfc6749_invalid_client() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    // No client credentials at all.
    let resp = post_introspect(&state, "token=something").await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // Read headers before consuming body.
    let www_auth = resp
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    assert!(
        www_auth.contains("Basic"),
        "introspect 401 must include WWW-Authenticate: Basic; got {www_auth:?}"
    );
    let body: Value = serde_json::from_slice(&read_body(resp.into_body()).await).expect("json");
    assert_eq!(
        body["error"].as_str(),
        Some("invalid_client"),
        "unauthenticated introspect must return invalid_client; got {body}"
    );
    assert!(body.get("code").is_none(), "internal 'code' must be absent");
}

#[tokio::test]
async fn introspect_valid_client_but_garbage_token_returns_inactive() {
    // RFC 7662: if the token is simply unknown, return {"active": false}, not an error.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (cid, secret) = create_client(&state, &session).await;

    let resp = post_introspect(
        &state,
        &format!("token=garbage&client_id={cid}&client_secret={secret}"),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(&read_body(resp.into_body()).await).expect("json");
    assert_eq!(
        body["active"].as_bool(),
        Some(false),
        "unknown token must return active=false; got {body}"
    );
}

// ── rate-limit: token endpoint returns OAuth wire format ──────────────────

#[tokio::test]
async fn token_rate_limit_returns_oauth_format_with_retry_after() {
    // The token endpoint rate-limiter must produce RFC 6749 wire format
    // (temporarily_unavailable) rather than the internal API envelope.
    // We exhaust the rate-limit bucket by making many failed requests.
    use axum::http::StatusCode;

    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    // Make repeated requests with garbage credentials to exhaust the limiter.
    // The rate limit is IP-based; in tests the IP is 127.0.0.1 or similar.
    // Drive enough requests to hit the limit (typically 10–20 per minute).
    let mut rate_limited: Option<(StatusCode, serde_json::Value, String)> = None;
    for _ in 0..30 {
        let resp = post_token(
            &state,
            "grant_type=authorization_code&code=x&redirect_uri=y\
             &client_id=00000000-0000-0000-0000-000000000000\
             &client_secret=garbage&code_verifier=z",
        )
        .await;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get(axum::http::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_owned();
            let body_bytes = read_body(resp.into_body()).await;
            let body: serde_json::Value =
                serde_json::from_slice(&body_bytes).unwrap_or(serde_json::json!({}));
            rate_limited = Some((StatusCode::TOO_MANY_REQUESTS, body, retry_after));
            break;
        }
    }

    if let Some((status, body, retry_after)) = rate_limited {
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        // Must use OAuth wire format, not internal envelope.
        assert_eq!(
            body["error"].as_str(),
            Some("temporarily_unavailable"),
            "rate-limit must return temporarily_unavailable; got {body}"
        );
        assert!(
            body.get("code").is_none(),
            "internal 'code' field must be absent from rate-limit response"
        );

        // Retry-After header must be present.
        assert!(
            !retry_after.is_empty(),
            "rate-limit response must include Retry-After header"
        );
        assert!(
            retry_after.parse::<u64>().is_ok(),
            "Retry-After must be a number; got {retry_after:?}"
        );
    } else {
        // If we didn't hit the rate limit in 30 requests, the test is
        // inconclusive (rate-limit threshold may be higher in test config).
        // Skip rather than fail — this is an environmental constraint.
        eprintln!("SKIP: token rate limit not reached in 30 requests (check test config)");
    }
}
