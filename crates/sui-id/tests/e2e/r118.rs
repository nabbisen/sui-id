//! RFC 118 — a credential change clears the *password* lockout, and the
//! holder of a consumed reset token is told what was cleared.
//!
//! The defect first: lock an account through the real sign-in endpoint, change
//! its password through a real reset link or the self-service form, and sign in
//! with the new password. Then D1's carve-out (the second-factor lock survives),
//! D4's message and its boundary (it is reachable only in the response to a
//! completion that consumed a token, and no other response changes), and D5's
//! audit attribute.

use super::common::*;
use super::r103_stage1::{Resp, csrf_from, exec, get, mint_token, send};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::AppState;
use sui_id_shared::ids::UserId;

const NEW_PASSWORD: &str = super::r103_stage1::NEW_PASSWORD;
const WRONG: &str = "definitely-not-the-password";
const TOKEN: &str = "r118-plaintext-reset-token-0123456789";

// ── helpers ──────────────────────────────────────────────────────────

async fn app() -> (AppState, UserId) {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("alice")
        .id;
    (state, id)
}

async fn sign_in(state: &AppState, password: &str) -> Resp {
    send(
        state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username={USERNAME}&password={}",
                urlencode(password)
            )))
            .expect("req"),
    )
    .await
}

/// Lock alice through the real endpoint: three wrong passwords stamp a
/// thirty-second lock (the limiter is ten a minute, so a small count keeps
/// the test clear of it).
async fn lock_through_sign_in(state: &AppState) {
    for _ in 0..3 {
        assert_eq!(sign_in(state, WRONG).await.status, StatusCode::UNAUTHORIZED);
    }
    let alice = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("alice");
    assert_eq!(alice.failed_login_count, 3);
    assert!(alice.locked_until.is_some(), "the account is locked");
    // The correct password is refused while it is.
    assert_eq!(
        sign_in(state, PASSWORD).await.status,
        StatusCode::UNAUTHORIZED
    );
}

/// The user's second-factor lock: `mfa_failure_count` at the threshold and a
/// lock until a fixed time. Returns the stamped time as the database holds it.
async fn second_factor_lock(state: &AppState) -> String {
    exec(
        state,
        "UPDATE users SET mfa_failure_count = 5, failed_login_count = 2, \
         locked_until = '2999-01-02T03:04:00Z' WHERE username = 'alice'"
            .into(),
    )
    .await;
    "2999-01-02 03:04 UTC".to_owned()
}

async fn columns(state: &AppState) -> (i64, Option<String>, i64) {
    state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT failed_login_count, locked_until, mfa_failure_count FROM users \
                 WHERE username = 'alice'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .await
        .expect("columns")
}

fn complete_with(csrf: &str, token: &str, password: &str, confirm: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/reset-password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
        .body(Body::from(format!(
            "_csrf={csrf}&token={}&password={}&confirm_password={}",
            urlencode(token),
            urlencode(password),
            urlencode(confirm)
        )))
        .expect("req")
}

/// Complete a reset with the given password. Returns the response and the
/// CSRF value the request carried (so a body can be normalised).
async fn complete_reset(state: &AppState, token: &str, password: &str) -> (Resp, String) {
    let csrf = csrf_from(state, "/reset-password").await;
    let resp = send(state, complete_with(&csrf, token, password, password)).await;
    (resp, csrf)
}

async fn last_note(state: &AppState, action: &str) -> String {
    let sql = format!(
        "SELECT note FROM audit_log WHERE action = '{action}' ORDER BY at DESC, rowid DESC LIMIT 1"
    );
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get::<_, Option<String>>(0))?))
        .await
        .expect("note")
        .unwrap_or_default()
}

/// A completion that took: the page (RFC 118) or, before it, the redirect.
fn completed(r: &Resp) -> bool {
    r.status.is_success() || r.status.is_redirection()
}

/// The success page, as the tests recognise it.
const DONE: &str = r#"id="reset-done""#;
const CLEARED: &str = r#"data-lockout="cleared""#;
const KEPT: &str = r#"data-lockout="second-factor""#;

// ── the defect, on both paths ────────────────────────────────────────

#[tokio::test]
async fn r118_a_reset_link_lets_the_user_sign_in_after_a_lockout() {
    let (state, alice) = app().await;
    lock_through_sign_in(&state).await;
    mint_token(&state, alice, TOKEN).await;

    let (done, _) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    assert!(completed(&done), "{} {}", done.status, done.body);

    let r = sign_in(&state, NEW_PASSWORD).await;
    assert_eq!(
        r.status,
        StatusCode::SEE_OTHER,
        "the password just set must sign in"
    );
    assert_eq!(columns(&state).await, (0, None, 0));
}

#[tokio::test]
async fn r118_a_self_service_change_lets_the_user_sign_in_after_a_lockout() {
    let (state, _alice) = app().await;
    // The session is taken before the lock: a lock refuses a *sign-in*, not a
    // session already held, which is how a user reaches the change form.
    let session = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    lock_through_sign_in(&state).await;

    let csrf = csrf_from_page(&state, &session, "/me/security/password").await;
    let body = format!(
        "_csrf={csrf}&current_password={}&new_password={}&confirm_password={}",
        urlencode(PASSWORD),
        urlencode(NEW_PASSWORD),
        urlencode(NEW_PASSWORD)
    );
    let r = send(
        &state,
        Request::builder()
            .method(Method::POST)
            .uri("/me/security/password")
            .header(
                header::COOKIE,
                format!("sui_id_session={session}; sui_id_csrf={csrf}"),
            )
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .expect("req"),
    )
    .await;
    assert!(r.status.is_redirection(), "change: {}", r.status);

    assert_eq!(columns(&state).await, (0, None, 0));
    assert_eq!(
        sign_in(&state, NEW_PASSWORD).await.status,
        StatusCode::SEE_OTHER
    );
}

async fn csrf_from_page(state: &AppState, session: &str, uri: &str) -> String {
    let r = send(
        state,
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .header(header::COOKIE, format!("sui_id_session={session}"))
            .body(Body::empty())
            .expect("req"),
    )
    .await;
    extract_set_cookie(&r.headers, "sui_id_csrf").expect("csrf cookie")
}

// ── D1's carve-out, over HTTP ────────────────────────────────────────

#[tokio::test]
async fn r118_a_reset_does_not_lift_the_second_factor_lock() {
    let (state, alice) = app().await;
    let until = second_factor_lock(&state).await;
    mint_token(&state, alice, TOKEN).await;

    let (done, _) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    assert!(completed(&done), "{}", done.status);

    let (password_failures, locked_until, mfa_failures) = columns(&state).await;
    assert_eq!(password_failures, 0, "the password counter is cleared");
    assert!(
        locked_until
            .as_deref()
            .is_some_and(|t| t.starts_with("2999-01-02")),
        "the second-factor lock is kept: {locked_until:?}"
    );
    assert_eq!(mfa_failures, 5, "and so is its count");
    assert_eq!(
        sign_in(&state, NEW_PASSWORD).await.status,
        StatusCode::UNAUTHORIZED,
        "a reset token does not buy sign-in past a second-factor lock"
    );
    // D4: the user is told when it lifts, as a time.
    assert!(done.body.contains(KEPT), "{}", done.body);
    assert!(done.body.contains(&until), "{}", done.body);
}

// ── D4: the message ──────────────────────────────────────────────────

#[tokio::test]
async fn r118_the_completion_response_says_what_was_cleared() {
    let (state, alice) = app().await;
    lock_through_sign_in(&state).await;
    mint_token(&state, alice, TOKEN).await;

    let (done, _) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    assert_eq!(done.status, StatusCode::OK);
    assert!(done.body.contains(DONE));
    assert!(done.body.contains(CLEARED), "{}", done.body);
    assert!(!done.body.contains(KEPT));
    assert!(
        done.location.is_none(),
        "the message is rendered in the response, not behind a redirect"
    );
    let cache = done.headers.get(header::CACHE_CONTROL).expect("cache");
    assert_eq!(cache, "no-store");
    let referrer = done.headers.get(header::REFERRER_POLICY).expect("referrer");
    assert_eq!(referrer, "no-referrer");
}

#[tokio::test]
async fn r118_it_never_shows_a_count_or_a_source() {
    let (state, alice) = app().await;
    exec(
        &state,
        "UPDATE users SET failed_login_count = 7777, \
         locked_until = '2999-01-01T00:00:00Z' WHERE username = 'alice'"
            .into(),
    )
    .await;
    mint_token(&state, alice, TOKEN).await;
    let (done, _) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    assert!(done.body.contains(CLEARED));
    assert!(!done.body.contains("7777"), "no attempt count");
    assert!(
        !done.body.contains("2999"),
        "no lock time for a cleared lock"
    );
    assert!(!done.body.contains("127.0.0.1"), "no source");
}

#[tokio::test]
async fn r118_an_account_with_nothing_to_clear_gets_a_plain_confirmation() {
    let (state, alice) = app().await;
    mint_token(&state, alice, TOKEN).await;
    let (done, _) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    assert_eq!(done.status, StatusCode::OK);
    assert!(done.body.contains(DONE));
    assert!(!done.body.contains(CLEARED));
    assert!(!done.body.contains(KEPT));
}

#[tokio::test]
async fn r118_the_message_appears_once_a_replay_gets_the_invalid_link_page() {
    let (state, alice) = app().await;
    lock_through_sign_in(&state).await;
    mint_token(&state, alice, TOKEN).await;
    let (first, _) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    assert!(first.body.contains(CLEARED));

    let (replay, _) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    assert_eq!(replay.status, StatusCode::BAD_REQUEST);
    assert!(!replay.body.contains(DONE));
    assert!(!replay.body.contains(CLEARED));
    assert!(replay.body.contains(r#"href="/forgot-password""#));
}

#[tokio::test]
async fn r118_the_locale_of_the_request_picks_the_language() {
    for (tag, marker) in [
        ("en", "Your password is set"),
        ("ja", "パスワードを設定しました"),
    ] {
        let (state, alice) = app().await;
        lock_through_sign_in(&state).await;
        mint_token(&state, alice, TOKEN).await;
        let csrf = csrf_from(&state, "/reset-password").await;
        let mut req = complete_with(&csrf, TOKEN, NEW_PASSWORD, NEW_PASSWORD);
        req.headers_mut()
            .insert(header::ACCEPT_LANGUAGE, tag.parse().expect("header"));
        let done = send(&state, req).await;
        assert!(done.body.contains(marker), "{tag}: {}", done.body);
    }
}

// ── D4's boundary: nothing else changes ──────────────────────────────

/// A response with the per-request CSRF value normalised away.
#[derive(Debug, PartialEq, Eq)]
struct Shape {
    status: StatusCode,
    location: Option<String>,
    cache_control: Option<String>,
    body: String,
}

fn shape(r: Resp, csrf: &str) -> Shape {
    Shape {
        status: r.status,
        location: r.location,
        cache_control: r
            .headers
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        body: r.body.replace(csrf, "CSRF"),
    }
}

#[derive(Clone, Copy, Debug)]
enum Refused {
    NoToken,
    UnknownToken,
    Expired,
    Revoked,
    Replayed,
    TooShort,
    Mismatch,
}

const ALL_REFUSED: [Refused; 7] = [
    Refused::NoToken,
    Refused::UnknownToken,
    Refused::Expired,
    Refused::Revoked,
    Refused::Replayed,
    Refused::TooShort,
    Refused::Mismatch,
];

/// Run one refused completion on a fresh app, with alice locked or not.
async fn refused(kind: Refused, locked: bool) -> Shape {
    let (state, alice) = app().await;
    if locked {
        lock_through_sign_in(&state).await;
    }
    let id = mint_token(&state, alice, TOKEN).await;
    match kind {
        Refused::Expired => {
            exec(
                &state,
                format!("UPDATE password_reset_tokens SET expires_at = '2000-01-01T00:00:00Z' WHERE id = '{id}'"),
            )
            .await
        }
        Refused::Revoked => {
            exec(
                &state,
                format!("UPDATE password_reset_tokens SET revoked_at = '2000-01-01T00:00:00Z' WHERE id = '{id}'"),
            )
            .await
        }
        Refused::Replayed => {
            let (first, _) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
            assert!(completed(&first), "{}", first.status);
        }
        _ => {}
    }
    let csrf = csrf_from(&state, "/reset-password").await;
    let req = match kind {
        Refused::NoToken => complete_with(&csrf, "", NEW_PASSWORD, NEW_PASSWORD),
        Refused::UnknownToken => complete_with(&csrf, "no-such-token", NEW_PASSWORD, NEW_PASSWORD),
        Refused::TooShort => complete_with(&csrf, TOKEN, "short", "short"),
        Refused::Mismatch => complete_with(&csrf, TOKEN, NEW_PASSWORD, "something-else-entirely"),
        _ => complete_with(&csrf, TOKEN, NEW_PASSWORD, NEW_PASSWORD),
    };
    let r = send(&state, req).await;
    shape(r, &csrf)
}

#[tokio::test]
async fn r118_every_refused_completion_is_the_same_whether_or_not_the_account_was_locked() {
    for kind in ALL_REFUSED {
        let unlocked = refused(kind, false).await;
        let locked = refused(kind, true).await;
        assert_eq!(locked, unlocked, "{kind:?}");
        assert_ne!(unlocked.status, StatusCode::OK, "{kind:?} is a refusal");
        assert!(!unlocked.body.contains(DONE), "{kind:?}");
        assert!(!locked.body.contains(CLEARED), "{kind:?}");
    }
}

#[tokio::test]
async fn r118_a_breached_password_in_block_mode_is_refused_alike() {
    let mut shapes = Vec::new();
    for locked in [false, true] {
        let (state, _mailer, hibp) = test_app_with_hibp();
        complete_setup_and_login(&state).await;
        set_hibp_mode(&state, sui_id_store::models::HibpMode::Block).await;
        hibp.set_breached(NEW_PASSWORD, 4242);
        let alice = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
            .await
            .expect("alice")
            .id;
        if locked {
            lock_through_sign_in(&state).await;
        }
        mint_token(&state, alice, TOKEN).await;
        let (r, csrf) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
        shapes.push(shape(r, &csrf));
    }
    assert_eq!(shapes[0], shapes[1]);
}

#[tokio::test]
async fn r118_a_refused_completion_clears_nothing() {
    // The clear is inside the credential's transaction: a refusal that never
    // reaches it leaves the lock exactly as it was.
    let (state, alice) = app().await;
    lock_through_sign_in(&state).await;
    let before = columns(&state).await;
    mint_token(&state, alice, TOKEN).await;
    let csrf = csrf_from(&state, "/reset-password").await;
    let r = send(&state, complete_with(&csrf, TOKEN, "short", "short")).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(columns(&state).await, before);
}

#[tokio::test]
async fn r118_the_sign_in_page_ignores_any_query() {
    let (state, alice) = app().await;
    lock_through_sign_in(&state).await;
    mint_token(&state, alice, TOKEN).await;
    let (done, _) = complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    assert!(done.body.contains(CLEARED));

    let plain = get(&state, "/admin/login").await;
    let csrf_plain = extract_set_cookie(&plain.headers, "sui_id_csrf").unwrap_or_default();
    let plain = shape(plain, &csrf_plain);
    for q in ["?reset=ok", "?reset=ok&locked=1", "?cleared=1&locked=1"] {
        let r = get(&state, &format!("/admin/login{q}")).await;
        let csrf = extract_set_cookie(&r.headers, "sui_id_csrf").unwrap_or_default();
        assert_eq!(shape(r, &csrf), plain, "{q}");
    }
    assert!(!plain.body.contains(CLEARED) && !plain.body.contains(DONE));
}

#[tokio::test]
async fn r118_the_reset_form_itself_says_nothing_about_lockout() {
    // GET /reset-password (the form) does not mention lockout at all.
    let (state, _alice) = app().await;
    let form = get(&state, "/reset-password").await;
    assert_eq!(form.status, StatusCode::OK);
    assert!(!form.body.contains("lockout") && !form.body.contains(DONE));
}

// ── D5 ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn r118_the_audit_row_records_that_a_lock_was_cleared() {
    let (state, alice) = app().await;
    lock_through_sign_in(&state).await;
    mint_token(&state, alice, TOKEN).await;
    complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    let note = last_note(&state, "auth.password.reset_completed").await;
    assert!(note.contains("lockout_cleared=3"), "{note}");

    // The story reads in order: lockout, then the reset that cleared it.
    let (state, alice) = app().await;
    mint_token(&state, alice, TOKEN).await;
    complete_reset(&state, TOKEN, NEW_PASSWORD).await;
    let note = last_note(&state, "auth.password.reset_completed").await;
    assert!(!note.contains("lockout_cleared"), "{note}");
}

// ── what this does not do ────────────────────────────────────────────

#[tokio::test]
async fn r118_the_lock_can_still_be_renewed_but_from_zero() {
    // The residual, pinned: after a clear the counter is zero, so three fresh
    // failures are needed to lock again, not one.
    let (state, alice) = app().await;
    lock_through_sign_in(&state).await;
    mint_token(&state, alice, TOKEN).await;
    complete_reset(&state, TOKEN, NEW_PASSWORD).await;

    assert_eq!(
        sign_in(&state, WRONG).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        columns(&state).await,
        (1, None, 0),
        "one failure: counted, not locked"
    );
    assert_eq!(
        sign_in(&state, NEW_PASSWORD).await.status,
        StatusCode::SEE_OTHER
    );
}
