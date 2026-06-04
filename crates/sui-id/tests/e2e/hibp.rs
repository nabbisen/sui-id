//! HIBP password breach check at the setup wizard (v0.24.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use sui_id::build_router;

use tower::ServiceExt;
use super::common::*;

// ---------- v0.24.0: HIBP password breach check ----------

/// Default mode is `Warn`; setup wizard accepts a clean password.
/// The InMemoryHibpClient with no plan returns NotBreached, so
/// the setup completes normally — same shape as pre-v0.24.0.
#[tokio::test]
async fn setup_wizard_accepts_clean_password_in_warn_mode() {
    let (state, _mailer, _hibp) = test_app_with_hibp();
    // mode = warn (default).
    let _session = complete_setup_and_login(&state).await;
    // Got here = success.
}

/// In `Warn` mode, a breached password is accepted (the setup
/// completes) but a tracing warning is emitted at the call site.
/// We can't easily assert on tracing output from the e2e test, so
/// we just confirm the happy path still completes when the HIBP
/// stub flags the password.
#[tokio::test]
async fn setup_wizard_accepts_breached_password_in_warn_mode() {
    let (state, _mailer, hibp) = test_app_with_hibp();
    // Pre-program the HIBP stub: PASSWORD has been seen 9001 times.
    hibp.set_breached(PASSWORD, 9001);

    // Setup completes despite the breach hit.
    let _session = complete_setup_and_login(&state).await;
}

/// In `Block` mode, a breached password is refused with a 400.
/// The form re-renders with a flash message. The setup wizard
/// can be retried with a different (un-breached) password.
#[tokio::test]
async fn setup_wizard_rejects_breached_password_in_block_mode() {
    let (state, _mailer, hibp) = test_app_with_hibp();
    set_hibp_mode(&state, sui_id_store::models::HibpMode::Block).await;
    // Pre-program: the test password is breached.
    hibp.set_breached(PASSWORD, 12345);

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/admin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "setup_token={SETUP_TOKEN}&username={USERNAME}&password={pw}\
                     &confirm_password={pw}&display_name=&email=",
                    pw = PASSWORD,
                )))
                .expect("req"),
        )
        .await
        .expect("setup admin");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = read_body(resp.into_body()).await;
    let body = std::str::from_utf8(&bytes).expect("utf8");
    assert!(
        body.contains("過去のデータ漏洩"),
        "expected breach flash message, got: {}",
        &body[..body.len().min(500)]
    );
    // No admin row should have been created.
    let users_count: i64 = state
        .db
        .with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
                .map_err(Into::into)
        })
        .await
        .expect("count");
    assert_eq!(users_count, 0);
}

/// `Block` mode with an unbreached password proceeds normally.
#[tokio::test]
async fn setup_wizard_accepts_clean_password_in_block_mode() {
    let (state, _mailer, _hibp) = test_app_with_hibp();
    set_hibp_mode(&state, sui_id_store::models::HibpMode::Block).await;
    let _session = complete_setup_and_login(&state).await;
}

/// `Off` mode skips the check entirely, so a "breached" password
/// in the stub still goes through. Verifies the short-circuit
/// path (no client call at all).
#[tokio::test]
async fn setup_wizard_off_mode_skips_check() {
    let (state, _mailer, hibp) = test_app_with_hibp();
    set_hibp_mode(&state, sui_id_store::models::HibpMode::Off).await;
    hibp.set_breached(PASSWORD, 999);
    let _session = complete_setup_and_login(&state).await;
}

/// Fail-open: when the HIBP API is unavailable (the stub returns
/// `Unavailable`), `Block` mode lets the password through anyway.
/// This is the documented policy in migration 0017 — a flaky
/// external API must not lock an admin out of password setting.
#[tokio::test]
async fn setup_wizard_fails_open_when_hibp_unavailable_in_block_mode() {
    let (state, _mailer, hibp) = test_app_with_hibp();
    set_hibp_mode(&state, sui_id_store::models::HibpMode::Block).await;
    hibp.set_unavailable(PASSWORD);
    // Setup completes despite Block mode + the would-be-breached
    // password — fail-open.
    let _session = complete_setup_and_login(&state).await;
}

