//! R11 Part 1 — login failures through the real handler.
//!
//! - 1a: with `audit_log` inserts failing, a wrong password for a known
//!   user gets the same 401 as an unknown user, the failure counter does
//!   not advance, and a correct password still signs in.
//! - 1b: a non-credential failure is logged at error level; an ordinary
//!   credential failure is not; the password never appears.
//! - 1c: every failure branch returns an identical status, body and
//!   metric increment.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::io::Write;
use std::sync::{Arc, Mutex};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::UserId;
use sui_id_store::metrics::{Metrics, signin_result};
use sui_id_store::models::{CredentialRow, Role, UserRow, UserSource};
use sui_id_store::repos::{credentials, users};
use tower::ServiceExt;

const WRONG: &str = "definitely-not-the-password";

fn app_with_metrics() -> AppState {
    let mut state = test_app();
    state.metrics = Some(Metrics::new().expect("metrics"));
    state
}

fn wrong_password_count(state: &AppState) -> u64 {
    state
        .metric()
        .expect("metrics enabled")
        .signin_attempts_total
        .with_label_values(&[signin_result::WRONG_PASSWORD])
        .get()
}

async fn post_login(
    state: &AppState,
    username: &str,
    password: &str,
) -> (StatusCode, Vec<u8>, bool) {
    let router = build_router(state.clone());
    let body = format!(
        "username={}&password={}&next=",
        urlencode(username),
        urlencode(password)
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    let status = resp.status();
    let has_session = extract_set_cookie(resp.headers(), "sui_id_session").is_some();
    let bytes = read_body(resp.into_body()).await;
    (status, bytes, has_session)
}

/// A local user with a password, optionally disabled or locked.
async fn seed_user(state: &AppState, username: &str, password: &str) -> UserId {
    let now = chrono::Utc::now();
    let uid = UserId::new();
    users::create(
        &state.db,
        &UserRow {
            id: uid,
            username: username.into(),
            display_name: None,
            is_admin: false,
            role: Role::User,
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: UserSource::Local,
            external_stable_id: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("create user");
    credentials::upsert(
        &state.db,
        &CredentialRow {
            user_id: uid,
            password_hash: sui_id_core::password::hash_password(password).expect("hash"),
            must_change: false,
            updated_at: now,
        },
    )
    .await
    .expect("credential");
    uid
}

async fn exec(state: &AppState, sql: &'static str) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(sql)?))
        .await
        .expect("sql");
}

async fn failed_login_count(state: &AppState, username: &'static str) -> i64 {
    state
        .db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT failed_login_count FROM users WHERE username = ?1",
                [username],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count")
}

/// Make every insert into `audit_log`, and only `audit_log`, fail.
async fn break_audit_log(state: &AppState) {
    exec(
        state,
        "CREATE TRIGGER r11_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'r11 test: audit_log insert rejected'); END;",
    )
    .await;
}

// ── 1a ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn r11_1a_audit_outage_known_user_wrong_password() {
    // Control: with the audit log working, a wrong password is counted.
    let control = test_app();
    complete_setup_and_login(&control).await;
    let (status, _, _) = post_login(&control, USERNAME, WRONG).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(failed_login_count(&control, USERNAME).await, 1);

    let state = test_app();
    complete_setup_and_login(&state).await;
    break_audit_log(&state).await;

    let (unknown_status, unknown_body, _) = post_login(&state, "nobody-here", WRONG).await;
    let (known_status, known_body, _) = post_login(&state, USERNAME, WRONG).await;
    assert_eq!(unknown_status, StatusCode::UNAUTHORIZED);
    assert_eq!(known_status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        known_body, unknown_body,
        "known-user store failure must look like any failure"
    );
    assert_eq!(
        failed_login_count(&state, USERNAME).await,
        0,
        "U22 rolled back: the wrong password was not counted"
    );

    let (ok_status, _, has_session) = post_login(&state, USERNAME, PASSWORD).await;
    assert_eq!(ok_status, StatusCode::SEE_OTHER);
    assert!(
        has_session,
        "the correct password still signs in during the outage"
    );
}

// ── 1b ────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("lock")).into_owned()
    }
    fn clear(&self) {
        self.0.lock().expect("lock").clear();
    }
}

#[tokio::test]
async fn r11_1b_non_credential_failure_is_logged_without_password() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    break_audit_log(&state).await;

    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    // Ordinary credential failure: no error line.
    post_login(&state, "nobody-here", WRONG).await;
    let ordinary = captured.text();
    assert!(
        !ordinary.contains("other than invalid credentials"),
        "an ordinary credential failure must not be logged as an error:\n{ordinary}"
    );
    captured.clear();

    // Store failure from U22: one error line, carrying the request id.
    post_login(&state, USERNAME, WRONG).await;
    let logged = captured.text();
    let line = logged
        .lines()
        .find(|l| l.contains("other than invalid credentials"))
        .unwrap_or_else(|| panic!("no error line for the store failure:\n{logged}"));
    assert!(line.contains("ERROR"), "wrong level: {line}");
    assert!(line.contains("request_id="), "no request id: {line}");
    assert!(
        line.contains("r11 test: audit_log insert rejected"),
        "cause missing: {line}"
    );
    assert!(
        !logged.contains(WRONG),
        "the submitted password leaked into the log"
    );
}

// ── 1c ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn r11_1c_every_failure_branch_is_identical() {
    let state = app_with_metrics();
    complete_setup_and_login(&state).await;
    seed_user(&state, "carol", "carol-correct-password").await;
    seed_user(&state, "dave", "dave-correct-password").await;
    exec(
        &state,
        "UPDATE users SET is_disabled = 1 WHERE username = 'carol'",
    )
    .await;
    exec(
        &state,
        "UPDATE users SET locked_until = '2999-01-01T00:00:00Z' WHERE username = 'dave'",
    )
    .await;

    // The store-failure branch needs a broken audit log, so it runs on its
    // own app instance; the response must still match byte for byte.
    let broken = app_with_metrics();
    complete_setup_and_login(&broken).await;
    break_audit_log(&broken).await;

    let branches: [(&str, &AppState, &str, &str); 5] = [
        ("unknown user", &state, "nobody-here", WRONG),
        (
            "disabled user, correct password",
            &state,
            "carol",
            "carol-correct-password",
        ),
        (
            "locked user, correct password",
            &state,
            "dave",
            "dave-correct-password",
        ),
        ("known user, wrong password", &state, USERNAME, WRONG),
        (
            "known user, wrong password, store failure",
            &broken,
            USERNAME,
            WRONG,
        ),
    ];

    let mut reference: Option<(StatusCode, Vec<u8>)> = None;
    for (name, app, user, pw) in branches {
        let before = wrong_password_count(app);
        let (status, body, has_session) = post_login(app, user, pw).await;
        let after = wrong_password_count(app);
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{name}: status");
        assert!(!has_session, "{name}: no session cookie");
        assert_eq!(
            after - before,
            1,
            "{name}: exactly one wrong_password increment"
        );
        match &reference {
            None => reference = Some((status, body)),
            Some((ref_status, ref_body)) => {
                assert_eq!(status, *ref_status, "{name}: status differs");
                assert!(
                    body == *ref_body,
                    "{name}: body differs from the unknown-user body"
                );
            }
        }
    }
}
