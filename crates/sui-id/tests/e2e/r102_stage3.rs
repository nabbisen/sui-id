//! RFC 102 stage 3 — L02 and L07: a second-factor sign-in commits its
//! session, its factor's guard, its bookkeeping and `auth.mfa.success`
//! together (A1, A2); a wrong factor is counted on the user (A8); a failed
//! commit is never counted (A9); freshness follows the method (N5); A7
//! holds on the MFA path; and the passkey sign-in keeps `next` (N14).

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{PendingMfaId, UserId};
use tower::ServiceExt;

const LOCKOUT: i64 = 48 * 60 * 60;

struct Resp {
    status: StatusCode,
    location: Option<String>,
    session: Option<String>,
    pending: Option<String>,
    body: String,
}

async fn send(state: &AppState, req: Request<Body>) -> Resp {
    let resp = build_router(state.clone())
        .oneshot(req)
        .await
        .expect("send");
    let status = resp.status();
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let session = extract_set_cookie(resp.headers(), "sui_id_session").filter(|v| !v.is_empty());
    let pending =
        extract_set_cookie(resp.headers(), "sui_id_pending_mfa").filter(|v| !v.is_empty());
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    Resp {
        status,
        location,
        session,
        pending,
        body,
    }
}

/// The password step. Returns the pending-MFA cookie when one is issued.
async fn password_step(state: &AppState, username: &str, password: &str, next: &str) -> Resp {
    send(
        state,
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
}

/// Submit `code` on the challenge page for `pending`.
async fn challenge(state: &AppState, pending: &str, code: &str) -> Resp {
    let page = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login/mfa")
                .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("challenge page");
    let csrf = extract_set_cookie(page.headers(), "sui_id_csrf").expect("csrf");
    send(
        state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login/mfa")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                header::COOKIE,
                format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
            )
            .body(Body::from(format!("_csrf={csrf}&code={}", urlencode(code))))
            .expect("req"),
    )
    .await
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

async fn events(state: &AppState, action: &str) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM audit_log WHERE action = '{action}'"),
    )
    .await
}

async fn pending_rows(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM login_pending_mfa WHERE user_id = '{user}'"),
    )
    .await
}

async fn sessions_of(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM sessions WHERE user_id = '{user}'"),
    )
    .await
}

async fn mfa_failures(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT mfa_failure_count FROM users WHERE id = '{user}'"),
    )
    .await
}

async fn last_used_step(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT last_used_step FROM user_totp WHERE user_id = '{user}'"),
    )
    .await
}

async fn recovery_codes_left(state: &AppState, user: UserId) -> usize {
    sui_id_core::mfa::count_recovery_codes_remaining(&state.db, user)
        .await
        .expect("count")
}

fn step_now() -> i64 {
    chrono::Utc::now().timestamp() / 30
}

async fn code_at(secret: &[u8], step: i64) -> String {
    format!(
        "{:06}",
        sui_id_core::totp::code_for_step(secret, step).await
    )
}

/// The setup admin with TOTP enrolled, the replay cursor two steps back
/// so the current and previous codes are both usable.
struct MfaAdmin {
    state: AppState,
    user: UserId,
    secret: Vec<u8>,
    codes: Vec<String>,
}

async fn mfa_admin() -> MfaAdmin {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (secret_b32, codes) = enroll_mfa_for(&state, &session).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let admin = MfaAdmin {
        state,
        user,
        secret: decode_b32(&secret_b32),
        codes,
    };
    rewind_step(&admin).await;
    admin
}

async fn rewind_step(admin: &MfaAdmin) {
    exec(
        &admin.state,
        format!(
            "UPDATE user_totp SET last_used_step = {} WHERE user_id = '{}'",
            step_now() - 2,
            admin.user
        ),
    )
    .await;
}

async fn pending_for(admin: &MfaAdmin) -> String {
    let r = password_step(&admin.state, USERNAME, PASSWORD, "").await;
    assert_eq!(
        r.location.as_deref(),
        Some("/admin/login/mfa"),
        "password step: {} {:?}",
        r.status,
        r.location
    );
    r.pending.expect("pending cookie")
}

fn normalise_csrf(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    let marker = r#"name="_csrf" value=""#;
    while let Some(i) = rest.find(marker) {
        out.push_str(&rest[..i + marker.len()]);
        rest = &rest[i + marker.len()..];
        let end = rest.find('"').expect("closing quote");
        out.push_str("CSRF");
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

// ── Happy paths and freshness ────────────────────────────────────────

#[tokio::test]
async fn r102_l02_totp_sign_in_commits_everything_and_is_fresh() {
    let a = mfa_admin().await;
    let pending = pending_for(&a).await;
    // Counters left from earlier attempts, reset only when L02 commits.
    exec(
        &a.state,
        format!(
            "UPDATE users SET failed_login_count = 2, mfa_failure_count = 3, \
             locked_until = '2000-01-01T00:00:00Z', last_login_at = NULL WHERE id = '{}'",
            a.user
        ),
    )
    .await;
    let sessions_before = sessions_of(&a.state, a.user).await;

    let r = challenge(&a.state, &pending, &code_at(&a.secret, step_now()).await).await;
    assert_eq!(r.location.as_deref(), Some("/admin"), "{}", r.status);
    let session = r.session.expect("session cookie");

    assert_eq!(sessions_of(&a.state, a.user).await, sessions_before + 1);
    assert_eq!(pending_rows(&a.state, a.user).await, 0, "pending consumed");
    assert_eq!(last_used_step(&a.state, a.user).await, step_now());
    assert_eq!(
        text(
            &a.state,
            format!("SELECT last_step_up_method FROM sessions WHERE id = '{session}'")
        )
        .await
        .as_deref(),
        Some("totp")
    );
    assert_eq!(
        scalar(
            &a.state,
            format!("SELECT last_step_up_at IS NOT NULL FROM sessions WHERE id = '{session}'")
        )
        .await,
        1,
        "a TOTP sign-in is fresh"
    );
    assert_eq!(
        scalar(
            &a.state,
            format!(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.mfa.success' \
                 AND actor = '{0}' AND target = '{0}' AND note = 'method=totp evicted=0'",
                a.user
            )
        )
        .await,
        1,
        "one Atomic success event"
    );
    assert_eq!(
        scalar(
            &a.state,
            format!(
                "SELECT failed_login_count + mfa_failure_count + (locked_until IS NOT NULL) \
                 + (last_login_at IS NULL) FROM users WHERE id = '{}'",
                a.user
            )
        )
        .await,
        0,
        "both counters and the stale lock reset, last_login_at set"
    );
}

#[tokio::test]
async fn r102_l02_recovery_code_sign_in_is_not_fresh() {
    let a = mfa_admin().await;
    let pending = pending_for(&a).await;
    let r = challenge(&a.state, &pending, &a.codes[0]).await;
    assert_eq!(r.location.as_deref(), Some("/admin"), "{}", r.status);
    let session = r.session.expect("session cookie");

    assert_eq!(
        recovery_codes_left(&a.state, a.user).await,
        7,
        "one code spent"
    );
    assert_eq!(
        scalar(
            &a.state,
            format!(
                "SELECT (last_step_up_at IS NULL) + (last_step_up_method IS NULL) \
                 FROM sessions WHERE id = '{session}'"
            )
        )
        .await,
        2,
        "a recovery-code sign-in sets no freshness"
    );
    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.mfa.success' \
             AND note = 'method=recovery_code evicted=0'"
                .into()
        )
        .await,
        1
    );

    // A step-up-gated action on that session asks for a step-up.
    let csrf = fetch_csrf(&a.state, &session).await;
    let r = send(
        &a.state,
        Request::builder()
            .method(Method::POST)
            .uri("/me/security/mfa/disable")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                header::COOKIE,
                format!("sui_id_session={session}; sui_id_csrf={csrf}"),
            )
            .body(Body::from(format!("_csrf={csrf}")))
            .expect("req"),
    )
    .await;
    assert!(
        r.location
            .as_deref()
            .is_some_and(|l| l.starts_with("/me/security/step-up")),
        "not fresh: {} {:?}",
        r.status,
        r.location
    );
}

// ── Injected append failure ──────────────────────────────────────────

#[tokio::test]
async fn r102_l02_append_failure_changes_nothing_and_looks_like_a_wrong_code() {
    // The ordinary wrong-code response, from a working instance.
    let control = mfa_admin().await;
    let control_pending = pending_for(&control).await;
    let ordinary = challenge(&control.state, &control_pending, "000000").await;
    assert_eq!(ordinary.status, StatusCode::UNAUTHORIZED);

    let a = mfa_admin().await;
    let pending = pending_for(&a).await;
    exec(
        &a.state,
        format!(
            "UPDATE users SET failed_login_count = 2, mfa_failure_count = 1 WHERE id = '{}'",
            a.user
        ),
    )
    .await;
    let step_before = last_used_step(&a.state, a.user).await;
    let sessions_before = sessions_of(&a.state, a.user).await;
    exec(
        &a.state,
        "CREATE TRIGGER r102_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'r102 test: audit_log insert rejected'); END;"
            .into(),
    )
    .await;

    for (label, code) in [
        ("TOTP", code_at(&a.secret, step_now()).await),
        ("recovery code", a.codes[0].clone()),
    ] {
        let r = challenge(&a.state, &pending, &code).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{label}");
        assert!(r.session.is_none(), "{label}: no session cookie");
        assert_eq!(
            normalise_csrf(&r.body),
            normalise_csrf(&ordinary.body),
            "{label}: the ordinary wrong-code response"
        );
        assert_eq!(
            sessions_of(&a.state, a.user).await,
            sessions_before,
            "{label}"
        );
        assert_eq!(
            pending_rows(&a.state, a.user).await,
            1,
            "{label}: pending kept"
        );
        assert_eq!(
            last_used_step(&a.state, a.user).await,
            step_before,
            "{label}"
        );
        assert_eq!(recovery_codes_left(&a.state, a.user).await, 8, "{label}");
        assert_eq!(
            scalar(
                &a.state,
                format!(
                    "SELECT failed_login_count * 10 + mfa_failure_count FROM users \
                     WHERE id = '{}'",
                    a.user
                )
            )
            .await,
            21,
            "{label}: counters unchanged, and the right code was not counted"
        );
    }
}

// ── L07 ──────────────────────────────────────────────────────────────

/// A sign-in ceremony the user started (`Authenticate` kind). L02 must leave
/// it alone: on the real path `webauthn::finish_authentication` deletes an
/// `Authenticate` ceremony before L02 runs, outside L02's transaction.
async fn seed_sign_in_ceremony(a: &MfaAdmin) -> sui_id_shared::ids::WebauthnPendingId {
    let now = chrono::Utc::now();
    let row = sui_id_store::models::WebauthnPendingRow {
        id: sui_id_shared::ids::WebauthnPendingId::new(),
        kind: sui_id_store::models::WebauthnPendingKind::Authenticate,
        user_id: Some(a.user),
        state_json: "{}".into(),
        expires_at: now + chrono::Duration::minutes(5),
        created_at: now,
    };
    sui_id_store::repos::webauthn_pending::insert(&a.state.db, &row)
        .await
        .expect("ceremony");
    row.id
}

async fn last_login(state: &AppState, user: UserId) -> String {
    text(
        state,
        format!("SELECT COALESCE(last_login_at, '') FROM users WHERE id = '{user}'"),
    )
    .await
    .unwrap_or_default()
}

async fn ceremony_rows(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM webauthn_pending WHERE user_id = '{user}'"),
    )
    .await
}

#[tokio::test]
async fn r102_l02_webauthn_proof_append_failure_changes_nothing() {
    // `verify_pending_webauthn` is what the sign-in handler calls once the
    // assertion has been verified (no authenticator is available to a test,
    // so the command is called directly, as the race test does).
    let a = mfa_admin().await;
    let pending = issue_pending(&a).await;
    let ceremony = seed_sign_in_ceremony(&a).await;
    exec(
        &a.state,
        format!(
            "UPDATE users SET failed_login_count = 2, mfa_failure_count = 1 WHERE id = '{}'",
            a.user
        ),
    )
    .await;
    let sessions_before = sessions_of(&a.state, a.user).await;
    let pending_before = pending_rows(&a.state, a.user).await;
    let step_before = last_used_step(&a.state, a.user).await;
    let events_before = events(&a.state, "auth.mfa.success").await;
    let login_before = last_login(&a.state, a.user).await;
    exec(
        &a.state,
        "CREATE TRIGGER r102_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'r102 test: audit_log insert rejected'); END;"
            .into(),
    )
    .await;

    let failed =
        sui_id_core::mfa::verify_pending_webauthn(&a.state.db, &a.state.clock, pending, a.user)
            .await;
    assert!(failed.is_err(), "the sign-in does not commit");
    assert!(
        !matches!(failed, Err(sui_id_core::errors::CoreError::Unauthenticated)),
        "a storage failure is not reported as a lost guard: {failed:?}"
    );
    assert_eq!(
        sessions_of(&a.state, a.user).await,
        sessions_before,
        "no session"
    );
    assert_eq!(
        pending_rows(&a.state, a.user).await,
        pending_before,
        "the pending row is kept"
    );
    assert_eq!(
        ceremony_rows(&a.state, a.user).await,
        1,
        "the ceremony is untouched ({ceremony})"
    );
    assert_eq!(last_used_step(&a.state, a.user).await, step_before);
    assert_eq!(events(&a.state, "auth.mfa.success").await, events_before);
    assert_eq!(
        scalar(
            &a.state,
            format!(
                "SELECT failed_login_count * 10 + mfa_failure_count FROM users WHERE id = '{}'",
                a.user
            )
        )
        .await,
        21,
        "nothing was counted, and the passed proof did not reset a counter"
    );
    assert_eq!(
        last_login(&a.state, a.user).await,
        login_before,
        "last_login_at is unchanged"
    );

    // Control: the same pending row completes once the trigger is gone, so the
    // failure above was the audit append and nothing else.
    exec(&a.state, "DROP TRIGGER r102_reject_audit".into()).await;
    let session =
        sui_id_core::mfa::verify_pending_webauthn(&a.state.db, &a.state.clock, pending, a.user)
            .await
            .expect("the control completes");
    assert_eq!(sessions_of(&a.state, a.user).await, sessions_before + 1);
    assert_eq!(pending_rows(&a.state, a.user).await, pending_before - 1);
    assert_eq!(
        events(&a.state, "auth.mfa.success").await,
        events_before + 1
    );
    assert_eq!(
        text(
            &a.state,
            format!(
                "SELECT last_step_up_method FROM sessions WHERE id = '{}'",
                session.id
            )
        )
        .await
        .as_deref(),
        Some("webauthn"),
        "a WebAuthn sign-in is fresh, by that method"
    );
    assert_eq!(
        ceremony_rows(&a.state, a.user).await,
        1,
        "L02 never touches the ceremony, on success either"
    );
}

#[tokio::test]
async fn r102_l07_four_wrong_codes_then_the_right_one_signs_in() {
    let a = mfa_admin().await;
    let pending = pending_for(&a).await;
    for _ in 0..4 {
        let r = challenge(&a.state, &pending, "000000").await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(mfa_failures(&a.state, a.user).await, 4);
    assert_eq!(events(&a.state, "auth.mfa.failure").await, 4);

    let r = challenge(&a.state, &pending, &code_at(&a.secret, step_now()).await).await;
    assert!(r.session.is_some(), "signed in: {}", r.status);
    assert_eq!(mfa_failures(&a.state, a.user).await, 0, "count reset");
}

#[tokio::test]
async fn r102_l07_five_wrong_codes_lock_the_account_across_pending_rows() {
    let a = mfa_admin().await;
    // Two wrong codes on one pending row, then a new row minted with the
    // password: the count carries over (A8).
    let first = pending_for(&a).await;
    for _ in 0..2 {
        challenge(&a.state, &first, "000000").await;
    }
    let second = pending_for(&a).await;
    assert_eq!(pending_rows(&a.state, a.user).await, 2);
    for _ in 0..2 {
        challenge(&a.state, &second, "000000").await;
    }
    assert_eq!(
        mfa_failures(&a.state, a.user).await,
        4,
        "a new pending row does not reset the count"
    );
    challenge(&a.state, &second, "000000").await;

    assert_eq!(mfa_failures(&a.state, a.user).await, 5);
    assert_eq!(events(&a.state, "auth.mfa.lockout").await, 1);
    assert_eq!(
        pending_rows(&a.state, a.user).await,
        0,
        "every pending row removed"
    );
    let locked_until = sui_id_store::repos::users::get(&a.state.db, a.user)
        .await
        .expect("user")
        .locked_until;
    assert!(
        locked_until.is_some_and(|until| until > chrono::Utc::now()),
        "the account is locked: {locked_until:?}"
    );

    // The right code is refused on either row, and so is the password.
    let sessions_before = sessions_of(&a.state, a.user).await;
    for pending in [&first, &second] {
        let r = challenge(&a.state, pending, &code_at(&a.secret, step_now()).await).await;
        assert!(r.session.is_none(), "the right code is refused");
    }
    assert_eq!(sessions_of(&a.state, a.user).await, sessions_before);
    let r = password_step(&a.state, USERNAME, PASSWORD, "").await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "locked at the password step"
    );
}

// ── A7 on the MFA path ───────────────────────────────────────────────

/// RFC 102 stage 9c: `sui-id admin unlock-user` (U08) clears the
/// second-factor failure count with the lock. Before, the count stayed at five
/// after `auth.mfa.lockout`, so the next wrong code was the sixth and locked
/// the account again for a longer window.
#[tokio::test]
async fn r102_u08_unlock_after_an_mfa_lockout_leaves_the_next_wrong_code_at_count_one() {
    let a = mfa_admin().await;
    let pending = pending_for(&a).await;
    for _ in 0..5 {
        challenge(&a.state, &pending, "000000").await;
    }
    assert_eq!(mfa_failures(&a.state, a.user).await, 5);
    assert_eq!(events(&a.state, "auth.mfa.lockout").await, 1);
    let locked = sui_id_store::repos::users::get(&a.state.db, a.user)
        .await
        .expect("user");
    assert!(locked.locked_until.is_some(), "the account is locked");

    // The operator's unlock: the same command the CLI runs.
    sui_id_store::commands::admin_unlock_user(&a.state.db, a.user)
        .await
        .expect("unlock");
    let unlocked = sui_id_store::repos::users::get(&a.state.db, a.user)
        .await
        .expect("user");
    assert!(unlocked.locked_until.is_none(), "the lock is gone");
    assert_eq!(unlocked.failed_login_count, 0);
    assert_eq!(
        mfa_failures(&a.state, a.user).await,
        0,
        "the count is cleared"
    );

    // The next wrong code is the first, not the sixth.
    let fresh = pending_for(&a).await;
    let r = challenge(&a.state, &fresh, "000000").await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(mfa_failures(&a.state, a.user).await, 1, "count 1");
    assert_eq!(
        events(&a.state, "auth.mfa.lockout").await,
        1,
        "no second lockout"
    );
    let after = sui_id_store::repos::users::get(&a.state.db, a.user)
        .await
        .expect("user");
    assert!(
        after.locked_until.is_none(),
        "the account is not locked again"
    );

    // And the account is usable: the right code signs in.
    let r = challenge(&a.state, &fresh, &code_at(&a.secret, step_now()).await).await;
    assert!(r.session.is_some(), "signed in: {}", r.status);
}

#[tokio::test]
async fn r102_a7_non_admin_with_mfa_to_an_admin_destination_gets_no_pending_row() {
    let a = mfa_admin().await;
    create_user_with_password(
        &a.state.db,
        &a.state.clock,
        &admin_actor_for(a.user),
        sui_id_core::admin::CreateUserSpec {
            username: "carol",
            display_name: None,
            email: None,
            is_admin: false,
        },
        "carol-very-strong-password",
    )
    .await
    .expect("create carol");
    let carol = sui_id_store::repos::users::find_by_username(&a.state.db, "carol")
        .await
        .expect("carol")
        .id;
    // Carol's factor: a passkey row is enough for `is_mfa_enabled`.
    exec(
        &a.state,
        format!(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at) \
             VALUES ('{}', '{carol}', X'0102', X'00', 'k', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;

    let r = password_step(&a.state, "carol", "carol-very-strong-password", "/admin").await;
    assert_eq!(r.status, StatusCode::OK, "the refusal page");
    assert!(r.pending.is_none() && r.session.is_none());
    assert_eq!(pending_rows(&a.state, carol).await, 0, "no pending row");
    assert_eq!(
        events(&a.state, "auth.login.password_ok_mfa_required").await,
        0
    );

    let r = password_step(
        &a.state,
        "carol",
        "carol-very-strong-password",
        "/me/security",
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/admin/login/mfa"));
    assert_eq!(pending_rows(&a.state, carol).await, 1);
}

// ── N14 ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn r102_n14_passkey_sign_in_failure_keeps_next_and_counts_nothing_on_a_bad_ceremony() {
    let a = mfa_admin().await;
    let next = "/oauth2/authorize?client_id=x&state=y";
    let pending = password_step(&a.state, USERNAME, PASSWORD, next)
        .await
        .pending
        .expect("pending");
    let page = build_router(a.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login/mfa")
                .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_set_cookie(page.headers(), "sui_id_csrf").expect("csrf");

    // No passkey ceremony was started: every error on this step is the
    // same redirect back to sign-in, carrying `next`.
    let r = send(
        &a.state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login/webauthn/complete")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                header::COOKIE,
                format!(
                    "sui_id_pending_mfa={pending}; sui_id_csrf={csrf}; \
                     sui_id_pending_mfa_next={}",
                    urlencode(next)
                ),
            )
            .body(Body::from(format!("_csrf={csrf}&credential={{}}")))
            .expect("req"),
    )
    .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    let location = r.location.expect("location");
    assert_eq!(
        location,
        format!("/admin/login?next={}", urlencode(next)),
        "next survives the failure"
    );
    assert!(r.session.is_none());
    assert_eq!(pending_rows(&a.state, a.user).await, 1, "pending kept");
    assert_eq!(mfa_failures(&a.state, a.user).await, 0, "nothing counted");

    // The login page renders that `next` into its form.
    let login = build_router(a.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&location)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("login page");
    let body = String::from_utf8_lossy(&read_body(login.into_body()).await).into_owned();
    assert!(body.contains("client_id=x"), "next carried into the form");

    // The sign-in script submits the completion natively, so the browser
    // follows the server's redirect (to `next` on success).
    let script = send(
        &a.state,
        Request::builder()
            .method(Method::GET)
            .uri("/static/webauthn.js")
            .body(Body::empty())
            .expect("req"),
    )
    .await;
    assert!(script.body.contains("complete.submit()"));
    assert!(!script.body.contains(r#"window.location.href = "/admin";"#));
}

#[tokio::test]
async fn r102_n14_totp_sign_in_lands_on_next() {
    let a = mfa_admin().await;
    let next = "/me/security";
    let r = password_step(&a.state, USERNAME, PASSWORD, next).await;
    let pending = r.pending.expect("pending");
    let page = build_router(a.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login/mfa")
                .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_set_cookie(page.headers(), "sui_id_csrf").expect("csrf");
    let code = code_at(&a.secret, step_now()).await;
    let r = send(
        &a.state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login/mfa")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                header::COOKIE,
                format!(
                    "sui_id_pending_mfa={pending}; sui_id_csrf={csrf}; \
                     sui_id_pending_mfa_next={}",
                    urlencode(next)
                ),
            )
            .body(Body::from(format!("_csrf={csrf}&code={code}")))
            .expect("req"),
    )
    .await;
    assert_eq!(r.location.as_deref(), Some(next));
}

// ── Concurrency (multi-thread runtime, repeated) ─────────────────────

const ITERATIONS: usize = 12;

async fn issue_pending(a: &MfaAdmin) -> PendingMfaId {
    sui_id_core::mfa::issue_pending_mfa(&a.state.db, &a.state.clock, a.user)
        .await
        .expect("pending")
        .id
}

async fn race(
    a: &MfaAdmin,
    first: (PendingMfaId, String),
    second: (PendingMfaId, String),
) -> usize {
    let spawn = |(pending, code): (PendingMfaId, String)| {
        let state = a.state.clone();
        tokio::spawn(async move {
            sui_id_core::mfa::verify_pending(&state.db, &state.clock, pending, &code, LOCKOUT)
                .await
                .is_ok()
        })
    };
    let (x, y) = (spawn(first), spawn(second));
    [x.await.expect("x"), y.await.expect("y")]
        .iter()
        .filter(|ok| **ok)
        .count()
}

async fn reset_failures(a: &MfaAdmin) {
    exec(
        &a.state,
        format!(
            "UPDATE users SET mfa_failure_count = 0, locked_until = NULL WHERE id = '{}'",
            a.user
        ),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn r102_l02_race_one_totp_code_on_two_pending_rows_gives_one_session() {
    let a = mfa_admin().await;
    for i in 0..ITERATIONS {
        rewind_step(&a).await;
        reset_failures(&a).await;
        let code = code_at(&a.secret, step_now()).await;
        let before = sessions_of(&a.state, a.user).await;
        let won = race(
            &a,
            (issue_pending(&a).await, code.clone()),
            (issue_pending(&a).await, code),
        )
        .await;
        assert_eq!(won, 1, "iteration {i}: exactly one completion");
        assert_eq!(
            sessions_of(&a.state, a.user).await,
            before + 1,
            "iteration {i}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn r102_l02_race_one_recovery_code_on_two_pending_rows_gives_one_session() {
    let a = mfa_admin().await;
    for (i, code) in a.codes.iter().enumerate() {
        reset_failures(&a).await;
        let left = recovery_codes_left(&a.state, a.user).await;
        let before = sessions_of(&a.state, a.user).await;
        let won = race(
            &a,
            (issue_pending(&a).await, code.clone()),
            (issue_pending(&a).await, code.clone()),
        )
        .await;
        assert_eq!(won, 1, "code {i}: exactly one completion");
        assert_eq!(sessions_of(&a.state, a.user).await, before + 1, "code {i}");
        assert_eq!(
            recovery_codes_left(&a.state, a.user).await,
            left - 1,
            "code {i}: exactly one code removed"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn r102_l02_race_two_completions_of_one_pending_row_give_one_session() {
    let a = mfa_admin().await;
    for i in 0..ITERATIONS {
        let pending = issue_pending(&a).await;
        let before = sessions_of(&a.state, a.user).await;
        let spawn = || {
            let state = a.state.clone();
            let user = a.user;
            tokio::spawn(async move {
                sui_id_core::mfa::verify_pending_webauthn(&state.db, &state.clock, pending, user)
                    .await
                    .is_ok()
            })
        };
        let (x, y) = (spawn(), spawn());
        let won = [x.await.expect("x"), y.await.expect("y")]
            .iter()
            .filter(|ok| **ok)
            .count();
        assert_eq!(won, 1, "iteration {i}: exactly one completion");
        assert_eq!(
            sessions_of(&a.state, a.user).await,
            before + 1,
            "iteration {i}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn r102_l02_race_steps_n_plus_one_and_n_leave_n_plus_one() {
    let a = mfa_admin().await;
    for i in 0..ITERATIONS {
        rewind_step(&a).await;
        reset_failures(&a).await;
        let newer = step_now();
        let (newer_code, older_code) = (
            code_at(&a.secret, newer).await,
            code_at(&a.secret, newer - 1).await,
        );
        race(
            &a,
            (issue_pending(&a).await, newer_code),
            (issue_pending(&a).await, older_code),
        )
        .await;
        assert_eq!(
            last_used_step(&a.state, a.user).await,
            newer,
            "iteration {i}: the stored step never moves backwards"
        );
    }
}
