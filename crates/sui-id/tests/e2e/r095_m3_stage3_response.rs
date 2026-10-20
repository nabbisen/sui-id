//! RFC 095 M3 stage 3 — the registration response, exercised through the
//! real `POST /oauth2/register` endpoint. One file per matrix bullet in
//! *Response/error assertions*.

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
    headers: axum::http::HeaderMap,
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
    let headers = resp.headers().clone();
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    RegResult {
        status,
        headers,
        json,
    }
}

async fn register_no_token(state: &AppState, body: &str) -> RegResult {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("register");
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    RegResult {
        status,
        headers,
        json,
    }
}

// ── item 1: client_id_issued_at / client_secret_expires_at ───────────────

#[tokio::test]
async fn a_confidential_registration_gets_both_new_fields() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body =
        r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);

    let issued_at = r.json["client_id_issued_at"]
        .as_i64()
        .expect("client_id_issued_at must be present and an integer");
    let now = Utc::now().timestamp();
    assert!(
        (now - 60..=now + 1).contains(&issued_at),
        "client_id_issued_at {issued_at} is not close to now {now}"
    );

    // RFC 7591: 0 means "does not expire", not "already expired" -- this
    // deployment's secrets never expire, so it is always 0 when present.
    assert_eq!(r.json["client_secret_expires_at"], 0);
}

#[tokio::test]
async fn a_public_registration_has_issued_at_but_no_secret_expiry() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","token_endpoint_auth_method":"none","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);

    assert!(r.json["client_id_issued_at"].is_i64());
    assert!(
        r.json.get("client_secret_expires_at").is_none(),
        "client_secret_expires_at must be absent when no secret was issued: {}",
        r.json
    );
}

// ── item 2: the echo is canonically ordered, after defaults ──────────────

#[tokio::test]
async fn redirect_uris_are_echoed_in_sorted_order_regardless_of_request_order() {
    let state = test_app();
    let token = seed_token(&state).await;
    // Deliberately out of sorted order in the request.
    let body = r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://z.test/cb","https://a.test/cb","https://m.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(
        r.json["redirect_uris"],
        serde_json::json!([
            "https://a.test/cb",
            "https://m.test/cb",
            "https://z.test/cb"
        ])
    );
}

#[tokio::test]
async fn grant_types_are_echoed_sorted_and_defaulted() {
    let state = test_app();
    let token = seed_token(&state).await;
    // No grant_types sent -- after-defaults means this must show what was
    // actually stored (authorization_code alone), not what was asked for
    // (nothing).
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
async fn grant_types_with_refresh_are_echoed_in_sorted_order() {
    let state = test_app();
    let token = seed_token(&state).await;
    // Sent out of (what would be) sorted order.
    let body = r#"{"client_name":"Example","scope":"openid offline_access","grant_types":["refresh_token","authorization_code"],"redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(
        r.json["grant_types"],
        serde_json::json!(["authorization_code", "refresh_token"])
    );
}

#[tokio::test]
async fn scope_is_echoed_in_sorted_token_order() {
    let state = test_app();
    let token = seed_token(&state).await;
    // "profile", "openid", "email" is not sorted order.
    let body = r#"{"client_name":"Example","scope":"profile openid email","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(r.json["scope"], "email openid profile");
}

#[tokio::test]
async fn the_stored_redirect_uris_match_the_echoed_canonical_order() {
    // "storage and echo use the same rule, or the two disagree and a
    // re-registration diff becomes noise" -- checked against the row
    // itself, not just the response.
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://z.test/cb","https://a.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    let client_id: sui_id_shared::ids::ClientId = r.json["client_id"]
        .as_str()
        .expect("client_id")
        .parse()
        .expect("valid client id");
    let row = sui_id_store::repos::clients::get(&state.db, client_id)
        .await
        .expect("client row exists");
    assert_eq!(
        row.redirect_uris,
        vec!["https://a.test/cb", "https://z.test/cb"]
    );
    assert_eq!(
        r.json["redirect_uris"],
        serde_json::json!(["https://a.test/cb", "https://z.test/cb"])
    );
}

// ── item 4: every invalid-token state is one shape ───────────────────────

#[tokio::test]
async fn every_invalid_token_state_produces_the_identical_response() {
    let state = test_app();
    let body =
        r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test/cb"]}"#;

    // 1. No token at all.
    let no_token = register_no_token(&state, body).await;

    // 2. A malformed token (wrong length, not hex).
    let malformed = register_raw(&state, "NOT-HEX-AND-TOO-SHORT", body).await;

    // 3. A syntactically well-formed but unknown token.
    let unknown = register_raw(&state, &"a".repeat(64), body).await;

    // 4. A revoked token.
    let revoked_plain = random_registration_token_plaintext();
    let revoked_row = RegistrationTokenRow {
        id: RegistrationTokenId::new(),
        token_hash: sha256_hex(&revoked_plain),
        max_uses: 10,
        used_count: 0,
        expires_at: None,
        revoked_at: Some(Utc::now()),
        note: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    sui_id_store::repos::client_registration_token::create(&state.db, &revoked_row)
        .await
        .expect("seed revoked token");
    let revoked = register_raw(&state, &revoked_plain, body).await;

    // 5. An exhausted token: max_uses 1, spent by one real registration.
    let exhausted_plain = seed_token_with_max_uses(&state, 1).await;
    let first = register_raw(&state, &exhausted_plain, body).await;
    assert_eq!(first.status, StatusCode::CREATED, "{}", first.json);
    let exhausted = register_raw(&state, &exhausted_plain, body).await;

    let all = [
        ("no_token", &no_token),
        ("malformed", &malformed),
        ("unknown", &unknown),
        ("revoked", &revoked),
        ("exhausted", &exhausted),
    ];
    let (first_name, first_resp) = &all[0];
    for (name, resp) in &all[1..] {
        assert_eq!(
            resp.status, first_resp.status,
            "{name} status differs from {first_name}"
        );
        assert_eq!(
            resp.json, first_resp.json,
            "{name} body differs from {first_name}"
        );
        assert_eq!(
            resp.headers.get(header::WWW_AUTHENTICATE),
            first_resp.headers.get(header::WWW_AUTHENTICATE),
            "{name} WWW-Authenticate differs from {first_name}"
        );
        assert_eq!(
            resp.headers.get(header::CACHE_CONTROL),
            first_resp.headers.get(header::CACHE_CONTROL),
            "{name} Cache-Control differs from {first_name}"
        );
        assert_eq!(
            resp.headers.get(header::PRAGMA),
            first_resp.headers.get(header::PRAGMA),
            "{name} Pragma differs from {first_name}"
        );
    }
    assert_eq!(first_resp.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        first_resp.headers.get(header::WWW_AUTHENTICATE).unwrap(),
        "Bearer realm=\"sui-id\""
    );
}

async fn seed_token_with_max_uses(state: &AppState, max_uses: i64) -> String {
    let plaintext = random_registration_token_plaintext();
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

// ── item 5: Cache-Control and Pragma on success and error ────────────────

#[tokio::test]
async fn a_successful_registration_carries_both_cache_headers() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body =
        r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);
    assert_eq!(r.headers.get(header::CACHE_CONTROL).unwrap(), "no-store");
    assert_eq!(r.headers.get(header::PRAGMA).unwrap(), "no-cache");
}

#[tokio::test]
async fn an_error_response_also_carries_both_cache_headers() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","redirect_uris":[]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.headers.get(header::CACHE_CONTROL).unwrap(), "no-store");
    assert_eq!(r.headers.get(header::PRAGMA).unwrap(), "no-cache");
}

// ── item 6: public errors say nothing they should not ────────────────────

#[tokio::test]
async fn a_malformed_redirect_uri_is_not_echoed_in_the_error() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test\\evil.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(
        !r.json.to_string().contains("evil.test"),
        "the offending URI must not be echoed: {}",
        r.json
    );
    assert!(
        r.json["error_description"]
            .as_str()
            .unwrap()
            .contains("index")
    );
}

#[tokio::test]
async fn an_unknown_scope_token_is_not_echoed_in_the_error() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid a_very_distinctive_unknown_scope_name","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(
        !r.json
            .to_string()
            .contains("a_very_distinctive_unknown_scope_name"),
        "the offending scope token must not be echoed: {}",
        r.json
    );
}

#[tokio::test]
async fn an_unrecognised_auth_method_is_not_echoed_in_the_error() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","token_endpoint_auth_method":"a_very_distinctive_auth_method_value","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(
        !r.json
            .to_string()
            .contains("a_very_distinctive_auth_method_value"),
        "the submitted auth method must not be echoed: {}",
        r.json
    );
}

#[tokio::test]
async fn a_malformed_application_uri_is_not_echoed_in_the_error() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid","logo_uri":"http://a-very-distinctive-logo-host.test/x.png","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(
        !r.json.to_string().contains("a-very-distinctive-logo-host"),
        "the submitted logo_uri must not be echoed: {}",
        r.json
    );
    assert!(
        r.json["error_description"]
            .as_str()
            .unwrap()
            .contains("logo_uri")
    );
}
