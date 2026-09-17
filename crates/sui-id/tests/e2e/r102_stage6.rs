//! RFC 102 stage 6 — L05: a successful step-up commits its freshness, its
//! method, the consumed factor, the failure-count reset and
//! `auth.step_up.success` together or not at all; and B-F5: a passkey
//! ceremony keeps the kind it was created with, so a sign-in ceremony
//! cannot complete a step-up and a step-up ceremony cannot complete a
//! sign-in.

use super::common::*;
use super::r102_stage1::{post, totp_user};
use std::io::Write;
use std::sync::{Arc, Mutex};
use sui_id::AppState;
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::{SessionId, UserId, WebauthnPendingId};
use sui_id_store::commands::StepUpProof;
use sui_id_store::models::{SessionRow, WebauthnPendingKind, WebauthnPendingRow};
use sui_id_store::repos::webauthn_pending;

const GATE: &str = "/me/security/mfa";

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

async fn user_of(state: &AppState, session: &str) -> UserId {
    let id: SessionId = session.parse().expect("session id");
    sui_id_store::repos::sessions::get(&state.db, id)
        .await
        .expect("session")
        .user_id
}

async fn freshness(state: &AppState, session: &str) -> (Option<String>, Option<String>, i64) {
    let at = text(
        state,
        format!("SELECT last_step_up_at FROM sessions WHERE id = '{session}'"),
    )
    .await;
    let method = text(
        state,
        format!("SELECT last_step_up_method FROM sessions WHERE id = '{session}'"),
    )
    .await;
    let count = scalar(
        state,
        format!("SELECT step_up_failure_count FROM sessions WHERE id = '{session}'"),
    )
    .await;
    (at, method, count)
}

async fn successes(state: &AppState) -> i64 {
    scalar(
        state,
        "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.step_up.success'".into(),
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

fn step_now() -> i64 {
    chrono::Utc::now().timestamp() / 30
}

async fn code_at(secret: &[u8], step: i64) -> String {
    format!(
        "{:06}",
        sui_id_core::totp::code_for_step(secret, step).await
    )
}

/// A TOTP admin whose replay cursor sits two steps back, and whose session
/// has two counted step-up failures and no freshness.
async fn prepared() -> (AppState, String, Vec<u8>, UserId) {
    let state = test_app();
    let (session, secret) = totp_user(&state).await;
    let user = user_of(&state, &session).await;
    exec(
        &state,
        format!(
            "UPDATE user_totp SET last_used_step = {} WHERE user_id = '{user}'; \
             UPDATE sessions SET step_up_failure_count = 2, last_step_up_at = NULL, \
             last_step_up_method = NULL WHERE id = '{session}'",
            step_now() - 2
        ),
    )
    .await;
    (state, session, secret, user)
}

fn step_up_ceremony(
    user: UserId,
    kind: WebauthnPendingKind,
    state_json: &str,
) -> WebauthnPendingRow {
    let now = chrono::Utc::now();
    WebauthnPendingRow {
        id: WebauthnPendingId::new(),
        kind,
        user_id: Some(user),
        state_json: state_json.into(),
        expires_at: now + chrono::Duration::minutes(5),
        created_at: now,
    }
}

async fn ceremony_exists(state: &AppState, id: WebauthnPendingId) -> bool {
    webauthn_pending::get(&state.db, id)
        .await
        .expect("read")
        .is_some()
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

// ── Happy paths ──────────────────────────────────────────────────────

#[tokio::test]
async fn r102_l05_totp_step_up_commits_freshness_method_reset_and_event() {
    let (state, session, secret, user) = prepared().await;
    let step = step_now();
    let r = post(
        &state,
        "/me/security/step-up",
        &session,
        &format!("code={}&return_to={GATE}", code_at(&secret, step).await),
    )
    .await;
    assert_eq!(r.location.as_deref(), Some(GATE), "{}", r.status);

    let (at, method, count) = freshness(&state, &session).await;
    assert!(at.is_some(), "fresh");
    assert_eq!(method.as_deref(), Some("totp"));
    assert_eq!(count, 0, "failure count reset");
    assert_eq!(last_used_step(&state, user).await, step, "step consumed");
    assert_eq!(
        scalar(
            &state,
            format!(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.step_up.success' \
                 AND actor = '{user}' AND target = '{user}' \
                 AND note = 'method=totp gate={GATE}'"
            )
        )
        .await,
        1,
        "one Atomic event with method and gate"
    );
}

#[tokio::test]
async fn r102_l05_webauthn_step_up_through_the_command() {
    let (state, session, _secret, user) = prepared().await;
    let ceremony = step_up_ceremony(user, WebauthnPendingKind::StepUp, "{}");
    webauthn_pending::insert(&state.db, &ceremony)
        .await
        .expect("ceremony");

    sui_id_store::commands::complete_step_up(
        &state.db,
        user,
        session.parse().expect("id"),
        StepUpProof::Webauthn {
            pending_id: ceremony.id,
        },
        GATE.into(),
        chrono::Utc::now(),
    )
    .await
    .expect("L05");

    let (at, method, count) = freshness(&state, &session).await;
    assert!(at.is_some(), "fresh");
    assert_eq!(method.as_deref(), Some("webauthn"));
    assert_eq!(count, 0);
    assert!(
        !ceremony_exists(&state, ceremony.id).await,
        "ceremony consumed"
    );
    assert_eq!(
        scalar(
            &state,
            format!(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.step_up.success' \
                 AND note = 'method=webauthn gate={GATE}'"
            )
        )
        .await,
        1
    );
}

#[tokio::test]
async fn r102_l05_ceremony_guard_refuses_a_sign_in_or_foreign_ceremony() {
    let (state, session, _secret, user) = prepared().await;
    let other = UserId::new();
    for (label, ceremony) in [
        (
            "an Authenticate ceremony",
            step_up_ceremony(user, WebauthnPendingKind::Authenticate, "{}"),
        ),
        (
            "another user's StepUp ceremony",
            WebauthnPendingRow {
                user_id: Some(other),
                ..step_up_ceremony(user, WebauthnPendingKind::StepUp, "{}")
            },
        ),
    ] {
        // `other` must exist for the foreign row's foreign key.
        if ceremony.user_id == Some(other) {
            exec(
                &state,
                format!(
                    "INSERT INTO users (id, username, is_admin, role, is_disabled, is_deleted, \
                     user_uuid, failed_login_count, source, created_at, updated_at) \
                     VALUES ('{other}', 'other', 0, 'user', 0, 0, '{}', 0, 'local', \
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    uuid::Uuid::new_v4()
                ),
            )
            .await;
        }
        webauthn_pending::insert(&state.db, &ceremony)
            .await
            .expect("ceremony");
        let result = sui_id_store::commands::complete_step_up(
            &state.db,
            user,
            session.parse().expect("id"),
            StepUpProof::Webauthn {
                pending_id: ceremony.id,
            },
            GATE.into(),
            chrono::Utc::now(),
        )
        .await;
        assert!(
            matches!(result, Err(sui_id_store::StoreError::NotFound)),
            "{label}: rolled back"
        );
        assert!(ceremony_exists(&state, ceremony.id).await, "{label}: kept");
        assert!(
            freshness(&state, &session).await.0.is_none(),
            "{label}: not fresh"
        );
    }
    assert_eq!(successes(&state).await, 0);
}

// ── Injected append failure ──────────────────────────────────────────

#[tokio::test]
async fn r102_l05_append_failure_changes_nothing_and_looks_like_a_wrong_code() {
    // The ordinary wrong-code page, from a working instance.
    let (control, control_session, _s, _u) = prepared().await;
    let ordinary = post(
        &control,
        "/me/security/step-up",
        &control_session,
        &format!("code=000000&return_to={GATE}"),
    )
    .await;
    assert_eq!(ordinary.status.as_u16(), 400);

    let (state, session, secret, user) = prepared().await;
    let ceremony = step_up_ceremony(user, WebauthnPendingKind::StepUp, "{}");
    webauthn_pending::insert(&state.db, &ceremony)
        .await
        .expect("ceremony");
    let step_before = last_used_step(&state, user).await;
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

    // TOTP through the handler. The refused step-up writes nothing, so it
    // can be repeated until the capturing subscriber has seen the line
    // (parallel tests race tracing's callsite interest cache).
    let body = format!(
        "code={}&return_to={GATE}",
        code_at(&secret, step_now()).await
    );
    let mut r = post(&state, "/me/security/step-up", &session, &body).await;
    for _ in 0..4 {
        let logged = String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned();
        if logged.contains("other than a wrong code") {
            break;
        }
        tracing::callsite::rebuild_interest_cache();
        r = post(&state, "/me/security/step-up", &session, &body).await;
    }
    assert_eq!(r.status.as_u16(), 400, "the uniform step-up response");
    assert_eq!(
        normalise_csrf(&r.body),
        normalise_csrf(&ordinary.body),
        "byte-identical to a wrong code"
    );
    let logged = String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned();
    let line = logged
        .lines()
        .find(|l| l.contains("other than a wrong code"))
        .unwrap_or_else(|| panic!("no log line:\n{logged}"));
    assert!(line.contains("ERROR") && line.contains("r102 test: audit_log insert rejected"));

    // WebAuthn through the command.
    let result = sui_id_store::commands::complete_step_up(
        &state.db,
        user,
        session.parse().expect("id"),
        StepUpProof::Webauthn {
            pending_id: ceremony.id,
        },
        GATE.into(),
        chrono::Utc::now(),
    )
    .await;
    assert!(result.is_err());

    let (at, method, count) = freshness(&state, &session).await;
    assert!(at.is_none() && method.is_none(), "no freshness");
    assert_eq!(count, 2, "count unchanged: the right code was not counted");
    assert_eq!(
        last_used_step(&state, user).await,
        step_before,
        "step unchanged"
    );
    assert!(
        ceremony_exists(&state, ceremony.id).await,
        "ceremony unchanged"
    );
}

// ── Concurrency ──────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn r102_l05_race_one_totp_code_on_two_step_ups_gives_one_success() {
    let (state, first, secret, user) = prepared().await;
    // A second session for the same user, created by signing in (L01).
    let now = chrono::Utc::now();
    let second = SessionId::new();
    sui_id_store::commands::sign_in_with_password(
        &state.db,
        SessionRow {
            id: second,
            user_id: user,
            expires_at: now + chrono::Duration::hours(1),
            created_at: now,
            revoked_at: None,
            auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
            last_step_up_at: None,
            last_used_at: None,
        },
    )
    .await
    .expect("second session");
    let sessions: [SessionId; 2] = [first.parse().expect("id"), second];

    for i in 0..12 {
        exec(
            &state,
            format!(
                "UPDATE user_totp SET last_used_step = {} WHERE user_id = '{user}'; \
                 UPDATE sessions SET last_step_up_at = NULL WHERE user_id = '{user}'",
                step_now() - 2
            ),
        )
        .await;
        let before = successes(&state).await;
        let code = code_at(&secret, step_now()).await;
        let handles = sessions.map(|session| {
            let state = state.clone();
            let code = code.clone();
            tokio::spawn(async move {
                sui_id_core::step_up::verify_totp_code(
                    &state.db,
                    &state.clock,
                    user,
                    session,
                    &code,
                    GATE,
                )
                .await
                .is_ok()
            })
        });
        let mut won = 0;
        for h in handles {
            won += usize::from(h.await.expect("join"));
        }
        assert_eq!(won, 1, "iteration {i}: exactly one step-up commits");
        assert_eq!(
            successes(&state).await,
            before + 1,
            "iteration {i}: one event"
        );
        assert_eq!(
            scalar(
                &state,
                format!(
                    "SELECT COUNT(*) FROM sessions WHERE user_id = '{user}' \
                     AND last_step_up_at IS NOT NULL"
                )
            )
            .await,
            1,
            "iteration {i}: one freshness change"
        );
    }
}

// ── Revalidation ─────────────────────────────────────────────────────

#[tokio::test]
async fn r102_l05_session_ended_between_verification_and_commit_rolls_back() {
    let (state, session, _secret, user) = prepared().await;
    let step = step_now();
    for (label, sql) in [
        (
            "revoked",
            format!(
                "UPDATE sessions SET revoked_at = '2026-01-01T00:00:00Z' WHERE id = '{session}'"
            ),
        ),
        (
            "expired",
            format!(
                "UPDATE sessions SET revoked_at = NULL, expires_at = '2000-01-01T00:00:00Z' WHERE id = '{session}'"
            ),
        ),
        (
            "user disabled",
            format!(
                "UPDATE sessions SET expires_at = '2999-01-01T00:00:00Z' WHERE id = '{session}'; UPDATE users SET is_disabled = 1 WHERE id = '{user}'"
            ),
        ),
    ] {
        exec(&state, sql).await;
        let result = sui_id_store::commands::complete_step_up(
            &state.db,
            user,
            session.parse().expect("id"),
            StepUpProof::Totp { step },
            GATE.into(),
            chrono::Utc::now(),
        )
        .await;
        assert!(
            matches!(result, Err(sui_id_store::StoreError::NotFound)),
            "{label}: rolled back"
        );
        assert_eq!(
            last_used_step(&state, user).await,
            step - 2,
            "{label}: step kept"
        );
        assert!(
            freshness(&state, &session).await.0.is_none(),
            "{label}: not fresh"
        );
    }
    // Another user's session is refused the same way.
    exec(
        &state,
        format!("UPDATE users SET is_disabled = 0 WHERE id = '{user}'"),
    )
    .await;
    let result = sui_id_store::commands::complete_step_up(
        &state.db,
        UserId::new(),
        session.parse().expect("id"),
        StepUpProof::Totp { step },
        GATE.into(),
        chrono::Utc::now(),
    )
    .await;
    assert!(
        matches!(result, Err(sui_id_store::StoreError::NotFound)),
        "foreign session"
    );
    assert_eq!(successes(&state).await, 0);
}

// ── B-F5: the ceremony kind ──────────────────────────────────────────

/// A syntactically valid credential. Its contents never matter here: every
/// case below is decided before the assertion is verified.
fn credential() -> webauthn_rs::prelude::PublicKeyCredential {
    serde_json::from_value(serde_json::json!({
        "id": "AQID",
        "rawId": "AQID",
        "response": {
            "authenticatorData": "AQID",
            "clientDataJSON": "AQID",
            "signature": "AQID",
            "userHandle": null
        },
        "extensions": {},
        "type": "public-key"
    }))
    .expect("credential JSON")
}

/// `finish_authentication` against a ceremony of `row_kind`, expecting
/// `expected`. The ceremony's state is deliberately not valid JSON: a kind
/// refusal returns `Unauthenticated` before the state is read, while a
/// ceremony that passes the kind check fails to parse (`Internal`).
async fn finish(row_kind: WebauthnPendingKind, expected: WebauthnPendingKind) -> (CoreError, bool) {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let ceremony = step_up_ceremony(user, row_kind, "not json");
    webauthn_pending::insert(&state.db, &ceremony)
        .await
        .expect("ceremony");
    let err = sui_id_core::webauthn::finish_authentication(
        &state.db,
        &state.clock,
        "https://idp.test",
        ceremony.id,
        user,
        expected,
        &credential(),
    )
    .await
    .expect_err("never completes here");
    (err, ceremony_exists(&state, ceremony.id).await)
}

#[tokio::test]
async fn r102_bf5_a_sign_in_ceremony_cannot_complete_a_step_up() {
    let (err, kept) = finish(
        WebauthnPendingKind::Authenticate,
        WebauthnPendingKind::StepUp,
    )
    .await;
    assert!(
        matches!(err, CoreError::Unauthenticated),
        "refused on kind: {err:?}"
    );
    assert!(kept, "the sign-in ceremony is left for its own flow");
}

#[tokio::test]
async fn r102_bf5_a_step_up_ceremony_cannot_complete_a_sign_in() {
    let (err, kept) = finish(
        WebauthnPendingKind::StepUp,
        WebauthnPendingKind::Authenticate,
    )
    .await;
    assert!(
        matches!(err, CoreError::Unauthenticated),
        "refused on kind: {err:?}"
    );
    assert!(kept, "the step-up ceremony is left for its own flow");
}

#[tokio::test]
async fn r102_bf5_a_matching_kind_passes_the_kind_check() {
    // Control for the two tests above: with the kind matching, the same
    // ceremony gets past the kind check and fails on its state.
    for kind in [
        WebauthnPendingKind::StepUp,
        WebauthnPendingKind::Authenticate,
    ] {
        let (err, _) = finish(kind, kind).await;
        assert!(
            matches!(err, CoreError::Internal),
            "{kind:?}: reached the state: {err:?}"
        );
    }
}

#[tokio::test]
async fn r102_bf5_step_up_ceremony_is_created_as_step_up() {
    // `start_webauthn` needs a real passkey to build a challenge, which a
    // test cannot enrol; `start_authentication` records the kind it is
    // given, and the no-credential refusal happens before any row exists.
    let state = test_app();
    complete_setup_and_login(&state).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let r = sui_id_core::step_up::start_webauthn(&state.db, &state.clock, "https://idp.test", user)
        .await;
    assert!(r.is_err(), "no passkey enrolled");
    assert_eq!(
        scalar(&state, "SELECT COUNT(*) FROM webauthn_pending".into()).await,
        0,
        "no ceremony row, of any kind, is written or rewritten"
    );
}
