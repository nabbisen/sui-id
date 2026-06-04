//! Multilingual support v1 (v0.23.0): locale resolution chain.
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use sui_id::{build_router, AppState};

use tower::ServiceExt;
use super::common::*;

// ---------- v0.23.0: Multilingual support ----------

/// `<html lang>` reflects whichever locale the resolution chain
/// picks. With no user, no cookie, no Accept-Language, the
/// migration's seeded default ('ja') wins.
#[tokio::test]
async fn login_page_html_lang_defaults_to_ja() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("login GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = std::str::from_utf8(&bytes).expect("utf8");
    assert!(
        body.contains("<html lang=\"ja\">"),
        "expected lang=ja, body starts: {}",
        &body[..body.len().min(200)]
    );
}

/// Accept-Language header pushes `<html lang>` to en.
#[tokio::test]
async fn login_page_html_lang_follows_accept_language() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login")
                .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("login GET");
    let bytes = read_body(resp.into_body()).await;
    let body = std::str::from_utf8(&bytes).expect("utf8");
    assert!(
        body.contains("<html lang=\"en\">"),
        "expected lang=en, head starts: {}",
        &body[..body.len().min(200)]
    );
}

/// `sui_id_lang` cookie overrides Accept-Language.
#[tokio::test]
async fn lang_cookie_overrides_accept_language() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login")
                .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
                .header(header::COOKIE, "sui_id_lang=ja")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("login GET");
    let bytes = read_body(resp.into_body()).await;
    let body = std::str::from_utf8(&bytes).expect("utf8");
    assert!(
        body.contains("<html lang=\"ja\">"),
        "cookie should override Accept-Language; body: {}",
        &body[..body.len().min(200)]
    );
}

/// Posting to /admin/profile/lang persists the value, sets the
/// cookie, and is reflected on the next render.
#[tokio::test]
async fn profile_lang_post_persists_and_sets_cookie() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // GET /admin/profile to obtain a CSRF token.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/profile")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("profile GET");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    // POST /admin/profile/lang with `lang=en`.
    let body = format!("_csrf={csrf}&lang=en");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/profile/lang")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(
        resp.status().is_redirection(),
        "expected redirect, got {}",
        resp.status()
    );
    // Set-Cookie sui_id_lang=en should appear.
    let set_cookies: Vec<_> = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert!(
        set_cookies.iter().any(|c| c.starts_with("sui_id_lang=en")),
        "expected sui_id_lang=en cookie; saw: {:?}",
        set_cookies
    );

    // DB has been updated.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("user");
    assert_eq!(user.preferred_lang.as_deref(), Some("en"));
}

/// Setting `lang=` (empty) clears the preference and the cookie.
#[tokio::test]
async fn profile_lang_clear_resets_to_browser_default() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // Pre-set to "en" first.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("user");
    sui_id_store::repos::users::set_preferred_lang(
        &state.db,
        user.id,
        Some("en"),
        chrono::Utc::now(),
    ).await
    .expect("preset");

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/profile")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("profile GET");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&lang=");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/profile/lang")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(resp.status().is_redirection());

    // Cookie cleared (Max-Age=0).
    let set_cookies: Vec<_> = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert!(
        set_cookies
            .iter()
            .any(|c| c.starts_with("sui_id_lang=") && c.contains("Max-Age=0")),
        "expected sui_id_lang to be cleared, saw: {:?}",
        set_cookies
    );

    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("user");
    assert_eq!(user.preferred_lang, None);
}

/// Unknown language tag is rejected with 400.
#[tokio::test]
async fn profile_lang_post_rejects_unknown_tag() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/profile")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&lang=xyz");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/profile/lang")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(
        resp.status().is_client_error(),
        "expected 4xx, got {}",
        resp.status()
    );
}

/// Admin can change the server default language at
/// /admin/settings/basic/lang.
#[tokio::test]
async fn admin_settings_basic_default_lang_change() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/basic")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("settings basic");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&default_lang=en");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/basic/lang")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(resp.status().is_redirection());

    let row = sui_id_store::repos::server_settings::get(&state.db).await.expect("settings");
    assert_eq!(row.default_lang, "en");
}

/// Forgot-password mail is sent in the recipient's preferred
/// locale.
#[tokio::test]
async fn forgot_password_email_in_user_preferred_locale() {
    let (state, mailer) = test_app_with_mailer();
    complete_setup_and_login(&state).await;

    // Configure SMTP and set the user's email + preferred lang to en.
    enable_smtp_with_inmemory_mailer(&state).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("user");
    let user_id = user.id;
    state
        .db
        .with_conn(move |conn| {
            conn.execute(
                "UPDATE users SET email = ?1, email_normalized = lower(trim(?1)), preferred_lang = ?2 WHERE id = ?3",
                rusqlite::params!["alice@example.test", "en", user_id.to_string()],
            )
            .expect("update");
            Ok(())
        })
        .await
        .expect("set fields");

    // GET /forgot-password to obtain a CSRF cookie.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&email=alice%40example.test");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/forgot-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    // Forgot-password is user-enumeration neutral so it always
    // returns 200; we don't assert on the redirect/body shape.
    let _ = resp.status();

    let sent = mailer.drain().await;
    assert_eq!(
        sent.len(),
        1,
        "expected one email, got {}",
        sent.len()
    );
    let mail = &sent[0];
    // English subject contains the English string from STRINGS_EN.
    assert!(
        mail.subject.starts_with("Reset your password"),
        "expected English subject, got: {}",
        mail.subject
    );
    assert!(
        mail.text_body.contains("password-reset request"),
        "expected English body, got: {}",
        mail.text_body
    );
}

// Helper: configure the singleton smtp_config row to enabled with
// a dummy host. The InMemoryMailSender bypasses SMTP entirely, so
// the host value never needs to be reachable.
async fn enable_smtp_with_inmemory_mailer(state: &AppState) {
    use chrono::Utc;
    let now = Utc::now();
    sui_id_store::repos::smtp_config::upsert(
        &state.db,
        &sui_id_store::models::SmtpConfigRow {
            enabled: true,
            host: "smtp.test".into(),
            port: 587,
            tls_mode: sui_id_store::models::SmtpTlsMode::StartTls,
            username: None,
            password_enc: None,
            from_address: "sui-id@example.test".into(),
            from_name: Some("sui-id".into()),
            base_url: "https://sui-id.example.test".into(),
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .expect("smtp upsert");
}

