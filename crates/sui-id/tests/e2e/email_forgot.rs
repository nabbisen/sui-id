//! Forgot-password and reset-password e-mail flows (v0.22.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use sui_id::{build_router, AppState};

use tower::ServiceExt;
use super::common::*;

// ---------- v0.22.0: email features ----------

/// Insert a minimal SMTP configuration directly into the database
/// so the forgot-password endpoints stop returning 404 in tests.
/// We don't actually try to talk to a real SMTP relay — the
/// `InMemoryMailSender` injected via `test_app_with_mailer`
/// captures all sends.
async fn enable_smtp_in_db(state: &AppState) {
    use chrono::Utc;
    use sui_id_store::models::{SmtpConfigRow, SmtpTlsMode};
    let now = Utc::now();
    let row = SmtpConfigRow {
        enabled: true,
        host: "smtp.test.invalid".into(),
        port: 587,
        tls_mode: SmtpTlsMode::StartTls,
        username: Some("test".into()),
        password_enc: None,
        from_address: "noreply@test.invalid".into(),
        from_name: Some("sui-id Test".into()),
        base_url: "https://idp.test.invalid".into(),
        created_at: now,
        updated_at: now,
    };
    sui_id_store::repos::smtp_config::upsert(&state.db, &row).await
        .expect("upsert smtp_config");
}

#[tokio::test]
async fn forgot_password_get_404_when_smtp_disabled() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("forgot GET");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn forgot_password_get_renders_form_when_smtp_enabled() {
    let state = test_app();
    enable_smtp_in_db(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("forgot GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains(r#"action="/forgot-password""#));
    assert!(body.contains(r#"name="email""#));
}

#[tokio::test]
async fn forgot_password_post_neutral_response_for_unknown_email() {
    let (state, mailer) = test_app_with_mailer();
    enable_smtp_in_db(&state).await;

    // Get a CSRF cookie via the GET first.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}&email=ghost%40nowhere.invalid");
    let resp = build_router(state)
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
        .expect("POST");
    assert_eq!(resp.status(), StatusCode::OK);

    // No mail was sent — the email did not match a user.
    assert_eq!(mailer.count().await, 0);
}

#[tokio::test]
async fn forgot_password_post_sends_mail_for_known_email() {
    let (state, mailer) = test_app_with_mailer();
    let _ = complete_setup_and_login(&state).await;
    enable_smtp_in_db(&state).await;

    // The default test admin doesn't have an email set; assign one
    // directly so the forgot-password lookup matches.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await
        .expect("alice");
    let mut updated = user.clone();
    updated.email = Some("alice@test.invalid".into());
    updated.updated_at = chrono::Utc::now();
    // No bulk update helper; round-trip a delete/create pair would
    // complicate things, so we use a raw SQL UPDATE via the DB
    // handle.
    sui_id_store::repos::users::update_email(
        &state.db,
        user.id,
        updated.email.as_deref(),
        chrono::Utc::now(),
    ).await
    .expect("set email");

    // CSRF cookie via GET.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}&email=alice%40test.invalid");
    let resp = build_router(state)
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
        .expect("POST");
    assert_eq!(resp.status(), StatusCode::OK);

    // One mail captured. Subject and body shape pinned so future
    // reword changes are intentional.
    assert_eq!(mailer.count().await, 1);
    let last = mailer.last().await.expect("at least one mail");
    assert_eq!(last.to, "alice@test.invalid");
    assert!(last.subject.contains("パスワードのリセット"));
    assert!(last.text_body.contains("/reset-password?token="));
    assert!(last.html_body.is_some());
}

#[tokio::test]
async fn reset_password_get_invalid_for_unknown_token() {
    let state = test_app();
    enable_smtp_in_db(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/reset-password?token=this-does-not-exist")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // The "this link is invalid or expired" page renders, not the
    // new-password form.
    assert!(body.contains("無効"));
    assert!(!body.contains(r#"name="password""#));
}

#[tokio::test]
async fn reset_password_full_flow_changes_password_and_sends_notification() {
    let (state, mailer) = test_app_with_mailer();
    let _ = complete_setup_and_login(&state).await;
    enable_smtp_in_db(&state).await;

    // Set the admin's email so they can reset.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await
        .expect("alice");
    sui_id_store::repos::users::update_email(
        &state.db,
        user.id,
        Some("alice@test.invalid"),
        chrono::Utc::now(),
    ).await
    .expect("set email");

    // 1) POST /forgot-password to mint a token + capture mail
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET forgot");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let body = format!("_csrf={csrf}&email=alice%40test.invalid");
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
        .expect("POST forgot");
    assert_eq!(resp.status(), StatusCode::OK);

    // Extract the token from the captured mail.
    let mail = mailer.last().await.expect("reset mail");
    let prefix = "/reset-password?token=";
    let start = mail
        .text_body
        .find(prefix)
        .expect("link in mail")
        + prefix.len();
    let end = mail.text_body[start..]
        .find(|c: char| c == '\n' || c.is_whitespace())
        .map(|i| start + i)
        .unwrap_or(mail.text_body.len());
    let token = mail.text_body[start..end].to_owned();
    assert!(!token.is_empty());

    // 2) GET /reset-password?token=... renders the form
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&format!("/reset-password?token={token}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET reset");
    assert_eq!(resp.status(), StatusCode::OK);
    let csrf2 = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf2");
    let body_bytes = read_body(resp.into_body()).await;
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(body_str.contains(r#"name="password""#));

    // 3) POST /reset-password with new password
    let new_pw = "brand-new-secure-pw-12345";
    let body = format!(
        "_csrf={csrf2}&token={token}&password={new_pw}&confirm_password={new_pw}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/reset-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf2}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST reset");
    assert!(resp.status().is_redirection(), "expected redirect, got {}", resp.status());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(location.starts_with("/admin/login"));

    // 4) The captured mailer now has 2 mails: the reset link + a
    //    post-reset password-changed notification.
    assert_eq!(mailer.count().await, 2);
    let drained = mailer.drain().await;
    assert!(drained
        .iter()
        .any(|m| m.subject.contains("パスワードが変更されました")));

    // 5) Replay of the same token returns 400 + invalid page.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/reset-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET2");
    let csrf3 = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf3");
    let body = format!(
        "_csrf={csrf3}&token={token}&password=different-second-password-99&confirm_password=different-second-password-99"
    );
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/reset-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf3}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST replay");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn settings_email_get_renders_for_admin() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/email")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("settings GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("メール"));
    assert!(body.contains(r#"name="host""#));
    assert!(body.contains(r#"name="port""#));
    assert!(body.contains(r#"name="from_address""#));
}

#[tokio::test]
async fn settings_email_get_requires_admin() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/email")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("settings GET");
    // Anonymous request must not see the page.
    assert_ne!(resp.status(), StatusCode::OK);
}


// ---------- RFC 010: forgot-password reset revokes sessions and refresh tokens ----------

/// Helper: extract a password-reset token from a mail captured by the in-memory
/// mailer. The token is embedded in the reset link inside the mail body.
fn extract_reset_token_from_mail(mail: &sui_id_core::mail::OutgoingMail) -> String {
    let prefix = "/reset-password?token=";
    let start = mail.text_body.find(prefix).expect("reset link in mail") + prefix.len();
    let end = mail.text_body[start..]
        .find(|c: char| c == '\n' || c.is_whitespace())
        .map(|i| start + i)
        .unwrap_or(mail.text_body.len());
    mail.text_body[start..end].to_owned()
}

/// Helper: issue a forgot-password request and return the captured token.
async fn issue_reset_token(state: &AppState, mailer: &sui_id_core::mail::InMemoryMailSender, email: &str) -> String {
    // GET for CSRF cookie.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET forgot");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}&email={}", urlencode(email));
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
        .expect("POST forgot");
    assert_eq!(resp.status(), StatusCode::OK);

    let mail = mailer.last().await.expect("reset mail captured");
    extract_reset_token_from_mail(&mail)
}

/// Helper: redeem a reset token with a new password via POST /reset-password.
async fn redeem_reset_token(state: &AppState, token: &str, new_password: &str) {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&format!("/reset-password?token={token}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET reset");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!(
        "_csrf={csrf}&token={token}&password={pw}&confirm_password={pw}",
        pw = urlencode(new_password),
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/reset-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST reset");
    assert!(
        resp.status().is_redirection(),
        "expected redirect after reset, got {}",
        resp.status()
    );
}

/// Count the active sessions for the user identified by `username`.
async fn count_active_sessions(state: &AppState, username: &str) -> usize {
    let user = sui_id_store::repos::users::find_by_username(&state.db, username).await
        .expect("user row");
    sui_id_store::repos::sessions::list_active_for_user(&state.db, user.id).await
        .expect("sessions")
        .len()
}

/// Count the active (non-revoked) refresh tokens for the user identified by `username`.
async fn count_active_refresh_tokens(state: &AppState, username: &str) -> usize {
    let user = sui_id_store::repos::users::find_by_username(&state.db, username).await
        .expect("user row");
    let uid = user.id;
    state
        .db
        .with_conn(move |conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM refresh_tokens \
                 WHERE user_id = ?1 AND revoked_at IS NULL",
                [uid.to_string()],
                |r| r.get(0),
            )?;
            Ok(n as usize)
        })
        .await
        .expect("refresh count")
}

#[tokio::test]
async fn forgot_password_reset_revokes_all_sessions_and_refresh_tokens() {
    let (state, mailer) = test_app_with_mailer();

    // Setup + sign in to create a session.
    let session_a = complete_setup_and_login(&state).await;
    enable_smtp_in_db(&state).await;

    // Give the admin user an email so forgot-password can proceed.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("user");
    sui_id_store::repos::users::update_email(
        &state.db,
        user.id,
        Some("alice@reset.test"),
        chrono::Utc::now(),
    ).await
    .expect("set email");

    // Sign in again to get a second session.
    let _session_b = login_again_for_admin(&state, USERNAME, PASSWORD).await;

    // Sanity: two active sessions before the reset.
    assert_eq!(count_active_sessions(&state, USERNAME).await, 2, "expected 2 active sessions before reset");

    // Issue and redeem a forgot-password token.
    let token = issue_reset_token(&state, &mailer, "alice@reset.test").await;
    let new_pw = "totally-fresh-password-111";
    redeem_reset_token(&state, &token, new_pw).await;

    // RFC 010 assertion: zero active sessions and zero active refresh tokens
    // after the reset, regardless of how many existed before.
    assert_eq!(
        count_active_sessions(&state, USERNAME).await,
        0,
        "all sessions must be revoked after forgot-password reset (RFC 010)"
    );
    assert_eq!(
        count_active_refresh_tokens(&state, USERNAME).await,
        0,
        "all refresh tokens must be revoked after forgot-password reset (RFC 010)"
    );

    // The old session cookie must now be rejected.
    // The server responds with 401 (Unauthenticated) — the HTML error
    // handler, not a redirect — which is the correct behaviour for a
    // revoked session presented to an HTML endpoint.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/dashboard")
                .header(header::COOKIE, format!("sui_id_session={session_a}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("dashboard after reset");
    assert!(
        !resp.status().is_success(),
        "stale session cookie must be rejected after forgot-password reset (got {})",
        resp.status()
    );
}

#[tokio::test]
async fn forgot_password_reset_is_no_op_when_user_has_no_sessions() {
    // Ensure revoke with no pre-existing sessions doesn't fail.
    let (state, mailer) = test_app_with_mailer();
    let _session = complete_setup_and_login(&state).await;
    enable_smtp_in_db(&state).await;

    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("user");
    sui_id_store::repos::users::update_email(
        &state.db,
        user.id,
        Some("bob@reset.test"),
        chrono::Utc::now(),
    ).await
    .expect("set email");
    // Revoke the one session we created so we start with zero.
    let uid2 = user.id;
    state
        .db
        .with_conn(move |conn| {
            conn.execute(
                "UPDATE sessions SET revoked_at = datetime('now') WHERE user_id = ?1",
                [uid2.to_string()],
            )?;
            Ok(())
        })
        .await
        .expect("setup no-session state");

    assert_eq!(count_active_sessions(&state, USERNAME).await, 0, "pre-condition: no sessions");

    let token = issue_reset_token(&state, &mailer, "bob@reset.test").await;
    // Should succeed even with no sessions to revoke.
    redeem_reset_token(&state, &token, "fresh-pass-no-sessions-987").await;

    assert_eq!(count_active_sessions(&state, USERNAME).await, 0);
    assert_eq!(count_active_refresh_tokens(&state, USERNAME).await, 0);
}
