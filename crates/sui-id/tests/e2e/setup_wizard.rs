//! Three-step setup wizard (v0.20.4).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};

use super::common::*;
use tower::ServiceExt;

// ---------- v0.20.4: setup wizard 3-step ----------

#[tokio::test]
async fn setup_welcome_renders_when_uninitialized() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("welcome");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("sui-id へようこそ"));
    assert!(body.contains(r#"href="/setup/admin""#));
    // No form on the welcome page.
    assert!(!body.contains(r#"action="/setup/admin""#));
    // Step indicator shows step 1 active.
    assert!(body.contains("ようこそ"));
    assert!(body.contains("管理者作成"));
    assert!(body.contains("完了"));
}

#[tokio::test]
async fn setup_welcome_redirects_when_initialized() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("welcome after init");
    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/login");
}

#[tokio::test]
async fn setup_admin_form_renders_with_email_and_confirm() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/admin")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("admin GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains(r#"action="/setup/admin""#));
    assert!(body.contains(r#"name="setup_token""#));
    assert!(body.contains(r#"name="username""#));
    assert!(body.contains(r#"name="email""#));
    assert!(body.contains(r#"name="display_name""#));
    assert!(body.contains(r#"name="password""#));
    assert!(body.contains(r#"name="confirm_password""#));
}

#[tokio::test]
async fn setup_admin_post_creates_admin_with_email_and_redirects_to_done() {
    let state = test_app();
    let body = format!(
        "setup_token={SETUP_TOKEN}\
         &username={USERNAME}\
         &display_name=Alice\
         &email=alice%40example.test\
         &password={PASSWORD}\
         &confirm_password={PASSWORD}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/admin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("admin POST");
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
    // RFC 012: admin step now redirects to /setup/lang (language picker)
    // instead of /setup/done.
    assert_eq!(location, "/setup/lang");
    // Session cookie was set so subsequent wizard steps are authenticated.
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_some());

    // The email was persisted on the user row.
    let row = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("user exists");
    assert_eq!(row.email.as_deref(), Some("alice@example.test"));
}

#[tokio::test]
async fn setup_admin_post_rejects_mismatched_confirm() {
    let state = test_app();
    let body = format!(
        "setup_token={SETUP_TOKEN}\
         &username={USERNAME}\
         &email=\
         &password={PASSWORD}\
         &confirm_password=different-password-here"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/admin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("admin POST");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("一致しません"));
    // No user was created.
    let result = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await;
    assert!(matches!(result, Err(sui_id_store::StoreError::NotFound)));
}

#[tokio::test]
async fn setup_done_renders_after_initialization() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/done")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("done");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("セットアップ完了"));
    assert!(body.contains(r#"href="/admin""#));
}

#[tokio::test]
async fn setup_done_says_not_yet_when_uninitialized() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/done")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("done before init");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("完了していません"));
    assert!(body.contains(r#"href="/setup""#));
}

#[tokio::test]
async fn admin_users_create_form_accepts_email() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Get the users page to obtain a CSRF token.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/users")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("users GET");
    let csrf =
        extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie set on users GET");
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body);
    // The create form must render an email field.
    assert!(html.contains(r#"name="email""#));

    // Submit the form with an email.
    let new_user_pw = "new-user-password-12345";
    let form = format!(
        "username=bob\
         &display_name=Bob\
         &email=bob%40example.test\
         &password={new_user_pw}\
         &_csrf={csrf}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/users")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(form))
                .expect("req"),
        )
        .await
        .expect("users POST");
    assert!(resp.status().is_redirection() || resp.status() == StatusCode::OK);

    // Verify the user has the email persisted.
    let row = sui_id_store::repos::users::find_by_username(&state.db, "bob")
        .await
        .expect("bob exists");
    assert_eq!(row.email.as_deref(), Some("bob@example.test"));
}

// ---------- RFC 012: Extended wizard (lang + HIBP steps) ----------

/// Helper: drive the admin-creation step and return (session, redirect_location).
async fn post_setup_admin(state: &AppState) -> String {
    let body = format!(
        "setup_token={SETUP_TOKEN}\
         &username={USERNAME}\
         &display_name=Alice\
         &email=\
         &password={PASSWORD}\
         &confirm_password={PASSWORD}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/admin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST /setup/admin");
    assert!(
        resp.status().is_redirection(),
        "expected redirect after admin creation, got {}",
        resp.status()
    );
    // Return the session cookie that auto-login produced.
    extract_set_cookie(resp.headers(), "sui_id_session").expect("session cookie after setup/admin")
}

#[tokio::test]
async fn setup_wizard_lang_step_renders() {
    let state = test_app();
    let session = post_setup_admin(&state).await;

    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/lang")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET /setup/lang");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains(r#"name="lang""#), "lang radio button missing");
    assert!(body.contains(r#"value="ja""#));
    assert!(body.contains(r#"value="en""#));
}

#[tokio::test]
async fn setup_wizard_lang_step_saves_selection() {
    let state = test_app();
    let _session = post_setup_admin(&state).await;

    // POST lang = "en"
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/lang")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("lang=en"))
                .expect("req"),
        )
        .await
        .expect("POST /setup/lang");
    assert!(
        resp.status().is_redirection(),
        "expected redirect to /setup/hibp, got {}",
        resp.status()
    );
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(loc, "/setup/hibp");

    // Verify server_settings updated
    let settings = sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings");
    assert_eq!(
        settings.default_lang, "en",
        "default_lang should be 'en' after selecting English"
    );
}

#[tokio::test]
async fn setup_wizard_lang_step_defaults_to_ja() {
    let state = test_app();
    let _session = post_setup_admin(&state).await;

    // POST without an explicit lang value (empty string) — should default to "ja"
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/lang")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(""))
                .expect("req"),
        )
        .await
        .expect("POST /setup/lang empty");
    assert!(resp.status().is_redirection());

    let settings = sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings");
    assert_eq!(settings.default_lang, "ja", "should default to 'ja'");
}

#[tokio::test]
async fn setup_wizard_hibp_step_renders() {
    let state = test_app();
    let _session = post_setup_admin(&state).await;

    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/hibp")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET /setup/hibp");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.contains(r#"name="hibp_mode""#),
        "hibp_mode radio button missing"
    );
    assert!(body.contains(r#"value="off""#));
    assert!(body.contains(r#"value="warn""#));
    assert!(body.contains(r#"value="block""#));
}

#[tokio::test]
async fn setup_wizard_hibp_step_saves_block() {
    let state = test_app();
    let _session = post_setup_admin(&state).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/hibp")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("hibp_mode=block"))
                .expect("req"),
        )
        .await
        .expect("POST /setup/hibp");
    assert!(resp.status().is_redirection());
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(loc, "/setup/done");

    let settings = sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings");
    assert_eq!(settings.hibp_mode, sui_id_store::models::HibpMode::Block);
}

#[tokio::test]
async fn setup_wizard_hibp_step_defaults_to_warn() {
    let state = test_app();
    let _session = post_setup_admin(&state).await;

    // POST without a value — should default to "warn"
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/hibp")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(""))
                .expect("req"),
        )
        .await
        .expect("POST /setup/hibp empty");
    assert!(resp.status().is_redirection());

    let settings = sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings");
    assert_eq!(settings.hibp_mode, sui_id_store::models::HibpMode::Warn);
}

#[tokio::test]
async fn setup_wizard_full_extended_flow_completes() {
    // Drive the complete 5-step wizard: admin → lang → hibp → done.
    let state = test_app();
    let _session = post_setup_admin(&state).await;

    // Step 3: language
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/lang")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("lang=en"))
                .expect("req"),
        )
        .await
        .expect("lang step");
    assert!(resp.status().is_redirection());

    // Step 4: HIBP
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/hibp")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("hibp_mode=off"))
                .expect("req"),
        )
        .await
        .expect("hibp step");
    assert!(resp.status().is_redirection());

    // Step 5: done renders
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/done")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("done page");
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify final settings
    let settings = sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings");
    assert_eq!(settings.default_lang, "en");
    assert_eq!(settings.hibp_mode, sui_id_store::models::HibpMode::Off);
}
