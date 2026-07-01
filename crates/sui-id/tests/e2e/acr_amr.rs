//! acr / amr claims in ID tokens (v0.15.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};

use super::common::*;
use tower::ServiceExt;
use url::Url;

// ---------- acr / amr in ID tokens (v0.15.0) ----------

/// Decode the unverified payload of a JWT and return it as JSON.
/// Signature verification is exercised by the JWKS-driven tests; here
/// we only need to read claims back.
fn decode_jwt_payload(jwt: &str) -> serde_json::Value {
    use base64ct::{Base64UrlUnpadded, Encoding};
    let segments: Vec<&str> = jwt.split('.').collect();
    assert_eq!(segments.len(), 3, "JWT must have header.payload.signature");
    let payload = Base64UrlUnpadded::decode_vec(segments[1]).expect("base64url payload");
    serde_json::from_slice(&payload).expect("payload JSON")
}

/// Drive the same authorize→token flow as full_flow_*, but stop at the
/// /token response and decode the ID token claims so individual tests
/// can assert on `acr` / `amr`.
async fn drive_to_id_token_claims(state: &AppState, session_cookie: &str) -> serde_json::Value {
    let (client_id, client_secret) = create_client(state, session_cookie).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&nonce=n0&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session_cookie}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location header")
        .to_owned();
    let parsed = Url::parse(&location).expect("absolute redirect");
    let code = parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code in redirect");

    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let id_token = json["id_token"].as_str().expect("id_token");
    decode_jwt_payload(id_token)
}

#[tokio::test]
async fn id_token_carries_acr_1_and_amr_pwd_for_password_only_login() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let claims = drive_to_id_token_claims(&state, &session).await;
    assert_eq!(
        claims["acr"].as_str(),
        Some("1"),
        "password-only login must produce acr=\"1\"; got: {claims}"
    );
    let amr: Vec<&str> = claims["amr"]
        .as_array()
        .expect("amr is an array")
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();
    assert_eq!(amr, vec!["pwd"]);
}

/// Helper: enrol TOTP for the freshly-set-up admin and return the
/// shared secret bytes ready for `totp::code_for_step`.
async fn enroll_totp_for_test(state: &AppState, session_cookie: &str) -> Vec<u8> {
    let (secret_b32, _codes) = enroll_mfa_for(state, session_cookie).await;
    decode_b32(&secret_b32)
}

/// Helper: log in with password+TOTP and return the resulting session
/// cookie. Mirrors `mfa_enroll_then_login_with_totp_succeeds` but
/// extracted so MFA-related tests can re-use it.
async fn login_with_totp_for_test(state: &AppState, secret: &[u8]) -> String {
    use sui_id_core::totp;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "username=alice&password=alice-the-tester-password",
        ))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    let pending =
        extract_set_cookie(resp.headers(), "sui_id_pending_mfa").expect("pending_mfa cookie");

    let step = chrono::Utc::now().timestamp() / 30 + 1;
    let code = totp::code_for_step(secret, step).await;

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("code={code:06}&_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST");
    extract_set_cookie(resp.headers(), "sui_id_session").expect("session cookie after MFA success")
}

#[tokio::test]
async fn id_token_carries_acr_2_and_amr_with_mfa_after_totp_login() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let secret = enroll_totp_for_test(&state, &session).await;
    let new_session = login_with_totp_for_test(&state, &secret).await;
    let claims = drive_to_id_token_claims(&state, &new_session).await;

    assert_eq!(
        claims["acr"].as_str(),
        Some("2"),
        "MFA-with-TOTP must produce acr=\"2\"; got: {claims}"
    );
    let amr: Vec<&str> = claims["amr"]
        .as_array()
        .expect("amr is an array")
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();
    assert_eq!(amr, vec!["pwd", "otp", "mfa"]);
}

#[tokio::test]
async fn refresh_grant_preserves_acr_and_amr_from_original_session() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let secret = enroll_totp_for_test(&state, &session).await;
    let new_session = login_with_totp_for_test(&state, &secret).await;

    let (client_id, client_secret) = create_client(&state, &new_session).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&nonce=n0&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={new_session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&location)
        .expect("url")
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code");

    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let refresh_token = json["refresh_token"].as_str().expect("rt").to_owned();
    let initial = decode_jwt_payload(json["id_token"].as_str().expect("idt"));
    assert_eq!(initial["acr"].as_str(), Some("2"));

    // Exchange refresh — new ID token must echo original acr/amr.
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh_token}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("refresh token");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let refreshed = decode_jwt_payload(json["id_token"].as_str().expect("idt"));
    assert_eq!(
        refreshed["acr"].as_str(),
        Some("2"),
        "refresh must preserve acr; got: {refreshed}"
    );
    let amr: Vec<&str> = refreshed["amr"]
        .as_array()
        .expect("amr array")
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();
    assert_eq!(amr, vec!["pwd", "otp", "mfa"]);
}
