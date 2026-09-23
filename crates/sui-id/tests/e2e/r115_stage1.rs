//! RFC 115 stage 1 — a local account that has never held a password is not
//! activated by `/forgot-password` (D1), and signing in to one fails like any
//! other wrong password: the dummy verify runs and the failure is counted and
//! audited (D8).
//!
//! The users here are created with `users::create` and no `credentials` row,
//! the shape RFC 115 stage 2 will make the web produce.

use super::common::*;
use super::r103_stage1::{Resp, csrf_from, events, reset_app, send};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::time::{Duration, Instant};
use sui_id::AppState;
use sui_id_shared::ids::UserId;
use sui_id_store::metrics::{Metrics, signin_result};
use sui_id_store::models::{CredentialRow, Role, UserRow, UserSource};
use sui_id_store::repos::{credentials, users};

const WRONG: &str = "definitely-not-the-password";

/// A local user with an address and, unless `password` is given, no
/// credential row at all.
async fn seed(
    state: &AppState,
    username: &str,
    email: &str,
    source: UserSource,
    password: Option<&str>,
) -> UserId {
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
            source,
            external_stable_id: None,
            email: Some(email.into()),
            preferred_lang: None,
            email_normalized: Some(sui_id_shared::normalize_email(email)),
            email_verified_at: None,
        },
    )
    .await
    .expect("create user");
    if let Some(password) = password {
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
    }
    uid
}

async fn forgot(state: &AppState, email: &str) -> Resp {
    let csrf = csrf_from(state, "/forgot-password").await;
    send(
        state,
        Request::builder()
            .method(Method::POST)
            .uri("/forgot-password")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
            .body(Body::from(format!(
                "_csrf={csrf}&email={}",
                urlencode(email)
            )))
            .expect("req"),
    )
    .await
}

/// The response headers that must not differ, minus the two that legitimately
/// vary per request (the CSRF cookie and the request id).
fn stable_headers(r: &Resp) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = r
        .headers
        .iter()
        .filter(|(k, _)| *k != header::SET_COOKIE && k.as_str() != "x-request-id")
        .map(|(k, val)| {
            (
                k.as_str().to_owned(),
                val.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    v.sort();
    v
}

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

async fn login(state: &AppState, username: &str, password: &str) -> (StatusCode, Vec<u8>, bool) {
    let body = format!(
        "username={}&password={}&next=",
        urlencode(username),
        urlencode(password)
    );
    let resp = tower::ServiceExt::oneshot(
        sui_id::build_router(state.clone()),
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .expect("req"),
    )
    .await
    .expect("login");
    let status = resp.status();
    let has_session = extract_set_cookie(resp.headers(), "sui_id_session").is_some();
    (status, read_body(resp.into_body()).await, has_session)
}

fn wrong_password_count(state: &AppState) -> u64 {
    state
        .metric()
        .expect("metrics enabled")
        .signin_attempts_total
        .with_label_values(&[signin_result::WRONG_PASSWORD])
        .get()
}

async fn failed_count(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT failed_login_count FROM users WHERE id = '{user}'"),
    )
    .await
}

// ── D1 ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn r115_d1_forgot_password_gives_a_never_activated_account_the_neutral_response() {
    let (state, mailer, _admin) = reset_app().await;
    let fresh = seed(
        &state,
        "fresh",
        "fresh@test.invalid",
        UserSource::Local,
        None,
    )
    .await;
    seed(
        &state,
        "ldapuser",
        "ldap@test.invalid",
        UserSource::Ldap,
        None,
    )
    .await;

    // Control: an account that has a password still gets its mail, so the
    // silence below is the rule and not a mailer that never sends.
    let control = forgot(&state, "alice@test.invalid").await;
    assert_eq!(control.status, StatusCode::OK);
    assert_eq!(
        mailer.count().await,
        1,
        "control: a credentialed account is mailed"
    );

    let requested_before = events(&state, "auth.password.reset_requested").await;
    let unknown = forgot(&state, "nobody@test.invalid").await;
    let non_local = forgot(&state, "ldap@test.invalid").await;
    let never_activated = forgot(&state, "fresh@test.invalid").await;

    // The equality is asserted directly: status, body and every header that
    // is not per-request. If `/forgot-password` differed for a never
    // activated account by so much as a character it would be an oracle.
    for (name, r) in [
        ("non-local", &non_local),
        ("never activated", &never_activated),
    ] {
        assert_eq!(r.status, unknown.status, "{name}: status");
        assert_eq!(r.body, unknown.body, "{name}: body");
        assert_eq!(
            stable_headers(r),
            stable_headers(&unknown),
            "{name}: headers"
        );
        assert_eq!(r.location, unknown.location, "{name}: location");
    }
    assert_eq!(unknown.status, StatusCode::OK);

    assert_eq!(
        mailer.count().await,
        1,
        "no mail for the never-activated account"
    );
    assert_eq!(
        scalar(
            &state,
            format!("SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = '{fresh}'")
        )
        .await,
        0,
        "no token for the never-activated account"
    );
    // The same Class-B event as an unknown address: one per request, naming
    // no user.
    assert_eq!(
        events(&state, "auth.password.reset_requested").await,
        requested_before + 3
    );
    assert_eq!(
        scalar(
            &state,
            format!(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.password.reset_requested' \
                 AND (actor = '{fresh}' OR target = '{fresh}')"
            )
        )
        .await,
        0,
        "the request names no user"
    );
    assert_eq!(
        events(&state, "auth.password.reset_email_sent").await,
        1,
        "only the control's mail was sent"
    );
}

#[tokio::test]
async fn r115_d1_an_account_that_has_held_a_password_is_unaffected() {
    // The check is on the credential row. An account that has one (every
    // account ever activated: credential rows are never deleted) is mailed
    // whether or not it has ever signed in.
    let (state, mailer, _admin) = reset_app().await;
    seed(
        &state,
        "carol",
        "carol@test.invalid",
        UserSource::Local,
        Some("carol-correct-password"),
    )
    .await;
    assert_eq!(
        forgot(&state, "carol@test.invalid").await.status,
        StatusCode::OK
    );
    assert_eq!(mailer.count().await, 1);
}

// ── D8 ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn r115_d8_a_never_activated_sign_in_fails_like_a_wrong_password_and_is_counted() {
    let mut state = test_app();
    state.metrics = Some(Metrics::new().expect("metrics"));
    complete_setup_and_login(&state).await;
    let fresh = seed(
        &state,
        "fresh",
        "fresh@test.invalid",
        UserSource::Local,
        None,
    )
    .await;
    seed(
        &state,
        "carol",
        "carol@test.invalid",
        UserSource::Local,
        Some("carol-correct-password"),
    )
    .await;

    // The reference: a known user, wrong password.
    let m0 = wrong_password_count(&state);
    let (ref_status, ref_body, ref_session) = login(&state, "carol", WRONG).await;
    assert_eq!(wrong_password_count(&state) - m0, 1);
    assert_eq!(ref_status, StatusCode::UNAUTHORIZED);
    assert!(!ref_session);

    let failures_before = events(&state, "auth.login.failure").await;
    let m1 = wrong_password_count(&state);
    let (status, body, session) = login(&state, "fresh", WRONG).await;
    assert_eq!(status, ref_status, "status");
    assert_eq!(body, ref_body, "body identical to a wrong password's");
    assert!(!session, "no session");
    assert_eq!(
        wrong_password_count(&state) - m1,
        1,
        "the metric counts it as a wrong password"
    );
    assert_eq!(
        events(&state, "auth.login.failure").await,
        failures_before + 1,
        "the failure is audited"
    );
    assert_eq!(failed_count(&state, fresh).await, 1, "the counter moved");

    // The counter drives the lockout exactly as it does for any account.
    login(&state, "fresh", WRONG).await;
    assert_eq!(events(&state, "auth.lockout").await, 0);
    login(&state, "fresh", WRONG).await;
    assert_eq!(failed_count(&state, fresh).await, 3);
    assert_eq!(
        events(&state, "auth.lockout").await,
        1,
        "the third failure locks the account, as for a credentialed one"
    );
    assert_eq!(
        scalar(
            &state,
            format!("SELECT locked_until IS NOT NULL FROM users WHERE id = '{fresh}'")
        )
        .await,
        1
    );
}

#[tokio::test]
async fn r115_d8_a_never_activated_sign_in_costs_an_argon2_verify() {
    // The dummy verify is observable only as time. A password check is tens
    // of milliseconds or more; a lookup that returns early is well under one.
    // The bar is set far below the real cost and far above the early return,
    // so this is not a benchmark and does not depend on the machine's speed.
    let state = test_app();
    complete_setup_and_login(&state).await;
    for i in 0..3 {
        seed(
            &state,
            &format!("known{i}"),
            &format!("known{i}@test.invalid"),
            UserSource::Local,
            Some("known-correct-password"),
        )
        .await;
        seed(
            &state,
            &format!("fresh{i}"),
            &format!("fresh{i}@test.invalid"),
            UserSource::Local,
            None,
        )
        .await;
    }
    async fn fastest(state: &AppState, prefix: &str) -> Duration {
        let mut best = Duration::MAX;
        for i in 0..3 {
            let start = Instant::now();
            let (status, _, _) = login(state, &format!("{prefix}{i}"), WRONG).await;
            best = best.min(start.elapsed());
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        best
    }
    let wrong_password = fastest(&state, "known").await;
    let never_activated = fastest(&state, "fresh").await;
    assert!(
        never_activated * 4 >= wrong_password,
        "a never-activated sign-in ({never_activated:?}) skipped the Argon2 verify \
         a wrong password ({wrong_password:?}) pays"
    );
}

#[tokio::test]
async fn r115_d8_a_credentialless_account_is_still_refused_when_the_password_is_empty() {
    // The branch must not depend on what was submitted.
    let state = test_app();
    complete_setup_and_login(&state).await;
    let fresh = seed(
        &state,
        "fresh",
        "fresh@test.invalid",
        UserSource::Local,
        None,
    )
    .await;
    let (status, _, session) = login(&state, "fresh", "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(!session);
    assert_eq!(failed_count(&state, fresh).await, 1);
}
