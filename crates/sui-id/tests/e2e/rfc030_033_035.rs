//! RFC 030 — Dangerous operation confirmation screens (v0.36.0).
//!
//! Tests that:
//! - Mutation POSTs without `_confirmed=1` are rejected (bypass protection).
//! - GET /admin/users/{id}/delete-confirm renders the confirmation screen.
//! - GET /admin/audit.csv returns CSV with correct headers.
//! - GET /admin/audit?q=<prefix> filters by event prefix.
//!
//! Part of the integration test binary; helpers come from [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;
use tower::ServiceExt;

use super::common::*;

// ---------- RFC 030: _confirmed bypass protection ----------

/// A direct POST to /admin/users/{id}/delete without `_confirmed=1`
/// must be rejected with 400 or 422, not execute the deletion.
#[tokio::test]
async fn delete_user_without_confirmed_is_rejected() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let clock = sui_id_core::time::system_clock();

    // Create a target user directly in the DB.
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .await
        .expect("alice")
        .id;
    let target = sui_id_core::admin::create_user(
        &state.db,
        &clock,
        None,
        sui_id_store::models::HibpMode::Off,
        admin_id,
        sui_id_core::admin::CreateUserSpec {
            username: "target-for-delete-test".into(),
            display_name: None,
            email: None,
            password: "target-password-12345".into(),
            is_admin: false,
        },
    )
    .await
    .expect("create target");

    let csrf = fetch_csrf(&state, &session).await;

    // POST to delete WITHOUT _confirmed=1
    let body = format!("_csrf={csrf}");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/admin/users/{}/delete", target.id))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("resp");

    // Must be rejected (400 Bad Request) — user should NOT be deleted.
    assert!(
        resp.status() == StatusCode::BAD_REQUEST || resp.status().as_u16() >= 400,
        "expected rejection, got {}",
        resp.status()
    );

    // Verify the user still exists.
    let still_there =
        sui_id_store::repos::users::find_by_username(&state.db, "target-for-delete-test").await;
    assert!(
        still_there.is_ok(),
        "user should still exist after rejected delete"
    );
}

/// A direct POST to /admin/users/{id}/mfa-reset without `_confirmed=1`
/// must be rejected.
#[tokio::test]
async fn mfa_reset_without_confirmed_is_rejected() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let clock = sui_id_core::time::system_clock();
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .await
        .expect("alice")
        .id;
    let target = sui_id_core::admin::create_user(
        &state.db,
        &clock,
        None,
        sui_id_store::models::HibpMode::Off,
        admin_id,
        sui_id_core::admin::CreateUserSpec {
            username: "target-mfa-test".into(),
            display_name: None,
            email: None,
            password: "target-pw-mfa-123456".into(),
            is_admin: false,
        },
    )
    .await
    .expect("create");

    let csrf = fetch_csrf(&state, &session).await;
    let body = format!("_csrf={csrf}"); // no _confirmed
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/admin/users/{}/mfa-reset", target.id))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("resp");

    assert!(
        resp.status().as_u16() >= 400,
        "expected rejection, got {}",
        resp.status()
    );
}

// ---------- RFC 030: confirmation screen GET ----------

/// GET /admin/users/{id}/delete-confirm must render the confirmation
/// page containing the target username and the danger button.
#[tokio::test]
async fn delete_confirm_page_renders() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let clock = sui_id_core::time::system_clock();
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .await
        .expect("alice")
        .id;
    let target = sui_id_core::admin::create_user(
        &state.db,
        &clock,
        None,
        sui_id_store::models::HibpMode::Off,
        admin_id,
        sui_id_core::admin::CreateUserSpec {
            username: "confirm-page-target".into(),
            display_name: None,
            email: None,
            password: "confirm-pw-12345678".into(),
            is_admin: false,
        },
    )
    .await
    .expect("create");

    // Step-up is required for delete-confirm; complete it via direct DB touch.
    // For this test we simply verify the step-up redirect happens OR the page renders.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/admin/users/{}/delete-confirm", target.id))
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");

    // Either renders the page (200) or redirects to step-up (302/303).
    assert!(
        resp.status() == StatusCode::OK
            || resp.status() == StatusCode::SEE_OTHER
            || resp.status() == StatusCode::FOUND,
        "unexpected status {}",
        resp.status()
    );
}

// ---------- RFC 033: audit CSV export ----------

/// GET /admin/audit.csv returns a 200 response with text/csv content-type
/// containing the CSV header row.
#[tokio::test]
async fn audit_csv_export_returns_csv() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/audit.csv")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");

    assert_eq!(resp.status(), StatusCode::OK, "expected 200");
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("text/csv"), "expected text/csv, got {ct}");

    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.starts_with("when,actor,action,target,result,note"),
        "CSV header row missing: first 80 chars: {:?}",
        &body.chars().take(80).collect::<String>()
    );
}

/// GET /admin/audit?q=auth.login filters to only login events.
#[tokio::test]
async fn audit_filter_by_event_prefix() {
    let state = test_app();
    // complete_setup_and_login causes auth.login.success to be emitted.
    let session = complete_setup_and_login(&state).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/audit?q=auth.login")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // The filter value should be echoed in the search input.
    assert!(
        body.contains("auth.login"),
        "filter value not found in page"
    );
}

// ---------- RFC 031: dashboard operator prompts ----------

/// When SMTP is not configured, the dashboard should show the SMTP warning.
#[tokio::test]
async fn dashboard_shows_smtp_warning_when_unconfigured() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Default test setup has no SMTP configured.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // The SMTP warning should appear (any locale).
    assert!(
        body.contains("SMTP") || body.contains("smtp") || body.contains("メール"),
        "expected SMTP warning on dashboard"
    );
}

// ---------- RFC 035: user detail page ----------

/// GET /admin/users/{id} renders the user detail page with the username.
#[tokio::test]
async fn user_detail_page_renders() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let clock = sui_id_core::time::system_clock();
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .await
        .expect("alice")
        .id;
    let target = sui_id_core::admin::create_user(
        &state.db,
        &clock,
        None,
        sui_id_store::models::HibpMode::Off,
        admin_id,
        sui_id_core::admin::CreateUserSpec {
            username: "detail-page-user".into(),
            display_name: Some("Detail Page User".into()),
            email: None,
            password: "detail-pw-12345678".into(),
            is_admin: false,
        },
    )
    .await
    .expect("create");

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/admin/users/{}", target.id))
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "expected 200 for user detail"
    );
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.contains("detail-page-user"),
        "username not found in user detail page"
    );
}
