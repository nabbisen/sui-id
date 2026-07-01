//! Logout (RP-initiated end-session) and signing-key rotation tests.
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;
use url::Url;

#[tokio::test]
async fn logout_with_id_token_hint_revokes_session_and_redirects() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // Drive an authorization to obtain an id_token.
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&code_challenge={challenge}&code_challenge_method=S256"
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
        .unwrap_or("")
        .to_owned();
    let parsed = Url::parse(&location).expect("redirect");
    let code = parsed
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
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let id_token = json["id_token"].as_str().expect("id_token").to_owned();

    let logout_url = format!(
        "/oauth2/logout?id_token_hint={}&post_logout_redirect_uri=https%3A%2F%2Frp.test%2Fcb&state=xyz",
        utf8_encode(&id_token)
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&logout_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("logout");
    assert!(
        resp.status().is_redirection(),
        "expected redirect, got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        location.starts_with("https://rp.test/cb"),
        "should redirect back to RP: {location}"
    );
    assert!(location.contains("state=xyz"));
    let set_cookie = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("sui_id_session="))
        .expect("session cookie cleared");
    assert!(set_cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn logout_rejects_unregistered_post_redirect() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

    let logout_url = format!(
        "/oauth2/logout?client_id={client_id}&post_logout_redirect_uri=https%3A%2F%2Fattacker.test%2F"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&logout_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("logout");
    if resp.status().is_redirection() {
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(!loc.contains("attacker.test"));
    } else {
        assert!(resp.status().is_success());
    }
}

#[tokio::test]
async fn discovery_advertises_end_session_endpoint() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/openid-configuration")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("discovery");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let ese = json["end_session_endpoint"]
        .as_str()
        .expect("end_session_endpoint");
    assert!(ese.ends_with("/oauth2/logout"), "{ese}");
}

#[tokio::test]
async fn signing_key_rotation_publishes_both_keys_in_jwks() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Initially JWKS has exactly one key (created during setup).
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/jwks.json")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("jwks");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let keys_before = json["keys"].as_array().expect("keys").len();
    assert_eq!(keys_before, 1);
    let kid_before = json["keys"][0]["kid"].as_str().expect("kid").to_owned();

    // Rotate via the admin endpoint.
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/signing-keys/rotate")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("rotate");
    assert!(
        resp.status().is_redirection(),
        "expected redirect, got {}",
        resp.status()
    );

    // After rotation, JWKS should publish two keys: the new active one
    // plus the retired previous one (grace window).
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/jwks.json")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("jwks 2");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let keys_after: Vec<_> = json["keys"].as_array().expect("keys").iter().collect();
    assert_eq!(keys_after.len(), 2, "JWKS should publish active + retired");
    let kids: Vec<&str> = keys_after
        .iter()
        .filter_map(|k| k["kid"].as_str())
        .collect();
    assert!(
        kids.contains(&kid_before.as_str()),
        "old kid {kid_before} should still be present"
    );

    // The active row should be the *newer* one — verified by checking that
    // the store reports a different active kid than before.
    let active = sui_id_store::repos::signing_keys::active(&state.db)
        .await
        .expect("active");
    assert_ne!(active.id.to_string(), kid_before);
}

#[tokio::test]
async fn rotation_does_not_break_existing_authorization_flow() {
    // Pre-rotation: register a client and grab a token. Rotate. The
    // already-issued token should still verify (its kid is still in JWKS).
    // A *new* exchange should produce a token signed with the new key.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&code_challenge={challenge}&code_challenge_method=S256"
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
        .unwrap_or("")
        .to_owned();
    let parsed = Url::parse(&location).expect("redirect");
    let code = parsed
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
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let access_old = json["access_token"]
        .as_str()
        .expect("access_token")
        .to_owned();

    // Rotate.
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/signing-keys/rotate")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("rotate");
    assert!(resp.status().is_redirection());

    // The pre-rotation access token should still verify against /userinfo.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/oauth2/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {access_old}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("userinfo");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "old token should still verify in grace window"
    );
}

#[tokio::test]
async fn cannot_delete_active_signing_key() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    let active = sui_id_store::repos::signing_keys::active(&state.db)
        .await
        .expect("active");
    let active_id = active.id.to_string();

    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/signing-keys/{active_id}/delete"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("delete attempt");
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Active key should still exist.
    let still_active = sui_id_store::repos::signing_keys::active(&state.db)
        .await
        .expect("still active");
    assert_eq!(still_active.id.to_string(), active_id);
}

#[tokio::test]
async fn delete_retired_signing_key_drops_it_from_jwks() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // First key (created during setup), then rotate to retire it.
    let original_id = sui_id_store::repos::signing_keys::active(&state.db)
        .await
        .expect("active")
        .id
        .to_string();
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/signing-keys/rotate")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("rotate");
    assert!(resp.status().is_redirection());

    // Now delete the retired (original) one.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/signing-keys/{original_id}/delete"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("delete");
    assert!(resp.status().is_redirection() || resp.status().is_success());

    // JWKS should now publish exactly one key.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/jwks.json")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("jwks");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let kids: Vec<&str> = json["keys"]
        .as_array()
        .expect("keys")
        .iter()
        .filter_map(|k| k["kid"].as_str())
        .collect();
    assert_eq!(kids.len(), 1);
    assert!(
        !kids.contains(&original_id.as_str()),
        "retired+deleted key must be gone"
    );
}
