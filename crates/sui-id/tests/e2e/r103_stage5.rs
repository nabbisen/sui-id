//! RFC 103 stage 5 — D7 notices, the account-page line, U06's retirement.
//!
//! **5a.** The post-reset "your password was changed" notice, at completion,
//! goes only to the address an email-origin link proved. A web- or
//! CLI-issued link's completion sends nothing, and neither does its
//! issuance: RFC 101 has not landed verified addresses yet, so there is no
//! proven address to notify with for those two origins.
//!
//! **5b.** `/me/security/overview` shows the signed-in user's own most
//! recent recovery-link event (issued, or completed), naming who acted by
//! `via` and never a token.
//!
//! **5c** (retiring U06) has no runtime surface: it is proven by the
//! workspace compiling with every reference removed, checked in the review
//! package rather than here.

use super::common::*;
use super::r102_stage1::sign_in;
use super::r103_stage1::{complete, exec, issue_token, reset_app, send};
use super::r103_stage3::{Admin, admin, stepped_up, target_user};
use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::AppState;
use sui_id_core::recovery_link;
use sui_id_shared::ids::UserId;

const REASON: &str = "caller verified by call-back, ticket 4711";

/// Like [`super::r103_stage3::target_user`], with an email address set — so a
/// test can tell "no notice was sent" from "there was never anywhere to send
/// one".
async fn target_user_with_email(a: &Admin, name: &str, email: &str) -> UserId {
    let id = target_user(a, name).await;
    sui_id_store::repos::users::update_email(&a.state.db, id, Some(email), chrono::Utc::now())
        .await
        .expect("set email");
    id
}

/// [`admin`], over an app that also captures mail, for the notice tests.
async fn admin_with_mailer() -> (Admin, std::sync::Arc<sui_id_core::mail::InMemoryMailSender>) {
    let (state, mailer) = test_app_with_mailer();
    let session = complete_setup_and_login(&state).await;
    let id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    exec(
        &state,
        format!(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at) \
             VALUES ('{}', '{id}', X'0102', X'00', 'k', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;
    stepped_up(&state, &session).await;
    (Admin { state, id, session }, mailer)
}

// ── 5a: D7 notices ──────────────────────────────────────────────────

#[tokio::test]
async fn r103_s5_an_email_origin_completion_sends_the_changed_notice_to_the_proven_address() {
    let (state, mailer, _admin) = reset_app().await;
    let (token, _) = issue_token(&state, &mailer).await;
    assert_eq!(mailer.count().await, 1, "just the reset-link mail so far");
    let r = complete(&state, &token).await;
    assert!(r.status.is_redirection(), "{}", r.status);
    assert_eq!(mailer.count().await, 2, "and now the completion notice too");
    let notice = mailer.last().await.expect("notice");
    assert_eq!(
        notice.to, "alice@test.invalid",
        "the address the link proved"
    );
    assert_ne!(
        notice.subject,
        mailer.drain().await[0].subject,
        "a different mail from the reset-link one"
    );
}

#[tokio::test]
async fn r103_s5_a_web_issued_link_sends_no_notice_at_issuance_or_completion() {
    let (a, mailer) = admin_with_mailer().await;
    let bob = target_user_with_email(&a, "bob", "bob@test.invalid").await;
    let link = recovery_link::issue_as_admin(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        REASON,
    )
    .await
    .expect("issue");
    assert_eq!(mailer.count().await, 0, "nothing sent at issuance");

    let r = complete(&a.state, link.token.expose()).await;
    assert!(r.status.is_redirection(), "{}", r.status);
    assert_eq!(mailer.count().await, 0, "and nothing at completion either");
}

#[tokio::test]
async fn r103_s5_a_cli_issued_link_sends_no_notice_at_issuance_or_completion() {
    let (a, mailer) = admin_with_mailer().await;
    let _bob = target_user_with_email(&a, "bob", "bob@test.invalid").await;
    let (_, link) = recovery_link::issue_as_operator(&a.state.db, &a.state.clock, "bob", REASON)
        .await
        .expect("issue");
    assert_eq!(mailer.count().await, 0, "nothing sent at issuance");

    let r = complete(&a.state, link.token.expose()).await;
    assert!(r.status.is_redirection(), "{}", r.status);
    assert_eq!(mailer.count().await, 0, "and nothing at completion either");
}

#[tokio::test]
async fn r103_s5_a_web_issued_link_for_a_user_with_no_email_still_completes_and_sends_nothing() {
    // The email-origin gate is on `issued_via`, not on whether the user has
    // an address; this pins that a addressless target still works end to
    // end (D7's "nothing is sent" is not standing in for a crash).
    let (a, mailer) = admin_with_mailer().await;
    let bob = target_user(&a, "bob").await; // no email
    let link = recovery_link::issue_as_admin(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        REASON,
    )
    .await
    .expect("issue");
    let r = complete(&a.state, link.token.expose()).await;
    assert!(r.status.is_redirection(), "{}", r.status);
    assert_eq!(mailer.count().await, 0);
}

// ── 5b: the account-page recovery-link line ──────────────────────────

async fn set_lang_en(state: &AppState, user_id: UserId) {
    sui_id_store::repos::users::set_preferred_lang(
        &state.db,
        user_id,
        Some("en"),
        chrono::Utc::now(),
    )
    .await
    .expect("set lang");
}

async fn overview_body(state: &AppState, session: &str) -> String {
    let r = send(
        state,
        Request::builder()
            .method(Method::GET)
            .uri("/me/security/overview")
            .header(header::COOKIE, format!("sui_id_session={session}"))
            .body(Body::empty())
            .expect("req"),
    )
    .await;
    assert_eq!(r.status, axum::http::StatusCode::OK, "{}", r.body);
    r.body
}

const ISSUED_BY_ADMIN: &str = "An administrator issued you an account-recovery link on";
const ISSUED_BY_OPERATOR: &str = "The host operator issued you an account-recovery link on";
const COMPLETED_SELF: &str = "You reset your password using an emailed link on";
const COMPLETED_BY_ADMIN: &str = "using a link an administrator issued.";
const COMPLETED_BY_OPERATOR: &str = "using a link the host operator issued.";
const ALL_RECOVERY_FRAGMENTS: [&str; 5] = [
    ISSUED_BY_ADMIN,
    ISSUED_BY_OPERATOR,
    COMPLETED_SELF,
    COMPLETED_BY_ADMIN,
    COMPLETED_BY_OPERATOR,
];

/// Assert that exactly `expected` of the five recovery-line fragments is on
/// the page — never more than one at once, since it is one line about the
/// single most recent event.
fn assert_only(body: &str, expected: &str) {
    for f in ALL_RECOVERY_FRAGMENTS {
        assert_eq!(
            body.contains(f),
            f == expected,
            "fragment {f:?} in:\n{body}"
        );
    }
}

#[tokio::test]
async fn r103_s5_overview_shows_nothing_with_no_recovery_event() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    set_lang_en(&a.state, bob).await;
    let session = sign_in(&a.state, "bob", "target-very-strong-password").await;
    let body = overview_body(&a.state, &session).await;
    for f in ALL_RECOVERY_FRAGMENTS {
        assert!(
            !body.contains(f),
            "fragment {f:?} should not appear:\n{body}"
        );
    }
}

#[tokio::test]
async fn r103_s5_overview_shows_a_link_issued_by_an_administrator() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    set_lang_en(&a.state, bob).await;
    recovery_link::issue_as_admin(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        REASON,
    )
    .await
    .expect("issue");
    let session = sign_in(&a.state, "bob", "target-very-strong-password").await;
    let body = overview_body(&a.state, &session).await;
    assert_only(&body, ISSUED_BY_ADMIN);
}

#[tokio::test]
async fn r103_s5_overview_shows_a_link_issued_by_the_operator() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    set_lang_en(&a.state, bob).await;
    recovery_link::issue_as_operator(&a.state.db, &a.state.clock, "bob", REASON)
        .await
        .expect("issue");
    let session = sign_in(&a.state, "bob", "target-very-strong-password").await;
    let body = overview_body(&a.state, &session).await;
    assert_only(&body, ISSUED_BY_OPERATOR);
}

#[tokio::test]
async fn r103_s5_overview_shows_a_web_completed_reset() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    set_lang_en(&a.state, bob).await;
    let link = recovery_link::issue_as_admin(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        REASON,
    )
    .await
    .expect("issue");
    let done = complete(&a.state, link.token.expose()).await;
    assert!(done.status.is_redirection(), "{}", done.status);
    // Completion revoked bob's other sessions; sign in fresh with the new
    // password he chose at `/reset-password`.
    let session = sign_in(&a.state, "bob", "brand-new-secure-pw-12345").await;
    let body = overview_body(&a.state, &session).await;
    assert_only(&body, COMPLETED_BY_ADMIN);
}

#[tokio::test]
async fn r103_s5_overview_shows_an_operator_completed_reset() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    set_lang_en(&a.state, bob).await;
    let (_, link) = recovery_link::issue_as_operator(&a.state.db, &a.state.clock, "bob", REASON)
        .await
        .expect("issue");
    let done = complete(&a.state, link.token.expose()).await;
    assert!(done.status.is_redirection(), "{}", done.status);
    let session = sign_in(&a.state, "bob", "brand-new-secure-pw-12345").await;
    let body = overview_body(&a.state, &session).await;
    assert_only(&body, COMPLETED_BY_OPERATOR);
}

#[tokio::test]
async fn r103_s5_overview_shows_a_self_service_completion() {
    let (state, mailer, admin_id) = reset_app().await;
    set_lang_en(&state, admin_id).await;
    let (token, _) = issue_token(&state, &mailer).await;
    let done = complete(&state, &token).await;
    assert!(done.status.is_redirection(), "{}", done.status);
    let session = sign_in(&state, USERNAME, "brand-new-secure-pw-12345").await;
    let body = overview_body(&state, &session).await;
    assert_only(&body, COMPLETED_SELF);
}

#[tokio::test]
async fn r103_s5_overview_shows_the_most_recent_event_not_the_first() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    set_lang_en(&a.state, bob).await;
    let link = recovery_link::issue_as_admin(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        REASON,
    )
    .await
    .expect("issue");
    // Before completion: the issued line, and only it.
    let session = sign_in(&a.state, "bob", "target-very-strong-password").await;
    assert_only(&overview_body(&a.state, &session).await, ISSUED_BY_ADMIN);

    let done = complete(&a.state, link.token.expose()).await;
    assert!(done.status.is_redirection(), "{}", done.status);
    // After completion: the completed line displaces the issued one.
    let session = sign_in(&a.state, "bob", "brand-new-secure-pw-12345").await;
    assert_only(&overview_body(&a.state, &session).await, COMPLETED_BY_ADMIN);
}

#[tokio::test]
async fn r103_s5_overview_never_shows_another_users_event() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    set_lang_en(&a.state, bob).await;
    recovery_link::issue_as_admin(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        REASON,
    )
    .await
    .expect("issue");

    // The issuing administrator is the actor, not the target: their own
    // overview shows nothing.
    let admin_body = overview_body(&a.state, &a.session).await;
    for f in ALL_RECOVERY_FRAGMENTS {
        assert!(!admin_body.contains(f), "the actor's own page: {f:?}");
    }

    // A bystander, unrelated to bob's link, shows nothing either.
    let carol = target_user(&a, "carol").await;
    set_lang_en(&a.state, carol).await;
    let carol_session = sign_in(&a.state, "carol", "target-very-strong-password").await;
    let carol_body = overview_body(&a.state, &carol_session).await;
    for f in ALL_RECOVERY_FRAGMENTS {
        assert!(!carol_body.contains(f), "a bystander's page: {f:?}");
    }
}

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

#[tokio::test]
async fn r103_s5_completing_a_web_issued_link_leaves_mfa_untouched() {
    // RFC 103 D4, T6: the link resets the password and nothing else — this
    // pins that the enrolment itself is byte-for-byte unchanged.
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    exec(
        &a.state,
        format!(
            "INSERT INTO user_totp (user_id, secret_enc, enabled, created_at) \
             VALUES ('{bob}', X'00aabbcc', 1, '2026-01-01T00:00:00Z')"
        ),
    )
    .await;
    let link = recovery_link::issue_as_admin(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        REASON,
    )
    .await
    .expect("issue");
    let done = complete(&a.state, link.token.expose()).await;
    assert!(done.status.is_redirection(), "{}", done.status);

    let still_enrolled = scalar(
        &a.state,
        format!(
            "SELECT COUNT(*) FROM user_totp WHERE user_id = '{bob}' \
             AND enabled = 1 AND secret_enc = X'00aabbcc'"
        ),
    )
    .await;
    assert_eq!(
        still_enrolled, 1,
        "the TOTP enrolment is untouched by the reset"
    );
}
