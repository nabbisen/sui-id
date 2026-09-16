//! A returning directory user can sign in (roadmap `ldap-returning-signin`).
//!
//! A local row with `source = ldap` authenticates against the user sources
//! by its stable id, never against a local credential; a wrong password is
//! counted on the shadow row; only `NotFound` means "unknown locally"; and
//! RFC 102 B7's re-bind uses the same stable id, so a suffixed shadow row
//! still re-binds.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::UserId;
use sui_id_store::user_source::{
    ExternalUserRecord, InMemoryUserSource, UserSource, UserSourceError,
};
use tower::ServiceExt;

const BOB_PW: &str = "bob-directory-password";
const BOB_ID: &str = "uuid-bob";

/// An in-memory directory that counts how often it is asked.
struct Counting {
    inner: InMemoryUserSource,
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl UserSource for Counting {
    async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.authenticate(username, password).await
    }
    async fn authenticate_stable_id(
        &self,
        stable_id: &str,
        password: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.authenticate_stable_id(stable_id, password).await
    }
    fn slug(&self) -> &str {
        "corp"
    }
}

/// A source that accepts any password and reports another identity.
struct OtherIdentity;

#[async_trait::async_trait]
impl UserSource for OtherIdentity {
    async fn authenticate(
        &self,
        _u: &str,
        _p: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
        Ok(None)
    }
    async fn authenticate_stable_id(
        &self,
        _s: &str,
        _p: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
        Ok(Some(ExternalUserRecord {
            stable_id: "uuid-someone-else".into(),
            display_username: "someone".into(),
            email: None,
            display_name: None,
            source_slug: "other".into(),
        }))
    }
    fn slug(&self) -> &str {
        "other"
    }
}

/// A directory that cannot be reached.
struct Unreachable;

#[async_trait::async_trait]
impl UserSource for Unreachable {
    async fn authenticate(
        &self,
        _u: &str,
        _p: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
        Err(UserSourceError::Transport("connection refused".into()))
    }
    async fn authenticate_stable_id(
        &self,
        _s: &str,
        _p: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
        Err(UserSourceError::Transport("connection refused".into()))
    }
    fn slug(&self) -> &str {
        "down"
    }
}

fn directory(entries: &[(&str, &str)]) -> InMemoryUserSource {
    let mut users = std::collections::HashMap::new();
    for (name, id) in entries {
        users.insert(
            (*name).to_owned(),
            (BOB_PW.to_owned(), (*id).to_owned(), None, None),
        );
    }
    InMemoryUserSource {
        slug: "corp".into(),
        users,
    }
}

/// An app whose only user source is a counting in-memory directory.
async fn app_with(entries: &[(&str, &str)]) -> (AppState, Arc<AtomicUsize>) {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let calls = Arc::new(AtomicUsize::new(0));
    state.user_sources = vec![Arc::new(Counting {
        inner: directory(entries),
        calls: calls.clone(),
    })];
    (state, calls)
}

struct Login {
    status: StatusCode,
    location: Option<String>,
    session: Option<String>,
    pending: Option<String>,
}

async fn login(state: &AppState, username: &str, password: &str) -> Login {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={}&password={}&next=/me/security",
                    urlencode(username),
                    urlencode(password)
                )))
                .expect("req"),
        )
        .await
        .expect("login");
    Login {
        status: resp.status(),
        location: resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        session: extract_set_cookie(resp.headers(), "sui_id_session").filter(|v| !v.is_empty()),
        pending: extract_set_cookie(resp.headers(), "sui_id_pending_mfa").filter(|v| !v.is_empty()),
    }
}

async fn user(state: &AppState, username: &str) -> sui_id_store::models::UserRow {
    sui_id_store::repos::users::find_by_username(&state.db, username)
        .await
        .expect("user row")
}

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn sessions_of(state: &AppState, user: UserId) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM sessions WHERE user_id = '{user}'");
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("count")
}

#[tokio::test]
async fn ldap_first_and_second_sign_in_both_succeed() {
    let (state, calls) = app_with(&[("bob", BOB_ID)]).await;

    let first = login(&state, "bob", BOB_PW).await;
    assert_eq!(first.status, StatusCode::SEE_OTHER);
    assert!(first.session.is_some(), "first sign-in");
    let bob = user(&state, "bob").await;
    assert_eq!(bob.source, sui_id_store::models::UserSource::Ldap);

    let second = login(&state, "bob", BOB_PW).await;
    assert_eq!(second.status, StatusCode::SEE_OTHER, "second sign-in");
    assert_eq!(second.location.as_deref(), Some("/me/security"));
    assert!(second.session.is_some(), "second sign-in has a cookie");
    assert_eq!(sessions_of(&state, bob.id).await, 2);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the directory was asked each time"
    );
}

#[tokio::test]
async fn ldap_wrong_password_on_the_second_sign_in_is_counted() {
    let (state, _calls) = app_with(&[("bob", BOB_ID)]).await;
    assert!(login(&state, "bob", BOB_PW).await.session.is_some());

    let r = login(&state, "bob", "not-the-directory-password").await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.session.is_none());
    assert_eq!(
        user(&state, "bob").await.failed_login_count,
        1,
        "counted (U22)"
    );

    // A correct password afterwards signs in and ends the run.
    assert!(login(&state, "bob", BOB_PW).await.session.is_some());
    assert_eq!(user(&state, "bob").await.failed_login_count, 0);
}

#[tokio::test]
async fn ldap_user_removed_from_the_directory_is_refused() {
    let (mut state, _calls) = app_with(&[("bob", BOB_ID)]).await;
    assert!(login(&state, "bob", BOB_PW).await.session.is_some());
    state.user_sources = vec![Arc::new(directory(&[]))];

    let r = login(&state, "bob", BOB_PW).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.session.is_none());
}

#[tokio::test]
async fn ldap_disabled_shadow_user_is_refused_without_asking_the_directory() {
    let (state, calls) = app_with(&[("bob", BOB_ID)]).await;
    assert!(login(&state, "bob", BOB_PW).await.session.is_some());
    let bob = user(&state, "bob").await;
    exec(
        &state,
        format!("UPDATE users SET is_disabled = 1 WHERE id = '{}'", bob.id),
    )
    .await;
    calls.store(0, Ordering::SeqCst);

    let r = login(&state, "bob", BOB_PW).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.session.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0, "directory not consulted");
    assert_eq!(
        user(&state, "bob").await.failed_login_count,
        0,
        "not counted"
    );
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

#[tokio::test]
async fn ldap_lookup_error_is_refused_logged_and_never_reaches_the_directory() {
    let (state, calls) = app_with(&[("bob", BOB_ID)]).await;
    // The username lookup fails with a storage error, not NotFound.
    exec(&state, "ALTER TABLE users RENAME TO users_gone".into()).await;

    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();

    let r = login(&state, "bob", BOB_PW).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.session.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0, "directory not consulted");
    let logged = String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned();
    let line = logged
        .lines()
        .find(|l| l.contains("other than invalid credentials"))
        .unwrap_or_else(|| panic!("no log line for the lookup error:\n{logged}"));
    assert!(line.contains("ERROR"), "wrong level: {line}");
    assert!(!logged.contains(BOB_PW), "the password leaked");
}

#[tokio::test]
async fn ldap_suffixed_shadow_user_signs_in_and_rebinds_by_stable_id() {
    let (state, _calls) = app_with(&[("bob", BOB_ID)]).await;
    // A shadow row suffixed on collision: local "bob2", directory "bob".
    sui_id_store::repos::users::upsert_ldap_shadow(
        &state.db,
        sui_id_store::repos::users::LdapShadowData {
            username: "bob2".into(),
            display_name: None,
            email: None,
            external_stable_id: BOB_ID.into(),
        },
        chrono::Utc::now(),
    )
    .await
    .expect("shadow");

    let signed_in = login(&state, "bob2", BOB_PW).await;
    let session = signed_in.session.expect("the suffixed shadow signs in");
    let bob2 = user(&state, "bob2").await;

    // RFC 102 B7: adding a first factor re-binds with the directory
    // password, by stable id; it succeeds and counts nothing.
    let page = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/mfa")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("mfa page");
    let csrf = extract_set_cookie(page.headers(), "sui_id_csrf").expect("csrf");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/mfa/enroll/start")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(format!(
                    "_csrf={csrf}&current_password={}",
                    urlencode(BOB_PW)
                )))
                .expect("req"),
        )
        .await
        .expect("enroll start");
    assert_eq!(resp.status(), StatusCode::OK, "re-bind accepted");
    let failures: i64 = {
        let sql = format!("SELECT step_up_failure_count FROM sessions WHERE id = '{session}'");
        state
            .db
            .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
            .await
            .expect("count")
    };
    assert_eq!(failures, 0, "the re-bind was not counted as a failure");
    assert_eq!(bob2.username, "bob2", "the shadow keeps its name");
}

#[tokio::test]
async fn ldap_returning_user_with_a_factor_gets_the_mfa_step() {
    let (state, _calls) = app_with(&[("bob", BOB_ID)]).await;
    assert!(login(&state, "bob", BOB_PW).await.session.is_some());
    let bob = user(&state, "bob").await;
    exec(
        &state,
        format!(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at) \
             VALUES ('{}', '{}', X'0102', X'00', 'k', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4(),
            bob.id
        ),
    )
    .await;
    let before = sessions_of(&state, bob.id).await;

    let r = login(&state, "bob", BOB_PW).await;
    assert_eq!(r.location.as_deref(), Some("/admin/login/mfa"));
    assert!(r.pending.is_some(), "a pending-MFA row");
    assert!(r.session.is_none(), "no session before the second factor");
    assert_eq!(sessions_of(&state, bob.id).await, before);
}

#[tokio::test]
async fn ldap_record_with_another_stable_id_is_refused_and_not_counted() {
    let (mut state, _calls) = app_with(&[("bob", BOB_ID)]).await;
    assert!(login(&state, "bob", BOB_PW).await.session.is_some());
    state.user_sources = vec![Arc::new(OtherIdentity)];

    let r = login(&state, "bob", BOB_PW).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.session.is_none());
    assert_eq!(
        user(&state, "bob").await.failed_login_count,
        0,
        "not counted"
    );
}

#[tokio::test]
async fn ldap_unreachable_directory_refuses_without_counting() {
    let (mut state, _calls) = app_with(&[("bob", BOB_ID)]).await;
    assert!(login(&state, "bob", BOB_PW).await.session.is_some());
    state.user_sources = vec![Arc::new(Unreachable)];

    let r = login(&state, "bob", BOB_PW).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        user(&state, "bob").await.failed_login_count,
        0,
        "not counted"
    );
}

#[tokio::test]
async fn ldap_known_local_user_with_a_wrong_password_never_reaches_the_directory() {
    // P4 and L4: a user found locally is decided locally. The directory is
    // asked only for a name the lookup reported as NotFound.
    let (state, calls) = app_with(&[(USERNAME, BOB_ID)]).await;
    let r = login(&state, USERNAME, BOB_PW).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.session.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0, "directory not consulted");
}
