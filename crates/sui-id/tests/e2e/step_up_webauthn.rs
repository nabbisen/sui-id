//! Step-up authentication via WebAuthn passkey (v0.21.1).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- v0.21.1: WebAuthn step-up ----------

#[tokio::test]
async fn step_up_form_shows_passkey_section_for_users_with_passkey() {
    use chrono::Utc;
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("alice");

    // Insert a fake passkey row directly. The contents need not be
    // a real webauthn-rs Passkey for the GET form to render — only
    // the existence of *any* row matters for `has_credentials`.
    let cred_row = sui_id_store::models::UserWebauthnCredentialRow {
        id: sui_id_shared::ids::WebauthnCredentialId::new(),
        user_id: user.id,
        credential_id: vec![1, 2, 3, 4],
        passkey_enc: vec![], // create().await seals our plaintext, this is overwritten
        nickname: "Test Key".into(),
        created_at: Utc::now(),
        last_used_at: None,
    };
    sui_id_store::repos::user_webauthn_credentials::create(&state.db, &cred_row, b"{}")
        .await
        .expect("create passkey");

    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up?return_to=/admin/users")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.contains(r#"id="step-up-passkey-form""#),
        "passkey form should render"
    );
    assert!(body.contains("/me/security/step-up/webauthn/start"));
    assert!(body.contains("/static/step-up-webauthn.js"));
}

#[tokio::test]
async fn step_up_form_omits_passkey_section_for_users_without_passkey() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(!body.contains(r#"id="step-up-passkey-form""#));
    assert!(!body.contains("/me/security/step-up/webauthn"));
}

#[tokio::test]
async fn step_up_webauthn_start_requires_csrf() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // No CSRF cookie set: posting to /webauthn/start must fail.
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up/webauthn/start")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::from("_csrf=&return_to=/me/security"))
                .expect("req"),
        )
        .await
        .expect("start POST");
    assert!(
        resp.status().is_client_error(),
        "missing CSRF must be rejected, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn step_up_webauthn_finish_without_pending_cookie_fails() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Get a CSRF cookie via the GET form so the finish call gets
    // past the CSRF guard and hits the pending-cookie check.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    // No pending-id cookie: finish must fail.
    let body = format!(
        "_csrf={csrf}\
         &credential={{}}\
         &return_to=/me/security"
    );
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up/webauthn/finish")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("finish POST");
    assert!(
        resp.status().is_client_error(),
        "missing pending cookie must reject, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn step_up_webauthn_start_for_user_without_passkey_returns_bad_request() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}&return_to=/me/security");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up/webauthn/start")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("start POST");
    // The user has no passkey, so start_webauthn returns BadRequest.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
