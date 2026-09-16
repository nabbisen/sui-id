//! RFC 102 stage 2 — L01: a password sign-in commits its session, its
//! `auth.login.success` event and its bookkeeping together or not at all
//! (A1, A2), is refused before any write when nobody will hold the session
//! (A7), and never counts a correct password as a failure (A9).

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::io::Write;
use std::sync::{Arc, Mutex};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::models::{CredentialRow, Role, SessionRow, UserRow, UserSource};
use sui_id_store::repos::{credentials, sessions, users};
use tower::ServiceExt;

const BOB: &str = "bob";
const BOB_PASSWORD: &str = "bob-correct-password";
const WRONG: &str = "definitely-not-the-password";

struct Login {
    status: StatusCode,
    location: Option<String>,
    session: Option<String>,
    body: Vec<u8>,
}

async fn post_login(state: &AppState, username: &str, password: &str, next: &str) -> Login {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={}&password={}&next={}",
                    urlencode(username),
                    urlencode(password),
                    urlencode(next)
                )))
                .expect("req"),
        )
        .await
        .expect("login");
    let status = resp.status();
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let session = extract_set_cookie(resp.headers(), "sui_id_session");
    let body = read_body(resp.into_body()).await.to_vec();
    Login {
        status,
        location,
        session,
        body,
    }
}

async fn seed_user(state: &AppState, username: &str, password: &str, role: Role) -> UserId {
    let now = chrono::Utc::now();
    let uid = UserId::new();
    users::create(
        &state.db,
        &UserRow {
            id: uid,
            username: username.into(),
            display_name: None,
            is_admin: role == Role::Admin,
            role,
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

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

async fn text(state: &AppState, sql: String) -> Option<String> {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("text")
}

async fn session_rows(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM sessions WHERE user_id = '{user}'"),
    )
    .await
}

async fn active_sessions(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM sessions WHERE user_id = '{user}' AND revoked_at IS NULL"),
    )
    .await
}

async fn success_events(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!(
            "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.login.success' \
             AND target = '{user}'"
        ),
    )
    .await
}

async fn set_cap(state: &AppState, cap: i64) {
    exec(
        state,
        format!("UPDATE server_settings SET max_concurrent_sessions = {cap}"),
    )
    .await;
}

/// An earlier live session for `user`, inserted directly.
async fn old_session(state: &AppState, user: UserId) -> SessionId {
    let at = chrono::Utc::now() - chrono::Duration::minutes(30);
    let row = SessionRow {
        id: SessionId::new(),
        user_id: user,
        expires_at: at + chrono::Duration::hours(12),
        created_at: at,
        revoked_at: None,
        auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
        last_step_up_at: None,
        last_used_at: None,
    };
    sessions::insert(&state.db, &row)
        .await
        .expect("old session");
    row.id
}

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

// ── Happy path ───────────────────────────────────────────────────────

#[tokio::test]
async fn r102_l01_sign_in_commits_session_event_and_bookkeeping_together() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let bob = seed_user(&state, BOB, BOB_PASSWORD, Role::User).await;
    // Two earlier failures and a lock that has already expired.
    exec(
        &state,
        format!(
            "UPDATE users SET failed_login_count = 2, \
             locked_until = '2000-01-01T00:00:00Z' WHERE id = '{bob}'"
        ),
    )
    .await;

    let r = post_login(&state, BOB, BOB_PASSWORD, "/me/security").await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert_eq!(r.location.as_deref(), Some("/me/security"));
    let cookie = r.session.expect("session cookie");

    assert_eq!(session_rows(&state, bob).await, 1, "one session");
    let session_user = text(
        &state,
        format!("SELECT user_id FROM sessions WHERE id = '{cookie}'"),
    )
    .await;
    assert_eq!(session_user.as_deref(), Some(bob.to_string().as_str()));
    assert_eq!(success_events(&state, bob).await, 1, "one event");
    let actor = text(
        &state,
        format!(
            "SELECT actor FROM audit_log WHERE action = 'auth.login.success' \
             AND target = '{bob}'"
        ),
    )
    .await;
    assert_eq!(actor.as_deref(), Some(bob.to_string().as_str()));
    let note = text(
        &state,
        format!(
            "SELECT note FROM audit_log WHERE action = 'auth.login.success' \
             AND target = '{bob}'"
        ),
    )
    .await;
    assert_eq!(note.as_deref(), Some("evicted=0"));
    assert_eq!(
        scalar(
            &state,
            format!(
                "SELECT failed_login_count + (locked_until IS NOT NULL) FROM users \
                 WHERE id = '{bob}'"
            )
        )
        .await,
        0,
        "counter and stale lock cleared"
    );
    assert_eq!(
        scalar(
            &state,
            format!("SELECT last_login_at IS NOT NULL FROM users WHERE id = '{bob}'")
        )
        .await,
        1,
        "last_login_at set"
    );
}

// ── Injected append failure ──────────────────────────────────────────

#[tokio::test]
async fn r102_l01_append_failure_commits_nothing_and_looks_like_any_failure() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let bob = seed_user(&state, BOB, BOB_PASSWORD, Role::User).await;
    let stale_lock = "2000-01-01T00:00:00Z";
    exec(
        &state,
        format!(
            "UPDATE users SET failed_login_count = 2, locked_until = '{stale_lock}' \
             WHERE id = '{bob}'"
        ),
    )
    .await;
    set_cap(&state, 1).await;
    let earlier = old_session(&state, bob).await;
    let events_before = scalar(&state, "SELECT COUNT(*) FROM audit_log".into()).await;
    let locked_before = text(
        &state,
        format!("SELECT locked_until FROM users WHERE id = '{bob}'"),
    )
    .await;

    // The R11 harness: every insert into audit_log fails.
    exec(
        &state,
        "CREATE TRIGGER r102_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'r102 test: audit_log insert rejected'); END;"
            .into(),
    )
    .await;

    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();

    let unknown = post_login(&state, "nobody-here", WRONG, "/me/security").await;
    tracing::callsite::rebuild_interest_cache();
    let correct = post_login(&state, BOB, BOB_PASSWORD, "/me/security").await;

    assert_eq!(correct.status, StatusCode::UNAUTHORIZED);
    assert!(correct.session.is_none(), "no session cookie");
    assert_eq!(
        correct.body, unknown.body,
        "byte-identical to an ordinary failed sign-in"
    );
    assert_eq!(unknown.status, correct.status);

    assert_eq!(session_rows(&state, bob).await, 1, "no new session row");
    assert_eq!(
        text(
            &state,
            format!("SELECT revoked_at FROM sessions WHERE id = '{earlier}'")
        )
        .await,
        None,
        "no eviction"
    );
    assert_eq!(
        scalar(&state, "SELECT COUNT(*) FROM audit_log".into()).await,
        events_before,
        "no event"
    );
    assert_eq!(
        scalar(
            &state,
            format!("SELECT failed_login_count FROM users WHERE id = '{bob}'")
        )
        .await,
        2,
        "A9: the correct password is not counted, and the counter is not cleared"
    );
    assert_eq!(
        text(
            &state,
            format!("SELECT locked_until FROM users WHERE id = '{bob}'")
        )
        .await,
        locked_before,
        "the stale lock is not cleared"
    );
    assert_eq!(
        scalar(
            &state,
            format!("SELECT last_login_at IS NULL FROM users WHERE id = '{bob}'")
        )
        .await,
        1,
        "last_login_at not set"
    );

    let logged = String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned();
    let line = logged
        .lines()
        .find(|l| l.contains("other than invalid credentials"))
        .unwrap_or_else(|| panic!("no log line for the failed L01:\n{logged}"));
    assert!(line.contains("ERROR"), "wrong level: {line}");
    assert!(line.contains("request_id="), "no request id: {line}");
    assert!(
        line.contains("r102 test: audit_log insert rejected"),
        "cause missing: {line}"
    );
    assert!(!logged.contains(BOB_PASSWORD), "the password leaked");
}

// ── Cap ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn r102_l01_sign_in_over_the_cap_leaves_exactly_the_cap() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let bob = seed_user(&state, BOB, BOB_PASSWORD, Role::User).await;
    let cap = 2;
    set_cap(&state, cap).await;

    for _ in 0..cap {
        let r = post_login(&state, BOB, BOB_PASSWORD, "/me/security").await;
        assert!(r.session.is_some());
    }
    assert_eq!(active_sessions(&state, bob).await, cap);

    let r = post_login(&state, BOB, BOB_PASSWORD, "/me/security").await;
    let newest = r.session.expect("the (N+1)th sign-in succeeds");
    assert_eq!(active_sessions(&state, bob).await, cap, "exactly N remain");
    assert_eq!(
        scalar(
            &state,
            format!("SELECT revoked_at IS NULL FROM sessions WHERE id = '{newest}'")
        )
        .await,
        1,
        "the new session is kept"
    );
    assert_eq!(success_events(&state, bob).await, cap + 1);
    assert_eq!(
        scalar(
            &state,
            format!(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.login.success' \
                 AND target = '{bob}' AND note = 'evicted=1'"
            )
        )
        .await,
        1,
        "the eviction is committed with its event"
    );
}

// ── A7 ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn r102_a7_non_admin_to_an_admin_destination_writes_nothing() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let bob = seed_user(&state, BOB, BOB_PASSWORD, Role::User).await;

    for next in ["", "/admin", "/admin/users"] {
        let r = post_login(&state, BOB, BOB_PASSWORD, next).await;
        assert_eq!(r.status, StatusCode::OK, "next={next:?}: refusal page");
        assert!(r.session.is_none(), "next={next:?}: no cookie");
        let body = String::from_utf8_lossy(&r.body);
        assert!(
            body.contains(
                sui_id_i18n::Locale::default()
                    .strings()
                    .login_no_admin_access
            ),
            "next={next:?}: the existing refusal message"
        );
    }
    assert_eq!(session_rows(&state, bob).await, 0, "no session row");
    assert_eq!(success_events(&state, bob).await, 0, "no success event");

    // The same user to a non-admin destination signs in.
    let r = post_login(&state, BOB, BOB_PASSWORD, "/me/security").await;
    assert!(r.session.is_some());
}

// ── The in-transaction re-read ───────────────────────────────────────

async fn l01_for(state: &AppState, user: UserId) -> Result<(), sui_id_store::StoreError> {
    let now = chrono::Utc::now();
    sui_id_store::commands::sign_in_with_password(
        &state.db,
        SessionRow {
            id: SessionId::new(),
            user_id: user,
            expires_at: now + chrono::Duration::hours(12),
            created_at: now,
            revoked_at: None,
            auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
            last_step_up_at: None,
            last_used_at: None,
        },
    )
    .await
    .map(|_| ())
}

#[tokio::test]
async fn r102_l01_rereads_the_user_and_rolls_back_when_inactive() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let bob = seed_user(&state, BOB, BOB_PASSWORD, Role::User).await;

    for (label, sql) in [
        ("disabled", "is_disabled = 1"),
        ("deleted", "is_deleted = 1"),
        ("locked", "locked_until = '2999-01-01T00:00:00Z'"),
    ] {
        exec(
            &state,
            format!(
                "UPDATE users SET is_disabled = 0, is_deleted = 0, locked_until = NULL, \
                 failed_login_count = 3 WHERE id = '{bob}'; \
                 UPDATE users SET {sql} WHERE id = '{bob}'"
            ),
        )
        .await;
        let result = l01_for(&state, bob).await;
        assert!(
            matches!(result, Err(sui_id_store::StoreError::NotFound)),
            "{label}: rolled back"
        );
        assert_eq!(session_rows(&state, bob).await, 0, "{label}: no session");
        assert_eq!(success_events(&state, bob).await, 0, "{label}: no event");
        assert_eq!(
            scalar(
                &state,
                format!("SELECT failed_login_count FROM users WHERE id = '{bob}'")
            )
            .await,
            3,
            "{label}: counter untouched"
        );
    }

    // Through the handler: a user disabled after the password check would
    // be refused the same way; here the ordinary pre-check refuses first,
    // with the uniform 401.
    let r = post_login(&state, BOB, BOB_PASSWORD, "/me/security").await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}
