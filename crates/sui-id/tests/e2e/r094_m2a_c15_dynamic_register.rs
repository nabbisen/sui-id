//! RFC 094 M2a, C15 — RFC 7591 dynamic client registration is sealed in one
//! Class-A transaction, and the registration token is validated first and
//! consumed second: a request that cannot succeed must not spend it.

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

/// Seed a usable registration token and return its plaintext bearer value.
async fn seed_token(state: &AppState, max_uses: i64) -> String {
    let plaintext = format!("regtoken-{}", uuid::Uuid::new_v4());
    let row = RegistrationTokenRow {
        id: RegistrationTokenId::new(),
        token_hash: sha256_hex(&plaintext),
        max_uses,
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

fn register_request(token: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/oauth2/register")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .expect("req")
}

async fn used_count(state: &AppState, token_hash: &str) -> i64 {
    let hash = token_hash.to_owned();
    state
        .db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT used_count FROM client_registration_token WHERE token_hash = ?1",
                [&hash],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("read used_count")
}

/// `a_c15_dynamic_register`: the happy path, end to end. The token is
/// spent exactly once, the client row exists with `registered_via`
/// correctly stamped (not left at the `clients` table's default), and the
/// registration is audited.
#[tokio::test]
async fn a_c15_dynamic_register() {
    let state = test_app();
    let token = seed_token(&state, 1).await;
    let token_hash = sha256_hex(&token);

    let resp = build_router(state.clone())
        .oneshot(register_request(
            &token,
            r#"{"redirect_uris":["https://rp.test/cb"],"client_name":"Example RP"}"#,
        ))
        .await
        .expect("register");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = read_body(resp.into_body()).await;
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let client_id = body["client_id"].as_str().expect("client_id").to_owned();

    assert_eq!(
        used_count(&state, &token_hash).await,
        1,
        "the token was spent exactly once"
    );

    let row = sui_id_store::repos::clients::get(&state.db, client_id.parse().expect("id"))
        .await
        .expect("client row exists");
    assert_eq!(
        row.registered_via,
        sui_id_store::models::RegistrationSource::Dynamic,
        "registered_via is stamped, not left at the clients table's default"
    );

    let last = sui_id_store::repos::audit::recent(&state.db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("an audit row");
    assert_eq!(last.action, "client.dynamic_register");
    assert_eq!(last.target.as_deref(), Some(client_id.as_str()));
}

/// The defect this dispatch exists to close: a valid token presented
/// against a malformed body must not be spent. Proven the only way that
/// means anything — not "an error was returned", but that the *same*
/// token succeeds on a following well-formed request.
#[tokio::test]
async fn the_burn_is_gone_an_invalid_body_leaves_the_token_unspent() {
    let state = test_app();
    let token = seed_token(&state, 1).await;

    let resp = build_router(state.clone())
        .oneshot(register_request(&token, r#"{"redirect_uris":[]}"#))
        .await
        .expect("register with empty redirect_uris");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // The same token, now against a well-formed body, still works --
    // proving the first, failed attempt spent nothing.
    let resp = build_router(state.clone())
        .oneshot(register_request(
            &token,
            r#"{"redirect_uris":["https://rp.test/cb"],"client_name":"Example RP"}"#,
        ))
        .await
        .expect("register with a valid body");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "the token the first, malformed request did not spend registers this one"
    );
}

/// The same property, for a body that fails validation further in (an
/// invalid `redirect_uris` entry, caught after the emptiness check).
#[tokio::test]
async fn an_invalid_redirect_uri_also_leaves_the_token_unspent() {
    let state = test_app();
    let token = seed_token(&state, 1).await;

    let resp = build_router(state.clone())
        .oneshot(register_request(
            &token,
            r#"{"redirect_uris":["not-a-valid-uri"],"client_name":"Example RP"}"#,
        ))
        .await
        .expect("register with an invalid redirect_uri");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = build_router(state.clone())
        .oneshot(register_request(
            &token,
            r#"{"redirect_uris":["https://rp.test/cb"],"client_name":"Example RP"}"#,
        ))
        .await
        .expect("register with a valid body");
    assert_eq!(resp.status(), StatusCode::CREATED);
}

/// An exhausted token is refused (unchanged behaviour): `max_uses: 1`,
/// consumed by one successful registration, refuses a second.
#[tokio::test]
async fn an_exhausted_token_is_refused() {
    let state = test_app();
    let token = seed_token(&state, 1).await;

    let resp = build_router(state.clone())
        .oneshot(register_request(
            &token,
            r#"{"redirect_uris":["https://rp.test/cb"],"client_name":"Example RP"}"#,
        ))
        .await
        .expect("first registration");
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = build_router(state.clone())
        .oneshot(register_request(
            &token,
            r#"{"redirect_uris":["https://rp.test/cb2"],"client_name":"Second RP"}"#,
        ))
        .await
        .expect("second registration");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
