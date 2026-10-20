//! RFC 095 M3 stage 2 — the field rules, exercised through the real
//! `POST /oauth2/register` endpoint. `dynamic_registration_validation`'s
//! own unit tests cover every boundary exhaustively; this file only
//! confirms the handler wiring itself: the right field maps to the right
//! public error code, the scope catalog is really read from the database,
//! and the response echoes what stage 2 computed.

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

// ── client_name ──────────────────────────────────────────────────────────

#[tokio::test]
async fn whitespace_around_client_name_is_trimmed() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body =
        r#"{"client_name":"  Example  ","scope":"openid","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(r.json["client_name"], "Example");
}

#[tokio::test]
async fn a_blank_client_name_is_rejected() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"   ","scope":"openid","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

// ── token_endpoint_auth_method ──────────────────────────────────────────

#[tokio::test]
async fn an_absent_auth_method_defaults_to_client_secret_basic_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body =
        r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(r.json["token_endpoint_auth_method"], "client_secret_basic");
    assert!(r.json["client_secret"].is_string());
}

#[tokio::test]
async fn the_matrix_s_shorthand_auth_method_is_rejected_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","token_endpoint_auth_method":"basic","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

// ── grant_types / response_types ────────────────────────────────────────

#[tokio::test]
async fn an_absent_grant_types_defaults_to_authorization_code_alone_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body =
        r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(
        r.json["grant_types"],
        serde_json::json!(["authorization_code"])
    );
}

#[tokio::test]
async fn refresh_token_without_authorization_code_is_rejected_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","grant_types":["refresh_token"],"redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn an_absent_response_types_defaults_to_code_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body =
        r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(r.json["response_types"], serde_json::json!(["code"]));
}

#[tokio::test]
async fn a_non_code_response_type_is_rejected_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","response_types":["token"],"redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

// ── scope, read from the real catalog ────────────────────────────────────

#[tokio::test]
async fn a_known_catalog_scope_including_openid_is_accepted() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid profile","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(r.json["scope"], "openid profile");
}

#[tokio::test]
async fn a_scope_missing_openid_is_rejected() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"profile email","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn a_scope_not_in_the_real_catalog_is_rejected() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid not_a_real_scope","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn an_absent_scope_is_rejected_not_treated_as_any_scope() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

// ── the refresh_token / offline_access coupling ──────────────────────────

#[tokio::test]
async fn refresh_token_without_offline_access_scope_is_rejected_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","grant_types":["authorization_code","refresh_token"],"redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn offline_access_scope_without_refresh_token_grant_is_rejected_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid offline_access","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn refresh_token_with_offline_access_scope_is_accepted_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid offline_access","grant_types":["authorization_code","refresh_token"],"redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
}

// ── application-identity URI fields ──────────────────────────────────────

#[tokio::test]
async fn a_canonical_https_logo_uri_is_accepted_and_echoed() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","logo_uri":"https://rp.test/logo.png","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(r.json["logo_uri"], "https://rp.test/logo.png");
}

#[tokio::test]
async fn an_http_logo_uri_is_rejected_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","logo_uri":"http://rp.test/logo.png","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

// ── post_logout_redirect_uris, the full policy ───────────────────────────

#[tokio::test]
async fn more_than_16_post_logout_uris_is_rejected_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let uris: Vec<String> = (0..17)
        .map(|i| format!("\"https://rp.test/logout{i}\""))
        .collect();
    let body = format!(
        r#"{{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test/cb"],"post_logout_redirect_uris":[{}]}}"#,
        uris.join(",")
    );
    let r = register_raw(&state, &token, &body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}

#[tokio::test]
async fn a_duplicate_post_logout_uri_is_rejected_end_to_end() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test/cb"],"post_logout_redirect_uris":["https://rp.test/logout","https://rp.test/logout"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json["error"], "invalid_client_metadata");
}
