//! Step-up authentication via TOTP code (v0.21.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- v0.21.0: step-up auth ----------

#[tokio::test]
async fn step_up_get_renders_form_with_return_to() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
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
    assert!(body.contains("再認証"));
    assert!(body.contains(r#"name="code""#));
    assert!(body.contains(r#"name="return_to""#));
    // The supplied return_to round-trips into the form.
    assert!(body.contains(r#"value="/admin/users""#));
}

#[tokio::test]
async fn step_up_get_sanitises_offsite_return_to() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up?return_to=https://attacker.example/")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // Off-site return_to gets collapsed to the safe default. The
    // attacker URL must NOT appear as a form value.
    assert!(!body.contains("attacker.example"));
    assert!(body.contains(r#"value="/me/security""#));
}

#[tokio::test]
async fn step_up_get_sanitises_protocol_relative_return_to() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up?return_to=//attacker.example/path")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(!body.contains("attacker"));
    assert!(body.contains(r#"value="/me/security""#));
}

#[tokio::test]
async fn step_up_post_with_correct_totp_marks_session_fresh_and_redirects() {
    use chrono::Utc;
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Enrol TOTP with a known secret directly via the store layer
    // so we can compute the expected code locally without going
    // through the enrolment flow (which would itself need the
    // running router).
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("alice exists");
    let secret = b"step-up-test-secret\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, user.id, secret)
        .await
        .expect("upsert pending totp");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, user.id, b"[]")
        .await
        .expect("confirm totp");

    // Acquire CSRF cookie via the GET.
    let resp = build_router(state.clone())
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
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie");

    let now = Utc::now().timestamp();
    let step = now / 30;
    let code = sui_id_core::totp::code_for_step(secret, step).await;

    let body = format!(
        "_csrf={csrf}\
         &code={code}\
         &return_to=/admin/users"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("step-up POST");
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
    assert_eq!(location, "/admin/users");

    // And the session row's last_step_up_at is now set.
    let session_id = sui_id_shared::ids::SessionId::from_uuid(
        session.parse::<uuid::Uuid>().expect("session id is uuid"),
    );
    let row = sui_id_store::repos::sessions::get(&state.db, session_id)
        .await
        .expect("session row");
    assert!(
        row.last_step_up_at.is_some(),
        "session must be marked fresh after a successful step-up"
    );
}

#[tokio::test]
async fn step_up_post_with_wrong_code_returns_400_with_flash() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Enrol TOTP so the verify path actually runs.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("alice exists");
    let secret = b"step-up-bad-code-secret\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, user.id, secret)
        .await
        .expect("upsert pending");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, user.id, b"[]")
        .await
        .expect("confirm");

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

    let body = format!(
        "_csrf={csrf}\
         &code=000000\
         &return_to=/me/security"
    );
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("コードが正しくありません"));
}

#[tokio::test]
async fn step_up_redirects_when_mfa_admin_lacks_fresh() {
    // Verify the gate fires from a sensitive admin handler when
    // the admin has MFA enrolled but no recent step-up.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Enrol MFA on the admin so the gate has something to gate
    // against.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("alice exists");
    let secret = b"gate-test-secret\x00\x00\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, user.id, secret)
        .await
        .expect("pending");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, user.id, b"[]")
        .await
        .expect("confirm");

    // The session predates the MFA enrolment, so its
    // last_step_up_at is still None and the gate should fire.

    // Get a CSRF cookie before posting to the gated endpoint.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/signing-keys")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("signing-keys GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/signing-keys/rotate")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("rotate POST");
    // Step-up gate must redirect to the challenge page.
    assert!(
        resp.status().is_redirection(),
        "expected step-up redirect, got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        location.starts_with("/me/security/step-up?"),
        "expected step-up URL, got {location}"
    );
    // The original target is preserved in `return_to`.
    assert!(
        location.contains("admin%2Fsigning-keys") || location.contains("/admin/signing-keys"),
        "return_to should preserve the original path, got {location}"
    );
}

#[tokio::test]
async fn admin_with_no_mfa_passes_step_up_gate_transparently() {
    // The default test admin has no MFA. The gate's
    // policy_for_session returns Allow for no-MFA users, so an
    // admin who never enrolled MFA can still rotate keys without
    // a re-prompt. This test pins that behaviour so future
    // refactors don't accidentally make MFA mandatory for admin
    // actions.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/signing-keys")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/signing-keys/rotate")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("rotate POST");
    // Should redirect to /admin/signing-keys (success), not to
    // /me/security/step-up (gate firing).
    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/signing-keys");
}
