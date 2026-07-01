//! i18n phase 1 auth-flow screens (v0.29.0): setup wizard / forgot-password / step-up.
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- v0.29.0: Auth-flow i18n (setup wizard / forgot-password / step-up) ----------

/// English Accept-Language renders the setup welcome screen in
/// English with `<html lang="en">`.
#[tokio::test]
async fn setup_welcome_renders_in_en() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup")
                .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("welcome");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(
        body.contains("lang=\"en\""),
        "expected lang=en, got: {}",
        &body[..200.min(body.len())]
    );
    assert!(
        body.contains("Welcome to sui-id") || body.contains("Begin setup"),
        "expected English wording in setup welcome"
    );
}

/// Same screen with Japanese Accept-Language stays in Japanese.
/// Pins the locale-resolution chain.
#[tokio::test]
async fn setup_welcome_renders_in_ja() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup")
                .header(header::ACCEPT_LANGUAGE, "ja-JP,ja;q=0.9")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("welcome");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"ja\""));
    assert!(
        body.contains("ようこそ") || body.contains("セットアップ"),
        "expected Japanese wording"
    );
}

/// Setup admin form renders in English.
#[tokio::test]
async fn setup_admin_form_renders_in_en() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/admin")
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("admin form");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"en\""));
    assert!(
        body.contains("Username") || body.contains("Password"),
        "expected English form labels"
    );
}

/// Forgot-password form renders in English (when SMTP is enabled).
/// Without SMTP enabled the endpoint is 404, so this test enables
/// SMTP first.
#[tokio::test]
async fn forgot_password_renders_in_en() {
    let state = test_app();
    enable_smtp(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("forgot");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"en\""));
    assert!(
        body.contains("Reset") || body.contains("password") || body.contains("Email"),
        "expected English wording in forgot-password"
    );
}

/// Reset-password invalid page (token missing) renders in English.
#[tokio::test]
async fn reset_password_invalid_renders_in_en() {
    let state = test_app();
    enable_smtp(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/reset-password")
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("reset invalid");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("lang=\"en\""));
    assert!(
        body.contains("invalid") || body.contains("expired") || body.contains("Request"),
        "expected English wording in reset-invalid"
    );
}

/// Cookie-based locale (`sui_id_lang=en`) overrides Accept-Language.
#[tokio::test]
async fn locale_cookie_overrides_accept_language() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup")
                // Browser asks Japanese, but the cookie says English —
                // cookie wins (per the documented resolution chain).
                .header(header::ACCEPT_LANGUAGE, "ja-JP,ja;q=0.9")
                .header(header::COOKIE, "sui_id_lang=en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("welcome");
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(
        body.contains("lang=\"en\""),
        "cookie should win over Accept-Language"
    );
}

/// Helper: enable SMTP so forgot-password / reset-password endpoints
/// stop returning 404. Uses the test mailer that's already wired in.
async fn enable_smtp(state: &sui_id::AppState) {
    use chrono::Utc;
    let now = Utc::now();
    sui_id_store::repos::smtp_config::upsert(
        &state.db,
        &sui_id_store::models::SmtpConfigRow {
            enabled: true,
            host: "smtp.test".into(),
            port: 587,
            tls_mode: sui_id_store::models::SmtpTlsMode::StartTls,
            username: Some("user".into()),
            password_enc: Some(
                sui_id_store::crypto::seal(
                    state.db.key(),
                    b"smtp-pw",
                    sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
                )
                .expect("seal"),
            ),
            from_address: "noreply@example.test".into(),
            from_name: None,
            base_url: "https://idp.test".into(),
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .expect("upsert smtp");
}
