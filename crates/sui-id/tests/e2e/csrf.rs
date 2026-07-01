//! CSRF token enforcement for admin POST endpoints; OIDC endpoints exempt.
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

// ---------- CSRF tests ----------

#[tokio::test]
async fn admin_get_sets_csrf_cookie() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/users")
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("admin GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf");
    assert!(csrf.is_some(), "admin GET must set sui_id_csrf");
    let value = csrf.unwrap();
    assert!(!value.is_empty());
    assert_eq!(value.len(), 43, "32 bytes b64url no-pad = 43 chars");
}

#[tokio::test]
async fn admin_post_without_csrf_cookie_is_forbidden() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // POST to clients/create with the session cookie but NO csrf cookie
    // and NO _csrf field. Should be 403.
    let router = build_router(state.clone());
    let body = "name=test-rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb&confidential=true";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/clients")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("post");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_post_with_mismatched_csrf_is_forbidden() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Real csrf cookie, but the form's _csrf field is something else.
    let real = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let body = format!(
        "name=test-rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb&confidential=true&_csrf=tampered-value-not-the-real-one"
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/clients")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={real}"),
        )
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("post");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_post_with_matching_csrf_succeeds() {
    // Sanity: this is what every other admin test relies on. If this
    // breaks, csrf has gone wrong systemically.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;
    assert!(!client_id.is_empty());
}

#[tokio::test]
async fn oidc_endpoints_are_not_subject_to_csrf() {
    // The /oauth2/* protocol surface must not require sui_id_csrf — it
    // is protocol traffic, not an admin form.
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

    // /token exchange with NO csrf cookie or field — must succeed.
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
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "OIDC token endpoint must not require CSRF"
    );
}
