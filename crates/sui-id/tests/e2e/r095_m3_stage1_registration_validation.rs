//! RFC 095 M3 stage 1 — the request envelope and the redirect profile,
//! exercised through the real `POST /oauth2/register` endpoint (not just
//! `dynamic_registration_validation`'s own unit tests, which never touch
//! the router, the registration-token store, or the response mapping).

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::RegistrationTokenId;
use sui_id_store::repos::client_registration_token::RegistrationTokenRow;

use super::common::*;
use tower::ServiceExt;

fn sha256_hex(input: &str) -> String {
    let hash = Sha256::digest(input.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

async fn seed_token(state: &AppState) -> String {
    let plaintext = random_registration_token_plaintext();
    let row = RegistrationTokenRow {
        id: RegistrationTokenId::new(),
        token_hash: sha256_hex(&plaintext),
        max_uses: 10,
        used_count: 0,
        expires_at: None,
        revoked_at: None,
        note: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    sui_id_store::repos::client_registration_token::create(&state.db, &row)
        .await
        .expect("seed registration token");
    plaintext
}

struct RegResult {
    status: StatusCode,
    json: serde_json::Value,
}

async fn register_raw(state: &AppState, token: &str, body: &str) -> RegResult {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/register")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("register");
    let status = resp.status();
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    RegResult { status, json }
}

// ── 1b: the envelope, through the real endpoint ─────────────────────────────

#[tokio::test]
async fn an_oversized_body_is_rejected() {
    let state = test_app();
    let token = seed_token(&state).await;
    let padding = "x".repeat(70 * 1024);
    let body = format!(r#"{{"client_name":"{padding}","redirect_uris":[]}}"#);
    let r = register_raw(&state, &token, &body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn malformed_json_is_rejected() {
    let state = test_app();
    let token = seed_token(&state).await;
    let r = register_raw(&state, &token, "{not json").await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn a_duplicate_member_is_rejected() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"a","client_name":"b","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn software_statement_gets_its_own_error_code() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body =
        r#"{"software_statement":"whatever.jwt.value","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "unapproved_software_statement");
}

#[tokio::test]
async fn a_known_unsupported_member_is_rejected() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"jwks_uri":"https://rp.test/jwks","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn an_unknown_extension_member_is_silently_ignored_and_never_echoed() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","redirect_uris":["https://rp.test/cb"],"x_custom_extension":"secret-looking-value"}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert!(
        !r.json.to_string().contains("secret-looking-value"),
        "an unknown extension member must never be echoed back: {}",
        r.json
    );
}

// ── bearer token format, through the real endpoint ──────────────────────────

#[tokio::test]
async fn a_malformed_bearer_token_takes_the_same_path_as_no_token() {
    let state = test_app();
    let body = r#"{"client_name":"Example","redirect_uris":["https://rp.test/cb"]}"#;

    let no_token = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("register");
    let malformed = register_raw(&state, "NOT-HEX-AND-TOO-SHORT", body).await;

    assert_eq!(no_token.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(malformed.status, StatusCode::UNAUTHORIZED);
    let no_token_bytes = read_body(no_token.into_body()).await;
    let no_token_json: serde_json::Value = serde_json::from_slice(&no_token_bytes).unwrap();
    assert_eq!(no_token_json, malformed.json);
}

// ── 1a: the derived closed profile, through the real endpoint ──────────────

#[tokio::test]
async fn confidential_https_registers_successfully() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert!(r.json["client_secret"].is_string());
}

#[tokio::test]
async fn public_https_registers_successfully_with_no_secret() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","token_endpoint_auth_method":"none","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert!(r.json["client_secret"].is_null());
}

#[tokio::test]
async fn public_native_loopback_registers_successfully() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example Native","token_endpoint_auth_method":"none","redirect_uris":["http://127.0.0.1:49152/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
}

#[tokio::test]
async fn confidential_with_loopback_is_rejected() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","redirect_uris":["http://127.0.0.1:49152/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_redirect_uri");
}

#[tokio::test]
async fn mixed_https_and_loopback_is_rejected() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","token_endpoint_auth_method":"none","redirect_uris":["https://rp.test/cb","http://127.0.0.1:49152/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_redirect_uri");
}

#[tokio::test]
async fn named_localhost_is_rejected_through_the_real_endpoint() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","token_endpoint_auth_method":"none","redirect_uris":["http://localhost:49152/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_redirect_uri");
}

#[tokio::test]
async fn an_http_post_logout_uri_is_rejected_even_for_a_loopback_profile() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example Native","token_endpoint_auth_method":"none","redirect_uris":["http://127.0.0.1:49152/cb"],"post_logout_redirect_uris":["http://127.0.0.1:49152/logout"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn a_public_native_profile_accepts_an_independent_https_logout_uri() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example Native","token_endpoint_auth_method":"none","redirect_uris":["http://127.0.0.1:49152/cb"],"post_logout_redirect_uris":["https://rp.test/logout"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
}

// Port-flexible matching for `PublicNativeLoopback` (the Redirect-corpus
// row this stage originally flagged as not built) is now built, by stage
// 1b (2026-10-05) -- see `r095_m3_stage1b_loopback_port_matching.rs`. The
// test that used to pin the (now closed) gap lived here; it is gone, not
// left behind with a stale assertion, since stage 1b's own file covers
// the real behaviour this one could only describe as absent.
