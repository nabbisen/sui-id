//! Refresh-token theft detection: family revocation on replay (v0.17.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use sui_id::build_router;

use url::Url;
use tower::ServiceExt;
use super::common::*;

// ---------- refresh token theft detection (v0.17.0) ----------

#[tokio::test]
async fn replaying_a_rotated_refresh_token_revokes_the_whole_family() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // Initial issuance via authorization-code grant.
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
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
        .unwrap()
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
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let original_refresh = json["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_owned();

    // First legitimate rotation.
    let body = format!(
        "grant_type=refresh_token&refresh_token={original_refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("first refresh");
    assert_eq!(resp.status(), StatusCode::OK, "first refresh must succeed");
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let new_refresh = json["refresh_token"].as_str().expect("rt").to_owned();

    // Now an attacker (who captured the original refresh token before
    // rotation) replays it. This must fail and revoke the entire family.
    let body = format!(
        "grant_type=refresh_token&refresh_token={original_refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("replay");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "replay of rotated refresh token must be refused"
    );

    // The new (legitimately rotated) refresh token must also be
    // revoked as part of the family-wide revoke. The legitimate
    // client will discover this on its next refresh and re-auth.
    let body = format!(
        "grant_type=refresh_token&refresh_token={new_refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("post-revoke refresh");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "legitimate token from the same family must also be revoked"
    );
}

#[tokio::test]
async fn theft_detection_writes_audit_event() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // Issue, rotate once, then replay the original.
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=s&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let code = Url::parse(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap(),
    )
    .unwrap()
    .query_pairs()
    .find(|(k, _)| k == "code")
    .map(|(_, v)| v.into_owned())
    .unwrap();

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
    let rt0 = json["refresh_token"].as_str().expect("rt").to_owned();

    // First rotation.
    let body = format!(
        "grant_type=refresh_token&refresh_token={rt0}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let _ = router.oneshot(req).await.expect("rotate");

    // Replay original.
    let body = format!(
        "grant_type=refresh_token&refresh_token={rt0}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let _ = router.oneshot(req).await.expect("replay");

    // Audit log should contain `auth.refresh.theft_detected`.
    let recent = sui_id_store::repos::audit::recent(&state.db, 50).await.expect("audit list");
    let count = recent
        .iter()
        .filter(|r| r.action == "auth.refresh.theft_detected")
        .count();
    assert!(
        count >= 1,
        "expected at least one auth.refresh.theft_detected audit row"
    );
}

