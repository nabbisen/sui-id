//! i18n /me/security and password change page (v0.29.1).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- v0.29.1: /me/security i18n ----------

/// /me/security renders English when Accept-Language asks for it.
#[tokio::test]
async fn me_security_renders_in_en() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/overview")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("me_security");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"en\""), "expected lang=en");
    assert!(
        body.contains("Account security") || body.contains("Where you are signed in"),
        "expected English wording in /me/security"
    );
}

/// /me/security still renders Japanese on default Accept-Language.
/// Pins the resolution chain.
#[tokio::test]
async fn me_security_renders_in_ja() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/overview")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::ACCEPT_LANGUAGE, "ja")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("me_security");
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"ja\""));
    assert!(
        body.contains("アカウントセキュリティ") || body.contains("サインイン中"),
        "expected Japanese wording"
    );
}

/// /me/security/password renders English form labels.
#[tokio::test]
async fn me_password_change_renders_in_en() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("password change");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"en\""));
    assert!(
        body.contains("Current password") || body.contains("New password"),
        "expected English form labels in password-change"
    );
}

/// /me/security with `sui_id_lang=en` cookie returns English even
/// when Accept-Language asks for Japanese — pins the cookie's
/// priority for an authenticated screen.
#[tokio::test]
async fn me_security_cookie_overrides_accept_language() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/overview")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_lang=en"),
                )
                .header(header::ACCEPT_LANGUAGE, "ja")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("me_security");
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"en\""), "cookie should win");
}
