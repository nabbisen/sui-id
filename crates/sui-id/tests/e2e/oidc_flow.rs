//! OIDC end-to-end happy and unhappy paths: full flow, PKCE, redirect_uri, discovery, healthz, rate limit, GC of expired auth codes.
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
async fn full_flow_setup_authorize_token_userinfo_refresh() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // /authorize - should redirect with ?code=...
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&nonce=n0&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    assert!(
        resp.status().is_redirection(),
        "expected redirect, got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location header")
        .to_owned();
    let parsed = Url::parse(&location).expect("absolute redirect");
    assert_eq!(parsed.host_str(), Some("rp.test"));
    let code = parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code in redirect");

    // /token authorization_code grant
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
    assert_eq!(resp.status(), StatusCode::OK, "/token should succeed");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let access = json["access_token"]
        .as_str()
        .expect("access_token")
        .to_owned();
    let refresh = json["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_owned();
    assert!(
        json["id_token"].is_string(),
        "openid scope should yield id_token"
    );
    assert_eq!(json["token_type"].as_str(), Some("Bearer"));

    // /userinfo with the bearer access token
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/oauth2/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("userinfo");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let info: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    assert_eq!(info["preferred_username"].as_str(), Some(USERNAME));

    // refresh_token grant
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("refresh");
    assert_eq!(resp.status(), StatusCode::OK, "refresh should succeed");
    let body_bytes = read_body(resp.into_body()).await;
    let json2: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let new_refresh = json2["refresh_token"].as_str().expect("rotated refresh");
    assert_ne!(new_refresh, refresh, "refresh tokens must rotate");

    // Old refresh must now be rejected.
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}&client_secret={client_secret}"
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
        "replayed refresh must fail"
    );
}

#[tokio::test]
async fn pkce_mismatch_is_rejected() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    let (_verifier, challenge) = pkce_pair();
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

    // Use a *wrong* verifier.
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier=not-the-right-verifier-at-all-x"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn redirect_uri_mismatch_is_rejected_at_authorize() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _) = create_client(&state, &session).await;

    let (_v, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Fattacker.test%2Fcb\
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
    // Should NOT be a redirect to the attacker.
    if resp.status().is_redirection() {
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            !loc.contains("attacker.test"),
            "redirect must not point at attacker.test, got {loc}"
        );
    } else {
        // 400-class response also acceptable.
        assert!(
            resp.status().is_client_error(),
            "expected error, got {}",
            resp.status()
        );
    }
}

#[tokio::test]
async fn discovery_advertises_only_supported_features() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/openid-configuration")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("discovery");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let algs = json["id_token_signing_alg_values_supported"]
        .as_array()
        .expect("array");
    assert!(algs.iter().any(|v| v.as_str() == Some("EdDSA")));
    assert!(!algs.iter().any(|v| v.as_str() == Some("RS256")));
}

#[tokio::test]
async fn healthz_returns_ok_and_does_not_leak_state() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    assert_eq!(json["status"].as_str(), Some("ok"));
    // Must NOT leak whether the system is initialized, who is logged in, etc.
    assert!(json.get("initialized").is_none());
    assert!(json.get("user_count").is_none());
}

#[tokio::test]
async fn login_rate_limit_returns_429_with_retry_after() {
    let state = test_app();
    let _session = complete_setup_and_login(&state).await;

    // The login limiter is configured to 10 requests per 60 seconds. Burn
    // through the budget with deliberately wrong credentials.
    for i in 0..10 {
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=alice&password=wrong"))
            .expect("req");
        let resp = router.oneshot(req).await.expect("login");
        assert_ne!(
            resp.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "request {i} unexpectedly throttled"
        );
    }
    // The eleventh attempt should be throttled with 429 + Retry-After.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=wrong"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login throttled");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry = resp
        .headers()
        .get(header::RETRY_AFTER)
        .expect("retry-after header");
    let secs: i64 = retry.to_str().expect("ascii").parse().expect("integer");
    assert!(secs > 0 && secs <= 60);
}

#[tokio::test]
async fn gc_purges_expired_auth_codes() {
    use chrono::{Duration, Utc};
    use sui_id_store::models::AuthorizationCodeRow;
    use sui_id_store::repos::auth_codes;

    let state = test_app();
    // Complete setup so that a real user and client exist in the DB.
    // Migration 0019 added FK constraints on auth_codes(user_id, client_id),
    // so we need real referents to insert a test row.
    let session = complete_setup_and_login(&state).await;
    let (client_id_str, _) = create_client(&state, &session).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("find user");
    let client_id: sui_id_shared::ids::ClientId = client_id_str.parse().expect("client_id");

    // Insert a code that already expired one minute ago.
    let row = AuthorizationCodeRow {
        code_hash: "deadbeef".repeat(8),
        client_id,
        user_id: user.id,
        redirect_uri: "https://rp.test/cb".into(),
        scope: "openid".into(),
        nonce: None,
        code_challenge: "x".into(),
        code_challenge_method: "S256".into(),
        expires_at: Utc::now() - Duration::minutes(1),
        consumed: false,
        created_at: Utc::now() - Duration::minutes(2),
        auth_methods: vec![],
    };
    auth_codes::insert(&state.db, &row).await.expect("insert");

    // Confirm the row exists before GC.
    let count_before: i64 = state
        .db
        .with_conn(|conn| {
            Ok(conn
                .query_row("SELECT COUNT(*) FROM auth_codes", [], |r| r.get(0))
                .expect("count"))
        })
        .await
        .expect("query");
    assert!(count_before >= 1);

    sui_id::gc::run_once(&state).await;

    let count_after: i64 = state
        .db
        .with_conn(|conn| {
            Ok(conn
                .query_row("SELECT COUNT(*) FROM auth_codes", [], |r| r.get(0))
                .expect("count"))
        })
        .await
        .expect("query");
    assert_eq!(count_after, 0, "expired auth code should have been GCed");
}
