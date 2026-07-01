//! Security response headers and CORS (v0.17.0).
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

// ---------- security headers + CORS (v0.17.0) ----------

#[tokio::test]
async fn admin_responses_carry_security_headers() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("admin login GET");
    let h = resp.headers();
    // CSP must be present and forbid framing.
    let csp = h
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(csp.contains("frame-ancestors 'none'"), "csp={csp}");
    assert!(csp.contains("default-src 'self'"), "csp={csp}");
    // X-Frame-Options DENY for older browsers.
    assert_eq!(
        h.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("DENY")
    );
    assert_eq!(
        h.get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
    assert!(
        h.get("referrer-policy")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .contains("strict-origin")
    );
    assert!(h.contains_key("permissions-policy"));
}

#[tokio::test]
async fn discovery_endpoint_allows_cross_origin_fetch() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/openid-configuration")
        .header(header::ORIGIN, "https://spa.example")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("discovery");
    assert_eq!(resp.status(), StatusCode::OK);
    let acao = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok());
    assert_eq!(acao, Some("*"), "discovery must allow cross-origin");
}

#[tokio::test]
async fn jwks_endpoint_allows_cross_origin_fetch() {
    let state = test_app();
    // Admin must exist for active signing key to be present, but we
    // can hit the JWKS endpoint regardless — it just returns the
    // current keys (or an empty set on a totally fresh DB).
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/jwks.json")
        .header(header::ORIGIN, "https://spa.example")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("jwks");
    let acao = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok());
    assert_eq!(acao, Some("*"));
}

#[tokio::test]
async fn userinfo_response_carries_no_store_cache_control() {
    // Drive a token issuance so we have an access token to call
    // userinfo with. We piggyback the existing full-flow setup.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;
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
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let access = json["access_token"]
        .as_str()
        .expect("access_token")
        .to_owned();

    // Now hit userinfo and assert Cache-Control.
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/oauth2/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("userinfo");
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok());
    assert_eq!(cc, Some("no-store"), "userinfo must not be cacheable");
}
