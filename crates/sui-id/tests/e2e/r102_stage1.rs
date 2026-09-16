//! RFC 102 stage 1 — factor additions need proof (B7), step-up failures
//! are counted and throttled (L06, the step-up bucket), recovery codes no
//! longer satisfy step-up (B3), and a storage failure on the success path
//! is never counted as a wrong factor (A9).

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::sync::Arc;
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{SessionId, UserId};
use tower::ServiceExt;

const WRONG_PASSWORD: &str = "definitely-not-the-password";

pub(super) struct Resp {
    pub(super) status: StatusCode,
    pub(super) location: Option<String>,
    pub(super) body: String,
}

/// A CSRF token from a self-service page, which any signed-in user (not
/// only an admin) can open.
pub(super) async fn me_csrf(state: &AppState, session: &str) -> String {
    let resp = build_router(state.clone())
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
    extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie on /me/security/mfa")
}

pub(super) async fn post(state: &AppState, path: &str, session: &str, body: &str) -> Resp {
    let csrf = me_csrf(state, session).await;
    let body = if body.is_empty() {
        format!("_csrf={csrf}")
    } else {
        format!("_csrf={csrf}&{body}")
    };
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    let status = resp.status();
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    Resp {
        status,
        location,
        body,
    }
}

async fn get(state: &AppState, path: &str, session: &str) -> Resp {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(path)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    let status = resp.status();
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    Resp {
        status,
        location,
        body,
    }
}

/// Sign in through `/admin/login` with a self-service `next`, so a
/// non-admin also receives a session cookie.
pub(super) async fn sign_in(state: &AppState, username: &str, password: &str) -> String {
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
    extract_set_cookie(resp.headers(), "sui_id_session").expect("session cookie")
}

async fn user_of(state: &AppState, session: &str) -> UserId {
    let id: SessionId = session.parse().expect("session id");
    sui_id_store::repos::sessions::get(&state.db, id)
        .await
        .expect("session")
        .user_id
}

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn failure_count(state: &AppState, session: &str) -> i64 {
    scalar(
        state,
        format!("SELECT step_up_failure_count FROM sessions WHERE id = '{session}'"),
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

async fn is_revoked(state: &AppState, session: &str) -> bool {
    scalar(
        state,
        format!("SELECT revoked_at IS NOT NULL FROM sessions WHERE id = '{session}'"),
    )
    .await
        == 1
}

pub(super) fn redirected_to_step_up(r: &Resp) -> bool {
    r.status.is_redirection()
        && r.location
            .as_deref()
            .is_some_and(|l| l.starts_with("/me/security/step-up"))
}

/// A signed-in admin with TOTP enabled. Returns (session, secret bytes).
pub(super) async fn totp_user(state: &AppState) -> (String, Vec<u8>) {
    let session = complete_setup_and_login(state).await;
    let (secret_b32, _codes) = enroll_mfa_for(state, &session).await;
    (session, decode_b32(&secret_b32))
}

/// The TOTP code for the step after the one enrolment consumed.
pub(super) async fn next_code(secret: &[u8]) -> String {
    let step = chrono::Utc::now().timestamp() / 30 + 1;
    format!(
        "{:06}",
        sui_id_core::totp::code_for_step(secret, step).await
    )
}

pub(super) async fn add_fake_passkey(state: &AppState, user: UserId) {
    exec(
        state,
        format!(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at) \
             VALUES ('{}', '{user}', X'0102', X'00', 'test', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;
}

// ── B7: the stolen session ───────────────────────────────────────────

#[tokio::test]
async fn r102_b7_stolen_session_with_a_factor_cannot_add_one_without_step_up() {
    let state = test_app();
    let (session, _secret) = totp_user(&state).await;
    let user = user_of(&state, &session).await;
    // The same cookie, held by a second client: no step-up has happened.

    let r = post(
        &state,
        "/me/security/passkeys/register/start",
        &session,
        "nickname=thief",
    )
    .await;
    assert!(redirected_to_step_up(&r), "passkey start: {}", r.status);
    assert_eq!(
        scalar(&state, "SELECT COUNT(*) FROM webauthn_pending".into()).await,
        0,
        "no ceremony started"
    );

    let r = post(
        &state,
        "/me/security/passkeys/register/complete",
        &session,
        "credential=%7B%7D",
    )
    .await;
    assert!(redirected_to_step_up(&r), "passkey complete: {}", r.status);

    let before = sui_id_core::mfa::count_recovery_codes_remaining(&state.db, user)
        .await
        .unwrap();
    let r = post(
        &state,
        "/me/security/mfa/recovery-codes/regenerate",
        &session,
        "",
    )
    .await;
    assert!(redirected_to_step_up(&r), "regenerate: {}", r.status);
    let blob_unchanged = sui_id_core::mfa::count_recovery_codes_remaining(&state.db, user)
        .await
        .unwrap();
    assert_eq!(before, blob_unchanged);
    assert_eq!(
        events(&state, "auth.mfa.factor_added").await,
        1,
        "only the enrolment"
    );
}

#[tokio::test]
async fn r102_b7_passkey_holder_cannot_enrol_totp_without_step_up_even_with_password() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let user = user_of(&state, &session).await;
    add_fake_passkey(&state, user).await;

    let r = post(
        &state,
        "/me/security/mfa/enroll/start",
        &session,
        &format!("current_password={}", urlencode(PASSWORD)),
    )
    .await;
    assert!(redirected_to_step_up(&r), "enroll start: {}", r.status);
    assert_eq!(
        scalar(&state, "SELECT COUNT(*) FROM user_totp".into()).await,
        0,
        "no pending enrolment"
    );

    let r = post(
        &state,
        "/me/security/mfa/enroll/confirm",
        &session,
        "code=123456",
    )
    .await;
    assert!(redirected_to_step_up(&r), "enroll confirm: {}", r.status);
}

#[tokio::test]
async fn r102_b7_local_user_without_a_factor_needs_the_password() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // No password: refused, no enrolment row, counted.
    let r = post(&state, "/me/security/mfa/enroll/start", &session, "").await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        scalar(&state, "SELECT COUNT(*) FROM user_totp".into()).await,
        0
    );

    // Wrong password: the same response, counted.
    let r2 = post(
        &state,
        "/me/security/mfa/enroll/start",
        &session,
        &format!("current_password={}", urlencode(WRONG_PASSWORD)),
    )
    .await;
    assert_eq!(r2.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        scalar(&state, "SELECT COUNT(*) FROM user_totp".into()).await,
        0
    );
    assert_eq!(failure_count(&state, &session).await, 2);
    assert_eq!(events(&state, "auth.step_up.failure").await, 2);

    // The passkey start answers in JSON, and also needs the password.
    let r3 = post(
        &state,
        "/me/security/passkeys/register/start",
        &session,
        "nickname=mine",
    )
    .await;
    assert_eq!(r3.status, StatusCode::BAD_REQUEST);
    assert!(r3.body.starts_with('{'), "JSON error: {}", r3.body);
    assert_eq!(failure_count(&state, &session).await, 3);

    // The right password proceeds to the enrolment page.
    let ok = post(
        &state,
        "/me/security/mfa/enroll/start",
        &session,
        &format!("current_password={}", urlencode(PASSWORD)),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK);
    assert_eq!(
        scalar(&state, "SELECT COUNT(*) FROM user_totp".into()).await,
        1
    );
}

#[tokio::test]
async fn r102_n1_fifth_wrong_password_on_the_enrolment_form_revokes_the_session() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    for i in 1..=5 {
        let r = post(
            &state,
            "/me/security/mfa/enroll/start",
            &session,
            &format!("current_password={}", urlencode(WRONG_PASSWORD)),
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "attempt {i}");
    }
    assert!(is_revoked(&state, &session).await);
    assert_eq!(events(&state, "auth.step_up.failure").await, 4);
    assert_eq!(events(&state, "auth.step_up.session_revoked").await, 1);
    let next = get(&state, "/me/security/mfa", &session).await;
    assert!(
        next.status.is_redirection() && next.location.as_deref() == Some("/admin/login"),
        "the next request is unauthenticated: {} {:?}",
        next.status,
        next.location
    );
}

#[tokio::test]
async fn r102_b7_directory_user_rebinds_and_federated_user_is_refused() {
    use sui_id_store::user_source::InMemoryUserSource;
    let mut state = test_app();
    let _admin = complete_setup_and_login(&state).await;
    let mut users = std::collections::HashMap::new();
    users.insert(
        "bob".to_string(),
        (
            "bob-directory-password".to_string(),
            "uuid-bob".to_string(),
            None,
            None,
        ),
    );
    state.user_sources = vec![Arc::new(InMemoryUserSource {
        slug: "corp".into(),
        users,
    })];
    let bob = sign_in(&state, "bob", "bob-directory-password").await;

    // Wrong directory password: counted.
    let r = post(
        &state,
        "/me/security/mfa/enroll/start",
        &bob,
        "current_password=nope",
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(failure_count(&state, &bob).await, 1);

    // The directory password re-binds.
    let ok = post(
        &state,
        "/me/security/mfa/enroll/start",
        &bob,
        "current_password=bob-directory-password",
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK);

    // No reachable source: refused, not counted.
    let mut offline = state.clone();
    offline.user_sources = Vec::new();
    let r = post(
        &offline,
        "/me/security/mfa/enroll/start",
        &bob,
        "current_password=bob-directory-password",
    )
    .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(failure_count(&state, &bob).await, 1);

    // A federated account: refused, not counted.
    let bob_id = user_of(&state, &bob).await;
    let provider = uuid::Uuid::new_v4();
    exec(
        &state,
        format!(
            "INSERT INTO federation_provider (id, slug, display_name, issuer, client_id, \
             created_at, updated_at) VALUES ('{provider}', 'up', 'Up', 'https://up.test', 'c', \
             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'); \
             INSERT INTO federation_link (user_id, provider_id, upstream_sub, linked_at, \
             last_seen_at) VALUES ('{bob_id}', '{provider}', 'sub-1', '2026-01-01T00:00:00Z', \
             '2026-01-01T00:00:00Z');"
        ),
    )
    .await;
    let r = post(
        &state,
        "/me/security/passkeys/register/start",
        &bob,
        "nickname=x&current_password=bob-directory-password",
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(failure_count(&state, &bob).await, 1);
}

// ── L06 on the step-up form ──────────────────────────────────────────

#[tokio::test]
async fn r102_l06_four_failures_then_success_resets_the_count() {
    let state = test_app();
    let (session, secret) = totp_user(&state).await;
    for _ in 0..4 {
        let r = post(
            &state,
            "/me/security/step-up",
            &session,
            "code=000000&return_to=/me/security/mfa",
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    assert_eq!(failure_count(&state, &session).await, 4);
    let code = next_code(&secret).await;
    let ok = post(
        &state,
        "/me/security/step-up",
        &session,
        &format!("code={code}&return_to=/me/security/mfa"),
    )
    .await;
    assert!(
        ok.status.is_redirection(),
        "step-up succeeds: {}",
        ok.status
    );
    assert_eq!(failure_count(&state, &session).await, 0);
    assert!(!is_revoked(&state, &session).await);
}

#[tokio::test]
async fn r102_l06_five_failures_revoke_the_session() {
    let state = test_app();
    let (session, _secret) = totp_user(&state).await;
    for _ in 0..5 {
        let r = post(
            &state,
            "/me/security/step-up",
            &session,
            "code=000000&return_to=/me/security/mfa",
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    assert!(is_revoked(&state, &session).await);
    assert_eq!(events(&state, "auth.step_up.session_revoked").await, 1);
    let next = get(&state, "/me/security/mfa", &session).await;
    assert_eq!(next.location.as_deref(), Some("/admin/login"));
}

#[tokio::test]
async fn r102_step_up_bucket_refuses_a_burst() {
    let state = test_app();
    // No second factor: every step-up attempt is a wrong factor.
    let s1 = complete_setup_and_login(&state).await;
    let s2 = sign_in(&state, USERNAME, PASSWORD).await;
    let s3 = sign_in(&state, USERNAME, PASSWORD).await;
    // Ten attempts spread over three sessions stay under each session's
    // revocation threshold; the eleventh from the same address is refused.
    let mut statuses = Vec::new();
    for i in 0..11 {
        let s = [&s1, &s2, &s3][i % 3];
        let r = post(
            &state,
            "/me/security/step-up",
            s,
            "code=000000&return_to=/me/security/mfa",
        )
        .await;
        statuses.push(r.status);
    }
    assert!(
        statuses[..10].iter().all(|s| *s == StatusCode::BAD_REQUEST),
        "{statuses:?}"
    );
    assert_eq!(statuses[10], StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn r102_a9_storage_failure_on_the_success_path_is_not_counted() {
    let state = test_app();
    let (session, secret) = totp_user(&state).await;
    exec(
        &state,
        "CREATE TRIGGER r102_block_touch BEFORE UPDATE OF last_step_up_at ON sessions \
         BEGIN SELECT RAISE(ABORT, 'r102 test: step-up touch rejected'); END;"
            .into(),
    )
    .await;
    let code = next_code(&secret).await;
    let r = post(
        &state,
        "/me/security/step-up",
        &session,
        &format!("code={code}&return_to=/me/security/mfa"),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "the uniform response");
    assert_eq!(failure_count(&state, &session).await, 0, "not counted");
    assert_eq!(events(&state, "auth.step_up.failure").await, 0);
}

// ── B3 ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn r102_b3_recovery_code_does_not_satisfy_step_up() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (_secret, codes) = enroll_mfa_for(&state, &session).await;
    let user = user_of(&state, &session).await;
    let r = post(
        &state,
        "/me/security/step-up",
        &session,
        &format!("code={}&return_to=/me/security/mfa", codes[0]),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        sui_id_core::mfa::count_recovery_codes_remaining(&state.db, user)
            .await
            .unwrap(),
        8,
        "not consumed"
    );
    assert_eq!(
        scalar(
            &state,
            format!("SELECT last_step_up_at IS NULL FROM sessions WHERE id = '{session}'")
        )
        .await,
        1,
        "freshness unchanged"
    );
}

// ── auth.mfa.factor_added is atomic with the enrolment ──────────────

#[tokio::test]
async fn r102_factor_added_rolls_back_with_its_write() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let ok = post(
        &state,
        "/me/security/mfa/enroll/start",
        &session,
        &format!("current_password={}", urlencode(PASSWORD)),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK);
    let user = user_of(&state, &session).await;
    let pending = sui_id_store::repos::user_totp::get(&state.db, user)
        .await
        .unwrap()
        .unwrap();
    let secret = sui_id_store::repos::user_totp::decrypt_secret(&state.db, &pending)
        .await
        .unwrap();
    exec(
        &state,
        "CREATE TRIGGER r102_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'r102 test: audit rejected'); END;"
            .into(),
    )
    .await;
    let step = chrono::Utc::now().timestamp() / 30;
    let code = format!(
        "{:06}",
        sui_id_core::totp::code_for_step(&secret, step).await
    );
    let r = post(
        &state,
        "/me/security/mfa/enroll/confirm",
        &session,
        &format!("code={code}"),
    )
    .await;
    assert!(
        !r.status.is_success(),
        "confirm fails with the audit log down"
    );
    assert_eq!(
        scalar(&state, "SELECT enabled FROM user_totp".into()).await,
        0,
        "enrolment rolled back"
    );
}
