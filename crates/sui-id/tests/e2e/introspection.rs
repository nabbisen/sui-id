//! RFC 7662 introspection + RFC 7009 revocation (v0.11.0).
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

// ---------- RFC 7662 introspection + RFC 7009 revocation (v0.11.0) ----------

/// Helper: full setup → authorize → token, returning
/// (client_id, client_secret, access_token, refresh_token).
async fn obtain_tokens(state: &AppState) -> (String, String, String, String) {
    let session = complete_setup_and_login(state).await;
    let (client_id, client_secret) = create_client(state, &session).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=x&nonce=n&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&loc)
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
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let access = json["access_token"].as_str().unwrap().to_owned();
    let refresh = json["refresh_token"].as_str().unwrap().to_owned();
    (client_id, client_secret, access, refresh)
}

#[tokio::test]
async fn introspect_returns_active_for_valid_access_token() {
    let state = test_app();
    let (client_id, client_secret, access, _refresh) = obtain_tokens(&state).await;
    let body = format!("token={access}&client_id={client_id}&client_secret={client_secret}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let v: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    assert_eq!(v["active"].as_bool(), Some(true));
    assert_eq!(v["token_type"].as_str(), Some("Bearer"));
    assert_eq!(v["client_id"].as_str(), Some(client_id.as_str()));
    assert_eq!(v["username"].as_str(), Some("alice"));
    assert!(v["sub"].is_string());
    assert!(v["exp"].is_i64());
}

#[tokio::test]
async fn introspect_returns_active_for_valid_refresh_token() {
    let state = test_app();
    let (client_id, client_secret, _access, refresh) = obtain_tokens(&state).await;
    let body = format!(
        "token={refresh}&token_type_hint=refresh_token\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert_eq!(v["active"].as_bool(), Some(true));
    assert_eq!(v["client_id"].as_str(), Some(client_id.as_str()));
}

#[tokio::test]
async fn introspect_returns_inactive_for_garbage_token() {
    let state = test_app();
    let (client_id, client_secret, _access, _refresh) = obtain_tokens(&state).await;
    let body =
        format!("token=this.is.not.a.token&client_id={client_id}&client_secret={client_secret}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert_eq!(v["active"].as_bool(), Some(false));
    // RFC 7662 §2.2: when active=false, no other fields should be sent.
    assert!(v.get("scope").is_none(), "inactive must not leak scope");
    assert!(v.get("sub").is_none(), "inactive must not leak sub");
}

#[tokio::test]
async fn introspect_rejects_unauthenticated_request() {
    let state = test_app();
    let (_cid, _cs, access, _refresh) = obtain_tokens(&state).await;
    let body = format!("token={access}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert!(
        resp.status().is_client_error(),
        "expected 4xx, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn revoke_then_introspect_shows_inactive_for_access_token() {
    let state = test_app();
    let (client_id, client_secret, access, _refresh) = obtain_tokens(&state).await;

    // Revoke.
    let body = format!(
        "token={access}&token_type_hint=access_token\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("revoke");
    assert_eq!(resp.status(), StatusCode::OK);

    // Now introspect should report inactive.
    let body = format!("token={access}&client_id={client_id}&client_secret={client_secret}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect2");
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert_eq!(v["active"].as_bool(), Some(false));

    // userinfo with the same revoked token must now reject.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/oauth2/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("userinfo");
    assert!(
        resp.status().is_client_error(),
        "userinfo must reject revoked token; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn revoke_refresh_token_invalidates_subsequent_refresh_grant() {
    let state = test_app();
    let (client_id, client_secret, _access, refresh) = obtain_tokens(&state).await;

    // Revoke the refresh.
    let body = format!(
        "token={refresh}&token_type_hint=refresh_token\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("revoke");
    assert_eq!(resp.status(), StatusCode::OK);

    // Subsequent refresh-grant attempt must fail.
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("refresh");
    assert!(
        resp.status().is_client_error(),
        "refresh of revoked token must fail; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn revoke_is_idempotent() {
    // RFC 7009 §2.2: revoking an already-revoked or invalid token
    // must still return 200.
    let state = test_app();
    let (client_id, client_secret, access, _refresh) = obtain_tokens(&state).await;

    for _ in 0..3 {
        let body = format!("token={access}&client_id={client_id}&client_secret={client_secret}");
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/oauth2/revoke")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .expect("req");
        let resp = router.oneshot(req).await.expect("revoke");
        assert_eq!(resp.status(), StatusCode::OK, "revoke must be idempotent");
    }

    // And a totally bogus token also returns 200.
    let body = format!("token=garbage&client_id={client_id}&client_secret={client_secret}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("revoke garbage");
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn introspect_other_clients_token_returns_inactive() {
    // Two clients exist; client B tries to introspect a token issued
    // to client A. RFC 7662 §2.2 requires inactive (no leakage).
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_a, secret_a) = create_client(&state, &session).await;
    let (client_b, secret_b) = create_client(&state, &session).await;

    // Get a token for client A.
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_a}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
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
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_owned();
    let code = Url::parse(&loc)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .unwrap();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_a}&client_secret={secret_a}&code_verifier={verifier}"
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
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    let access = v["access_token"].as_str().unwrap().to_owned();

    // Client B introspects A's token. Must come back inactive.
    let body = format!("token={access}&client_id={client_b}&client_secret={secret_b}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert_eq!(
        v["active"].as_bool(),
        Some(false),
        "client B must not see client A's token"
    );
}

#[tokio::test]
async fn discovery_advertises_introspect_and_revoke_endpoints() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/openid-configuration")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("discovery");
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert!(
        v["introspection_endpoint"]
            .as_str()
            .unwrap()
            .ends_with("/oauth2/introspect")
    );
    assert!(
        v["revocation_endpoint"]
            .as_str()
            .unwrap()
            .ends_with("/oauth2/revoke")
    );
    let methods = v["introspection_endpoint_auth_methods_supported"]
        .as_array()
        .unwrap();
    let methods_set: Vec<_> = methods.iter().filter_map(|x| x.as_str()).collect();
    assert!(methods_set.contains(&"client_secret_basic"));
    // Public clients (auth method "none") must NOT be listed.
    assert!(!methods_set.contains(&"none"));
}
