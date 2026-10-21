//! RFC 095 M3 stage 5 — browser presentation, exercised through the real
//! `/oauth2/register` and `/oauth2/authorize` endpoints. The renderer-level
//! assertions live in `sui_id_web::pages::oidc::tests`; this file closes the
//! gap between "the renderer is correct" and "the handler passes the right
//! thing" by registering a client with a distinctive `logo_uri` host and
//! reaching the real consent screen as that client.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::RegistrationTokenId;
use sui_id_store::repos::client_registration_token::RegistrationTokenRow;

use super::common::*;
use super::r120::{REDIRECT, consent_screen};
use tower::ServiceExt;

const LOGO_HOST: &str = "https://logo.e2e-marker.test/logo.png";

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

/// Registers a dynamic client with `LOGO_HOST` as its `logo_uri` and
/// `REDIRECT` as its sole redirect URI, and returns its `client_id`.
///
/// A dynamically registered client is created `is_disabled = true` (an
/// administrator must explicitly enable it, per `dynamic_register.rs`'s own
/// doc comment at `:15`) — enabled here to reach `/oauth2/authorize` at
/// all, standing in for that approval. This stage's gate does not depend on
/// `is_disabled`, so enabling it changes nothing the test is measuring.
async fn register_dynamic_client_with_logo(state: &AppState) -> String {
    let token = seed_token(state).await;
    let body = format!(
        r#"{{"client_name":"e2e stage 5 client","redirect_uris":["{REDIRECT}"],
             "scope":"openid","logo_uri":"{LOGO_HOST}"}}"#
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/register")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("register");
    assert_eq!(resp.status(), StatusCode::CREATED, "dynamic registration");
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("registration response");
    let client_id = json["client_id"]
        .as_str()
        .expect("client_id in registration response")
        .to_owned();
    sui_id_store::repos::clients::set_disabled(&state.db, client_id.parse().expect("id"), false)
        .await
        .expect("enable the dynamically registered client");
    client_id
}

#[tokio::test]
async fn a_dynamically_registered_clients_logo_host_appears_nowhere_in_the_consent_page() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = register_dynamic_client_with_logo(&state).await;

    let screen = consent_screen(&state, &client_id, &session, "rp-state").await;

    assert!(
        !screen.body.contains(LOGO_HOST),
        "RFC 095 `:269-270`: a dynamically registered client's logo_uri \
         must never be emitted — found it in the real consent response"
    );
}

/// Measured, not argued (stage 5 dispatch §5): whether `Content-Security-
/// Policy` reaches the consent response, and its exact value. It does, and
/// it is the global policy from `sui_id::http::security_headers` — the
/// header that makes an admin-registered client's logo unfetchable even
/// though this stage's gate does not apply to it.
#[tokio::test]
async fn the_consent_response_carries_the_image_blocking_csp() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = register_dynamic_client_with_logo(&state).await;

    let screen = consent_screen(&state, &client_id, &session, "rp-state").await;

    let csp = screen
        .csp
        .as_deref()
        .expect("the consent response must carry a Content-Security-Policy");
    assert!(
        csp.contains("img-src 'self' data:"),
        "unexpected CSP on the consent response: {csp}"
    );
}
