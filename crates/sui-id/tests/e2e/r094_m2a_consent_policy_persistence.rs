//! RFC 094 M2a, the persistence bug (2026-10-02) — the behaviour that
//! matters, not the column value: a dynamically registered client, once an
//! administrator enables it, shows the consent screen on first
//! authorization. Before the fix, `consent_policy` silently fell back to
//! the table's `'none'` default and the consent screen was never shown.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::RegistrationTokenId;
use sui_id_store::repos::client_registration_token::RegistrationTokenRow;

use super::common::*;
use tower::ServiceExt;

const REDIRECT: &str = "https://rp.test/cb";

fn sha256_hex(input: &str) -> String {
    let hash = Sha256::digest(input.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

async fn seed_token(state: &AppState) -> String {
    let plaintext = format!("regtoken-{}", uuid::Uuid::new_v4());
    let row = RegistrationTokenRow {
        id: RegistrationTokenId::new(),
        token_hash: sha256_hex(&plaintext),
        max_uses: 1,
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

/// Register a client dynamically and return its id. Starts disabled (P4).
async fn register_dynamic_client(state: &AppState) -> String {
    let token = seed_token(state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/register")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"redirect_uris":["{REDIRECT}"],"client_name":"Example RP"}}"#
                )))
                .expect("req"),
        )
        .await
        .expect("register");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = read_body(resp.into_body()).await;
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    body["client_id"].as_str().expect("client_id").to_owned()
}

#[tokio::test]
async fn a_dynamically_registered_client_shows_the_consent_screen_on_first_authorization() {
    let state = test_app();
    let admin_session = complete_setup_and_login(&state).await;
    let client_id = register_dynamic_client(&state).await;

    // An administrator enables it (P4's gate; not the behaviour under test).
    sui_id_store::repos::clients::set_disabled(
        &state.db,
        client_id.parse().expect("client id"),
        false,
    )
    .await
    .expect("enable");

    let (_, challenge) = pkce_pair();
    let uri = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri={}&response_type=code\
         &scope=openid&state=rp-state&code_challenge={challenge}&code_challenge_method=S256",
        urlencode(REDIRECT),
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(header::COOKIE, format!("sui_id_session={admin_session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "expected the consent screen (200), not a redirect carrying a code \
         straight to the client — a dynamically registered client's default \
         consent policy must not be silently skipped"
    );
}
