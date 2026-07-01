//! Password-change notification e-mail (v0.22.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::{AppState, build_router};

use super::common::*;
use tower::ServiceExt;

// ---------- v0.22.0: password-change notification mail ----------

/// Helper: set the admin's email column directly in the DB. Setup
/// runs through `/setup/admin` which doesn't accept an email
/// post-setup; we bypass that by writing the column ourselves.
async fn set_admin_email(state: &AppState, email: &str) {
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("user");
    let uid = user.id;
    let email_owned = email.to_owned();
    state
        .db
        .with_conn(move |conn| {
            conn.execute(
                "UPDATE users SET email = ?1, email_normalized = lower(trim(?1)) WHERE id = ?2",
                rusqlite::params![email_owned, uid.to_string()],
            )
            .expect("update");
            Ok(())
        })
        .await
        .expect("set email");
}

#[tokio::test]
async fn password_change_sends_notification_mail_when_email_is_set() {
    let (state, mailer) = test_app_with_mailer();
    let s1 = complete_setup_and_login(&state).await;
    set_admin_email(&state, "alice@example.test").await;

    // GET to obtain CSRF cookie.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let new_pw = "the-new-tester-password";
    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode(PASSWORD),
        urlencode(new_pw),
        urlencode(new_pw)
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert!(resp.status().is_redirection());

    // Drain captured emails. We expect exactly one — the
    // "your password has changed" notification.
    let sent = mailer.drain().await;
    assert_eq!(
        sent.len(),
        1,
        "expected one notification email, got {}",
        sent.len()
    );
    let mail = &sent[0];
    assert_eq!(mail.to, "alice@example.test");
    assert!(
        mail.subject.contains("パスワード"),
        "subject should mention password: {}",
        mail.subject
    );
    assert!(
        mail.text_body.contains("変更"),
        "body should mention 'changed': {}",
        mail.text_body
    );
}

#[tokio::test]
async fn password_change_sends_no_mail_when_email_is_unset() {
    let (state, mailer) = test_app_with_mailer();
    let s1 = complete_setup_and_login(&state).await;
    // Deliberately do NOT set the admin's email.

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let new_pw = "the-new-tester-password";
    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode(PASSWORD),
        urlencode(new_pw),
        urlencode(new_pw)
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert!(resp.status().is_redirection());

    let sent = mailer.drain().await;
    assert!(
        sent.is_empty(),
        "expected no notification mail (no email on user), got {}",
        sent.len()
    );
}
