//! RFC 102 stage 4 — L03 and L04: the directory and federated sign-ins
//! commit their session, their bookkeeping and their success event
//! together or not at all (A1, A2); A7 refuses a directory sign-in bound
//! for the admin panel before any write; a failed commit is never counted
//! and gets the uniform response (A9).

use super::common::*;
use super::federation_fail_closed::{federated_signin, linked_user, mock_upstream};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::io::Write;
use std::sync::{Arc, Mutex};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::user_source::InMemoryUserSource;
use tower::ServiceExt;

const BOB_PW: &str = "bob-directory-password";
const BOB_ID: &str = "uuid-bob";

fn directory(display_name: Option<&str>) -> Arc<InMemoryUserSource> {
    let mut users = std::collections::HashMap::new();
    users.insert(
        "bob".to_owned(),
        (
            BOB_PW.to_owned(),
            BOB_ID.to_owned(),
            Some("bob@test.invalid".to_owned()),
            display_name.map(str::to_owned),
        ),
    );
    Arc::new(InMemoryUserSource {
        slug: "corp".into(),
        users,
    })
}

async fn directory_app() -> AppState {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.user_sources = vec![directory(Some("Bob"))];
    state
}

struct Login {
    status: StatusCode,
    session: Option<String>,
    body: Vec<u8>,
}

async fn login(state: &AppState, username: &str, password: &str, next: &str) -> Login {
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
    Login {
        status: resp.status(),
        session: extract_set_cookie(resp.headers(), "sui_id_session").filter(|v| !v.is_empty()),
        body: read_body(resp.into_body()).await.to_vec(),
    }
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

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn bob(state: &AppState) -> Option<sui_id_store::models::UserRow> {
    sui_id_store::repos::users::find_by_username(&state.db, "bob")
        .await
        .ok()
}

async fn active_sessions(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM sessions WHERE user_id = '{user}' AND revoked_at IS NULL"),
    )
    .await
}

async fn all_sessions(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM sessions WHERE user_id = '{user}'"),
    )
    .await
}

async fn events(state: &AppState, action: &str) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM audit_log WHERE action = '{action}'"),
    )
    .await
}

async fn success_events_for(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM audit_log WHERE action = 'auth.login.success' AND target = '{user}'"),
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

async fn break_audit_log(state: &AppState) {
    exec(
        state,
        "CREATE TRIGGER r102_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'r102 test: audit_log insert rejected'); END;"
            .into(),
    )
    .await;
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

fn capture() -> (Captured, tracing::subscriber::DefaultGuard) {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    (captured, guard)
}

fn logged(captured: &Captured) -> String {
    String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned()
}

// ── L03 ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn r102_l03_first_and_returning_sign_in_commit_session_event_and_bookkeeping() {
    let state = directory_app().await;

    // First sign-in: the shadow row is created by L03.
    let r = login(&state, "bob", BOB_PW, "/me/security").await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    let session = r.session.expect("session cookie");
    let row = bob(&state).await.expect("shadow row created");
    assert_eq!(row.display_name.as_deref(), Some("Bob"));
    assert!(row.last_login_at.is_some(), "last_login_at set");
    assert_eq!(all_sessions(&state, row.id).await, 1, "one session");
    assert_eq!(
        text(
            &state,
            format!("SELECT user_id FROM sessions WHERE id = '{session}'")
        )
        .await
        .as_deref(),
        Some(row.id.to_string().as_str())
    );
    assert_eq!(
        scalar(
            &state,
            format!(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.login.success' \
                 AND actor = '{0}' AND target = '{0}' AND note = 'evicted=0 source=corp'",
                row.id
            )
        )
        .await,
        1,
        "one Atomic event with its source"
    );
    assert_eq!(
        events(&state, "auth.user_source.matched").await,
        0,
        "the best-effort match row is retired"
    );

    // Returning sign-in: counter and stale lock reset, display name
    // refreshed, a second event.
    exec(
        &state,
        format!(
            "UPDATE users SET failed_login_count = 2, locked_until = '2000-01-01T00:00:00Z', \
             display_name = 'Old' WHERE id = '{}'",
            row.id
        ),
    )
    .await;
    let r = login(&state, "bob", BOB_PW, "/me/security").await;
    assert!(r.session.is_some(), "returning sign-in");
    let row = bob(&state).await.expect("row");
    assert_eq!(row.failed_login_count, 0);
    assert!(row.locked_until.is_none());
    assert_eq!(
        row.display_name.as_deref(),
        Some("Bob"),
        "display name refreshed"
    );
    assert_eq!(success_events_for(&state, row.id).await, 2);
}

#[tokio::test]
async fn r102_l03_append_failure_commits_nothing_and_looks_like_any_failure() {
    // First sign-in: no shadow row may appear.
    let state = directory_app().await;
    let sessions_before = scalar(&state, "SELECT COUNT(*) FROM sessions".into()).await;
    break_audit_log(&state).await;
    let (captured, _guard) = capture();
    let unknown = login(&state, "nobody-here", "wrong-password-x", "/me/security").await;
    // The refused sign-in writes nothing, so it can be repeated until the
    // capturing subscriber has seen its line: tests running in parallel
    // without a subscriber can race tracing's callsite interest cache.
    let mut r = login(&state, "bob", BOB_PW, "/me/security").await;
    for _ in 0..4 {
        if logged(&captured).contains("other than invalid credentials") {
            break;
        }
        tracing::callsite::rebuild_interest_cache();
        r = login(&state, "bob", BOB_PW, "/me/security").await;
    }
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.session.is_none());
    assert_eq!(r.body, unknown.body, "the uniform 401 body");
    assert!(bob(&state).await.is_none(), "no shadow row created");
    assert_eq!(
        scalar(&state, "SELECT COUNT(*) FROM sessions".into()).await,
        sessions_before,
        "no session"
    );
    let log = logged(&captured);
    let line = log
        .lines()
        .find(|l| l.contains("other than invalid credentials"))
        .unwrap_or_else(|| panic!("no log line:\n{log}"));
    assert!(line.contains("ERROR") && line.contains("r102 test: audit_log insert rejected"));
    assert!(!log.contains(BOB_PW), "the password leaked");
}

#[tokio::test]
async fn r102_l03_append_failure_on_a_returning_user_changes_no_shadow_field() {
    let mut state = directory_app().await;
    assert!(
        login(&state, "bob", BOB_PW, "/me/security")
            .await
            .session
            .is_some()
    );
    let before = bob(&state).await.expect("row");
    exec(
        &state,
        format!(
            "UPDATE users SET failed_login_count = 2, locked_until = '2000-01-01T00:00:00Z' \
             WHERE id = '{}'",
            before.id
        ),
    )
    .await;
    // The directory now reports another display name.
    state.user_sources = vec![directory(Some("Robert"))];
    let events_before = scalar(&state, "SELECT COUNT(*) FROM audit_log".into()).await;
    break_audit_log(&state).await;

    let r = login(&state, "bob", BOB_PW, "/me/security").await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.session.is_none());
    let after = bob(&state).await.expect("row");
    assert_eq!(
        after.display_name.as_deref(),
        Some("Bob"),
        "upsert rolled back"
    );
    assert_eq!(after.failed_login_count, 2, "not counted, not reset");
    assert!(after.locked_until.is_some(), "stale lock not cleared");
    assert_eq!(
        after.last_login_at, before.last_login_at,
        "last_login_at unchanged"
    );
    assert_eq!(all_sessions(&state, before.id).await, 1, "no new session");
    assert_eq!(
        scalar(&state, "SELECT COUNT(*) FROM audit_log".into()).await,
        events_before,
        "no event"
    );
}

#[tokio::test]
async fn r102_l03_directory_sign_in_evicts_over_the_cap() {
    let state = directory_app().await;
    set_cap(&state, 1).await;
    assert!(
        login(&state, "bob", BOB_PW, "/me/security")
            .await
            .session
            .is_some()
    );
    let row = bob(&state).await.expect("row");
    let r = login(&state, "bob", BOB_PW, "/me/security").await;
    let newest = r.session.expect("second sign-in");
    assert_eq!(active_sessions(&state, row.id).await, 1, "exactly the cap");
    assert_eq!(
        scalar(
            &state,
            format!("SELECT revoked_at IS NULL FROM sessions WHERE id = '{newest}'")
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.login.success' \
             AND note = 'evicted=1 source=corp'"
                .into()
        )
        .await,
        1
    );
}

#[tokio::test]
async fn r102_a7_non_admin_through_the_directory_to_admin_writes_nothing() {
    let state = directory_app().await;
    let successes_before = events(&state, "auth.login.success").await;
    // First sign-in bound for the admin panel: not even a shadow row.
    let r = login(&state, "bob", BOB_PW, "/admin").await;
    assert_eq!(r.status, StatusCode::OK, "the refusal page");
    assert!(r.session.is_none());
    assert!(
        String::from_utf8_lossy(&r.body).contains(
            sui_id_i18n::Locale::default()
                .strings()
                .login_no_admin_access
        )
    );
    assert!(bob(&state).await.is_none(), "no shadow row");
    assert_eq!(events(&state, "auth.login.success").await, successes_before);

    // A returning shadow user: no session row either.
    assert!(
        login(&state, "bob", BOB_PW, "/me/security")
            .await
            .session
            .is_some()
    );
    let row = bob(&state).await.expect("row");
    let r = login(&state, "bob", BOB_PW, "").await;
    assert!(r.session.is_none());
    assert_eq!(all_sessions(&state, row.id).await, 1, "no session row");
    assert_eq!(success_events_for(&state, row.id).await, 1);
}

#[tokio::test]
async fn r102_l03_rereads_the_user_and_rolls_back_when_inactive() {
    let state = directory_app().await;
    assert!(
        login(&state, "bob", BOB_PW, "/me/security")
            .await
            .session
            .is_some()
    );
    let row = bob(&state).await.expect("row");
    let shadow = || sui_id_store::repos::users::LdapShadowData {
        username: "bob".into(),
        display_name: Some("Changed".into()),
        email: None,
        external_stable_id: BOB_ID.into(),
    };
    for (label, sql) in [
        ("disabled", "is_disabled = 1"),
        ("deleted", "is_deleted = 1"),
        ("locked", "locked_until = '2999-01-01T00:00:00Z'"),
    ] {
        exec(
            &state,
            format!(
                "UPDATE users SET is_disabled = 0, is_deleted = 0, locked_until = NULL \
                 WHERE id = '{0}'; UPDATE users SET {sql} WHERE id = '{0}'",
                row.id
            ),
        )
        .await;
        let now = chrono::Utc::now();
        let result = sui_id_store::commands::sign_in_from_directory(
            &state.db,
            shadow(),
            "corp".into(),
            sui_id_store::models::SessionRow {
                id: SessionId::new(),
                user_id: row.id,
                expires_at: now + chrono::Duration::hours(24),
                created_at: now,
                revoked_at: None,
                auth_methods: vec![sui_id_shared::AuthMethod::Fed],
                last_step_up_at: None,
                last_used_at: None,
            },
        )
        .await;
        assert!(
            matches!(result, Err(sui_id_store::StoreError::NotFound)),
            "{label}: rolled back"
        );
        assert_eq!(all_sessions(&state, row.id).await, 1, "{label}: no session");
        assert_eq!(
            bob(&state).await.expect("row").display_name.as_deref(),
            Some("Bob"),
            "{label}: upsert rolled back"
        );
    }
}

// ── L04 ──────────────────────────────────────────────────────────────

async fn federation_app() -> (AppState, UserId) {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let issuer = mock_upstream().await;
    let user = linked_user(&state, &issuer).await;
    (state, user)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn r102_l04_federated_sign_in_commits_session_event_and_last_login() {
    let (state, user) = federation_app().await;
    let o = federated_signin(&state).await;
    assert_eq!(o.location, "/admin");
    let session = o.session_cookie.expect("session");
    assert_eq!(all_sessions(&state, user).await, 1);
    assert_eq!(
        text(
            &state,
            format!("SELECT user_id FROM sessions WHERE id = '{session}'")
        )
        .await
        .as_deref(),
        Some(user.to_string().as_str())
    );
    assert_eq!(
        scalar(
            &state,
            format!(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.federation.signin.success' \
                 AND actor = '{user}' AND target = '{user}' AND note = 'provider=up evicted=0'"
            )
        )
        .await,
        1,
        "one Atomic event with the provider"
    );
    assert_eq!(
        scalar(
            &state,
            format!("SELECT last_login_at IS NOT NULL FROM users WHERE id = '{user}'")
        )
        .await,
        1
    );
}

// Current-thread runtime: the handler runs on this thread, so the
// thread-local capturing subscriber sees its log line.
#[tokio::test]
async fn r102_l04_append_failure_commits_nothing_and_redirects_uniformly() {
    let (state, user) = federation_app().await;
    break_audit_log(&state).await;
    let (captured, _guard) = capture();
    // Repeatable for the same reason as the L03 case: nothing is written.
    let mut o = federated_signin(&state).await;
    for _ in 0..4 {
        if logged(&captured).contains("federation: sign-in transaction failed") {
            break;
        }
        tracing::callsite::rebuild_interest_cache();
        o = federated_signin(&state).await;
    }
    assert!(o.status.is_redirection());
    assert_eq!(o.location, "/admin/login?fed_error=signin_failed");
    assert!(o.session_cookie.is_none());
    assert_eq!(all_sessions(&state, user).await, 0, "no session");
    assert_eq!(events(&state, "auth.federation.signin.success").await, 0);
    assert_eq!(
        scalar(
            &state,
            format!("SELECT last_login_at IS NULL FROM users WHERE id = '{user}'")
        )
        .await,
        1,
        "last_login_at unchanged"
    );
    let log = logged(&captured);
    let line = log
        .lines()
        .find(|l| l.contains("federation: sign-in transaction failed"))
        .unwrap_or_else(|| panic!("no log line:\n{log}"));
    assert!(line.contains("ERROR"), "wrong level: {line}");
    assert!(
        line.contains("r102 test: audit_log insert rejected"),
        "cause missing: {line}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn r102_l04_federated_sign_in_evicts_over_the_cap() {
    let (state, user) = federation_app().await;
    set_cap(&state, 1).await;
    assert!(federated_signin(&state).await.session_cookie.is_some());
    let newest = federated_signin(&state)
        .await
        .session_cookie
        .expect("second");
    assert_eq!(active_sessions(&state, user).await, 1, "exactly the cap");
    assert_eq!(
        scalar(
            &state,
            format!("SELECT revoked_at IS NULL FROM sessions WHERE id = '{newest}'")
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.federation.signin.success' \
             AND note = 'provider=up evicted=1'"
                .into()
        )
        .await,
        1
    );
}

#[tokio::test]
async fn r102_l04_rereads_the_user_and_rolls_back_when_inactive() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    for (label, sql) in [
        ("disabled", "is_disabled = 1"),
        ("deleted", "is_deleted = 1"),
    ] {
        exec(
            &state,
            format!(
                "UPDATE users SET is_disabled = 0, is_deleted = 0 WHERE id = '{user}'; \
                 UPDATE users SET {sql} WHERE id = '{user}'"
            ),
        )
        .await;
        let before = all_sessions(&state, user).await;
        let now = chrono::Utc::now();
        let result = sui_id_store::commands::sign_in_federated(
            &state.db,
            "up".into(),
            sui_id_store::models::SessionRow {
                id: SessionId::new(),
                user_id: user,
                expires_at: now + chrono::Duration::hours(24),
                created_at: now,
                revoked_at: None,
                auth_methods: vec![sui_id_shared::AuthMethod::Fed],
                last_step_up_at: None,
                last_used_at: None,
            },
        )
        .await;
        assert!(
            matches!(result, Err(sui_id_store::StoreError::NotFound)),
            "{label}: rolled back"
        );
        assert_eq!(
            all_sessions(&state, user).await,
            before,
            "{label}: no session"
        );
    }
    assert_eq!(events(&state, "auth.federation.signin.success").await, 0);
}
