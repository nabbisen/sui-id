//! i18n phase 2 self-service security screens (v0.29.1): MFA challenge / Profile / MFA setup.
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- v0.29.1: i18n auth-flow phase 2 ----------

/// Profile page renders in English when Accept-Language requests
/// it. Login first; the page is only reachable when authenticated.
#[tokio::test]
async fn profile_renders_in_en() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let router = build_router(state);
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/overview")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("profile");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"en\""), "expected lang=en");
    assert!(
        body.contains("Two-factor authentication") || body.contains("Passkeys"),
        "expected English profile wording"
    );
}

/// Same page with cookie-based locale override pinned to ja stays
/// Japanese even with English Accept-Language.
#[tokio::test]
async fn profile_renders_in_ja_with_cookie_override() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let router = build_router(state);
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/overview")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_lang=ja"),
                )
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("profile");
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"ja\""));
    assert!(
        body.contains("プロフィール") || body.contains("パスキー"),
        "expected Japanese profile wording"
    );
}

/// MFA setup page (after POST /me/security/mfa/enroll/start)
/// renders in English.
#[tokio::test]
async fn mfa_setup_renders_in_en() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // First GET /me/security/overview to obtain a CSRF token cookie.
    let router = build_router(state.clone());
    let prof_resp = router
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/overview")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("profile GET");
    let csrf = extract_set_cookie(prof_resp.headers(), "sui_id_csrf").expect("csrf cookie");
    let body_bytes = read_body(prof_resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    let csrf_token = extract_csrf_token(&body);

    // POST /me/security/mfa/enroll/start.
    let router = build_router(state);
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/mfa/enroll/start")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::ACCEPT_LANGUAGE, "en")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("_csrf={csrf_token}")))
                .expect("req"),
        )
        .await
        .expect("enroll start");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"en\""), "expected lang=en");
    assert!(
        body.contains("Set up two-factor") || body.contains("Verify"),
        "expected English MFA setup wording"
    );
}

/// MFA challenge screen (the one shown between password verification
/// and full session, when the user has TOTP enabled) renders in
/// English. We don't have a clean way to drive the full enrolment
/// then sign-out then re-login flow here, so we directly seed the
/// state and verify the GET returns English wording with no pending
/// MFA cookie (the page should still render the form, just without
/// the passkey block).
#[tokio::test]
async fn mfa_challenge_renders_in_en_without_pending() {
    let state = test_app();
    // Setup wizard so the DB has bootstrap data, but stay logged out.
    complete_setup_and_login(&state).await;
    let router = build_router(state);
    let resp = router
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login/mfa")
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("mfa challenge GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"en\""), "expected lang=en");
    assert!(
        body.contains("Verification code") || body.contains("Verify"),
        "expected English MFA challenge wording"
    );
}

/// Helper: extract `_csrf` token from rendered HTML.
fn extract_csrf_token(html: &str) -> String {
    let needle = "name=\"_csrf\" value=\"";
    let i = html.find(needle).expect("find _csrf input");
    let start = i + needle.len();
    let rest = &html[start..];
    let end = rest.find('"').expect("end of _csrf value");
    rest[..end].to_owned()
}
