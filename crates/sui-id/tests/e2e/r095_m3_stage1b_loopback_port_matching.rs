//! RFC 095 M3 stage 1b — the native-loopback port exception, exercised
//! through the real dynamic-registration endpoint and the real
//! `validate_client_and_redirect_uri`/CORS cache, not just
//! `authorize::redirect_uri_matches`'s own unit tests.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{ClientId, RegistrationTokenId};
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
        max_uses: 5,
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

/// Registers a client via the real `/oauth2/register` endpoint, enables
/// it (dynamically registered clients start disabled; `validate_client_
/// and_redirect_uri` would otherwise refuse it before matching is ever
/// reached), and returns its id.
async fn register_and_enable(state: &AppState, body: &str) -> ClientId {
    let token = seed_token(state).await;
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
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let client_id: ClientId = json["client_id"].as_str().unwrap().parse().unwrap();
    sui_id_store::repos::clients::set_disabled(&state.db, client_id, false)
        .await
        .expect("enable");
    client_id
}

#[tokio::test]
async fn a_public_native_loopback_client_matches_a_different_request_time_port() {
    let state = test_app();
    let client_id = register_and_enable(
        &state,
        r#"{"client_name":"Native","token_endpoint_auth_method":"none","redirect_uris":["http://127.0.0.1:49152/cb"]}"#,
    )
    .await;
    let outcome = sui_id_core::authorize::validate_client_and_redirect_uri(
        &state.db,
        client_id,
        "http://127.0.0.1:51234/cb",
    )
    .await;
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[tokio::test]
async fn the_same_client_still_refuses_a_differing_path() {
    let state = test_app();
    let client_id = register_and_enable(
        &state,
        r#"{"client_name":"Native","token_endpoint_auth_method":"none","redirect_uris":["http://127.0.0.1:49152/cb"]}"#,
    )
    .await;
    let outcome = sui_id_core::authorize::validate_client_and_redirect_uri(
        &state.db,
        client_id,
        "http://127.0.0.1:51234/other",
    )
    .await;
    assert!(outcome.is_err());
}

#[tokio::test]
async fn a_confidential_https_client_refuses_a_differing_port() {
    let state = test_app();
    let client_id = register_and_enable(
        &state,
        r#"{"client_name":"Confidential","redirect_uris":["https://rp.test:8443/cb"]}"#,
    )
    .await;
    let outcome = sui_id_core::authorize::validate_client_and_redirect_uri(
        &state.db,
        client_id,
        "https://rp.test:9443/cb",
    )
    .await;
    assert!(outcome.is_err());
}

#[tokio::test]
async fn a_public_https_client_refuses_a_differing_port() {
    let state = test_app();
    let client_id = register_and_enable(
        &state,
        r#"{"client_name":"PublicHttps","token_endpoint_auth_method":"none","redirect_uris":["https://rp.test:8443/cb"]}"#,
    )
    .await;
    let outcome = sui_id_core::authorize::validate_client_and_redirect_uri(
        &state.db,
        client_id,
        "https://rp.test:9443/cb",
    )
    .await;
    assert!(outcome.is_err());
}

/// CORS origin comparison is untouched: the loopback port exception
/// applies only to authorization-redirect matching, never to the
/// origin allowlist a `PublicNativeLoopback` client's own registered
/// loopback port is cached under (RFC 095's architecture document,
/// `architecture.md:197-199`).
#[tokio::test]
async fn cors_origin_comparison_still_requires_the_exact_port() {
    let state = test_app();
    register_and_enable(
        &state,
        r#"{"client_name":"Native","token_endpoint_auth_method":"none","redirect_uris":["http://127.0.0.1:49152/cb"]}"#,
    )
    .await;
    state
        .caches
        .redirect_origins
        .rebuild(&state.db)
        .await
        .expect("rebuild");
    assert!(
        state
            .caches
            .redirect_origins
            .contains("http://127.0.0.1:49152")
            .await
    );
    assert!(
        !state
            .caches
            .redirect_origins
            .contains("http://127.0.0.1:51234")
            .await,
        "CORS must not inherit the loopback port exception"
    );
}
