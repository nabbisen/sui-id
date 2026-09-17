//! RFC 102 stage 7 — B4: each step-up-gated sealed command records what
//! authorized it, computed from the acting session inside its own
//! transaction; K01 (signing-key rotation) is the live rotation path and
//! records the administrator; and L05/L06 take `now` from the caller's
//! clock.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::sync::Arc;
use sui_id::{AppState, build_router};
use sui_id_core::errors::CoreError;
use sui_id_core::time::{MockClock, SharedClock};
use sui_id_shared::ids::UserId;
use sui_id_store::StoreError;
use tower::ServiceExt;

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

struct Admin {
    state: AppState,
    id: UserId,
    session: String,
}

async fn admin() -> Admin {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    Admin { state, id, session }
}

/// Give the admin a second factor (a passkey row is enough for the
/// in-transaction check).
async fn give_second_factor(a: &Admin) {
    exec(
        &a.state,
        format!(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at) \
             VALUES ('{}', '{}', X'0102', X'00', 'k', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4(),
            a.id
        ),
    )
    .await;
}

/// Record a step-up `ago_secs` seconds ago on the admin's session.
async fn stepped_up(a: &Admin, method: &'static str, ago_secs: i64) {
    let at = chrono::Utc::now() - chrono::Duration::seconds(ago_secs);
    let session = a.session.clone();
    a.state
        .db
        .with_conn(move |c| {
            c.execute(
                "UPDATE sessions SET last_step_up_at = ?1, last_step_up_method = ?2 WHERE id = ?3",
                rusqlite::params![at, method, session],
            )?;
            Ok(())
        })
        .await
        .expect("step up");
}

async fn target_user(a: &Admin, name: &str) -> UserId {
    sui_id_core::admin::create_user(
        &a.state.db,
        &a.state.clock,
        None,
        sui_id_store::models::HibpMode::Off,
        &admin_actor_for(a.id),
        sui_id_core::admin::CreateUserSpec {
            username: name,
            password: "target-very-strong-password",
            min_password_len: 12,
            display_name: None,
            email: None,
            is_admin: false,
        },
    )
    .await
    .expect("create target")
    .id
}

/// Run all five gated commands through their `sui-id-core` entry points,
/// as the admin on the admin's session, and return each event's note.
async fn run_all_five(a: &Admin) -> Vec<(&'static str, Option<String>, Option<String>)> {
    let actor = admin_actor_on_session(a.id, &a.session);
    let db = &a.state.db;
    let bob = target_user(a, "bob").await;
    let carol = target_user(a, "carol").await;
    let dave = target_user(a, "dave").await;

    sui_id_core::admin::set_user_disabled(db, &actor, bob, true, Some("r".into()))
        .await
        .expect("U02");
    sui_id_core::admin::set_user_disabled(db, &actor, bob, false, None)
        .await
        .expect("U03");
    sui_id_core::admin::delete_user(db, &actor, carol, None)
        .await
        .expect("U04");
    sui_id_core::admin::admin_reset_mfa(db, &actor, dave, None)
        .await
        .expect("U07");
    sui_id_core::admin::rotate_signing_key(
        db,
        &a.state.clock,
        "unused",
        &actor,
        Some("scheduled".into()),
        &a.state.caches,
    )
    .await
    .expect("K01");

    let mut out = Vec::new();
    for action in [
        "user.disable",
        "user.enable",
        "user.delete",
        "mfa.admin_reset",
        "signing_key.rotate",
    ] {
        let note = text(
            &a.state,
            format!(
                "SELECT note FROM audit_log WHERE action = '{action}' ORDER BY seq DESC LIMIT 1"
            ),
        )
        .await;
        let actor = text(
            &a.state,
            format!(
                "SELECT actor FROM audit_log WHERE action = '{action}' ORDER BY seq DESC LIMIT 1"
            ),
        )
        .await;
        out.push((action, note, actor));
    }
    out
}

fn step_up_of(note: &str) -> &str {
    note.split(' ')
        .find_map(|kv| kv.strip_prefix("step_up="))
        .unwrap_or_else(|| panic!("no step_up in {note:?}"))
}

// ── The three forms ──────────────────────────────────────────────────

#[tokio::test]
async fn r102_b4_fresh_step_up_is_recorded_on_every_gated_command() {
    let a = admin().await;
    give_second_factor(&a).await;
    stepped_up(&a, "webauthn", 30).await;
    for (action, note, actor) in run_all_five(&a).await {
        let note = note.unwrap_or_else(|| panic!("{action}: no note"));
        let evidence = step_up_of(&note);
        let age: i64 = evidence
            .strip_prefix("fresh:webauthn:")
            .unwrap_or_else(|| panic!("{action}: {evidence}"))
            .parse()
            .expect("seconds");
        assert!((30..=300).contains(&age), "{action}: age {age}");
        assert_eq!(
            actor.as_deref(),
            Some(a.id.to_string().as_str()),
            "{action}: actor"
        );
    }
}

#[tokio::test]
async fn r102_b4_no_second_factor_is_recorded_as_not_required() {
    let a = admin().await;
    for (action, note, actor) in run_all_five(&a).await {
        let note = note.unwrap_or_else(|| panic!("{action}: no note"));
        assert_eq!(
            step_up_of(&note),
            "not_required:no_second_factor",
            "{action}"
        );
        assert_eq!(
            actor.as_deref(),
            Some(a.id.to_string().as_str()),
            "{action}: actor"
        );
    }
}

#[tokio::test]
async fn r102_b4_not_applicable_only_on_the_cli_reset() {
    let a = admin().await;
    let erin = target_user(&a, "erin").await;
    sui_id_core::admin::operator_reset_mfa(&a.state.db, "erin", "lost everything")
        .await
        .expect("CLI U07");
    assert_eq!(
        text(
            &a.state,
            format!(
                "SELECT note FROM audit_log WHERE action = 'mfa.admin_reset' AND target = '{erin}' \
                 AND actor IS NULL"
            )
        )
        .await
        .as_deref(),
        Some(
            "totp=absent passkeys=0 reason=lost everything via=cli step_up=not_applicable:system_principal"
        )
    );
    // No session-bound event anywhere records the system-principal form.
    run_all_five(&a).await;
    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM audit_log WHERE note LIKE '%step_up=not_applicable%' \
             AND actor IS NOT NULL"
                .into()
        )
        .await,
        0
    );
}

// ── Freshness lapse between the gate and the commit ──────────────────

#[tokio::test]
async fn r102_b4_lapsed_freshness_rolls_back_every_gated_command() {
    let a = admin().await;
    give_second_factor(&a).await;
    // Stepped up 10 minutes ago: past the 5-minute window at commit.
    stepped_up(&a, "totp", 600).await;
    let actor = admin_actor_on_session(a.id, &a.session);
    let db = &a.state.db;
    let bob = target_user(&a, "bob").await;
    let events_before = scalar(&a.state, "SELECT COUNT(*) FROM audit_log".into()).await;
    let keys_before = scalar(&a.state, "SELECT COUNT(*) FROM signing_keys".into()).await;

    let lapsed =
        |r: Result<(), CoreError>| matches!(r, Err(CoreError::Store(StoreError::StepUpRequired)));
    assert!(
        lapsed(sui_id_core::admin::set_user_disabled(db, &actor, bob, true, None).await),
        "U02"
    );
    assert!(
        lapsed(sui_id_core::admin::delete_user(db, &actor, bob, None).await),
        "U04"
    );
    assert!(
        lapsed(
            sui_id_core::admin::admin_reset_mfa(db, &actor, bob, None)
                .await
                .map(|_| ())
        ),
        "U07"
    );
    assert!(
        lapsed(
            sui_id_core::admin::rotate_signing_key(
                db,
                &a.state.clock,
                "unused",
                &actor,
                None,
                &a.state.caches
            )
            .await
            .map(|_| ())
        ),
        "K01"
    );

    let row = sui_id_store::repos::users::get(db, bob).await.expect("bob");
    assert!(!row.is_disabled && !row.is_deleted, "no mutation");
    assert_eq!(
        scalar(&a.state, "SELECT COUNT(*) FROM audit_log".into()).await,
        events_before
    );
    assert_eq!(
        scalar(&a.state, "SELECT COUNT(*) FROM signing_keys".into()).await,
        keys_before
    );
}

#[tokio::test]
async fn r102_b4_lapse_after_the_gate_redirects_to_step_up() {
    // The HTTP gate reads the application clock; the command re-reads the
    // session against the time of its own transaction. Holding the
    // application clock back reproduces a step-up that was fresh at the
    // gate and lapsed by the commit.
    let mut a = admin().await;
    give_second_factor(&a).await;
    stepped_up(&a, "totp", 600).await;
    let bob = target_user(&a, "bob").await;
    let at_the_gate = chrono::Utc::now() - chrono::Duration::seconds(420);
    a.state.clock = Arc::new(MockClock::at(at_the_gate)) as SharedClock;

    let csrf = fetch_csrf(&a.state, &a.session).await;
    let resp = build_router(a.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/admin/users/{bob}/disabled"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={}; sui_id_csrf={csrf}", a.session),
                )
                .body(Body::from(format!(
                    "_csrf={csrf}&_confirmed=1&disabled=true"
                )))
                .expect("req"),
        )
        .await
        .expect("disable");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/me/security/step-up?return_to=%2Fadmin%2Fusers"),
        "the step-up redirect, as the gate gives"
    );
    let row = sui_id_store::repos::users::get(&a.state.db, bob)
        .await
        .expect("bob");
    assert!(!row.is_disabled, "no mutation");
    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'user.disable'".into()
        )
        .await,
        0,
        "no event"
    );
}

// ── The stage 6 clock follow-up ──────────────────────────────────────

#[tokio::test]
async fn r102_l05_l06_take_now_from_the_callers_clock() {
    use super::r102_stage1::totp_user;
    let state = test_app();
    let (session, secret) = totp_user(&state).await;
    let session_id: sui_id_shared::ids::SessionId = session.parse().expect("id");
    let user = sui_id_store::repos::sessions::get(&state.db, session_id)
        .await
        .expect("session")
        .user_id;

    // A mocked clock ten minutes back: L05 records exactly that time.
    let mocked = chrono::Utc::now() - chrono::Duration::minutes(10);
    let clock: SharedClock = Arc::new(MockClock::at(mocked));
    let step = mocked.timestamp() / 30;
    exec(
        &state,
        format!(
            "UPDATE user_totp SET last_used_step = {} WHERE user_id = '{user}'",
            step - 2
        ),
    )
    .await;
    let code = format!(
        "{:06}",
        sui_id_core::totp::code_for_step(&secret, step).await
    );
    sui_id_core::step_up::verify_totp_code(
        &state.db,
        &clock,
        user,
        session_id,
        &code,
        "/me/security/mfa",
    )
    .await
    .expect("L05 at the mocked time");
    let row = sui_id_store::repos::sessions::get(&state.db, session_id)
        .await
        .expect("session");
    assert_eq!(
        row.last_step_up_at,
        Some(mocked),
        "last_step_up_at is the mocked now"
    );

    // The expiry check uses the same now: a session that expired in real
    // time is still live at an earlier mocked instant, and refused at a
    // later one.
    exec(
        &state,
        format!(
            "UPDATE user_totp SET last_used_step = {} WHERE user_id = '{user}'",
            step - 2
        ),
    )
    .await;
    let expires = chrono::Utc::now() - chrono::Duration::minutes(5);
    state
        .db
        .with_conn(move |c| {
            c.execute(
                "UPDATE sessions SET expires_at = ?1 WHERE id = ?2",
                rusqlite::params![expires, session_id.to_string()],
            )?;
            Ok(())
        })
        .await
        .expect("expire");
    let live = sui_id_store::commands::complete_step_up(
        &state.db,
        user,
        session_id,
        sui_id_store::commands::StepUpProof::Totp { step },
        "/me/security/mfa".into(),
        mocked,
    )
    .await;
    assert!(live.is_ok(), "live at the mocked time: {:?}", live.err());
    let refused = sui_id_store::commands::complete_step_up(
        &state.db,
        user,
        session_id,
        sui_id_store::commands::StepUpProof::Totp { step: step + 1 },
        "/me/security/mfa".into(),
        chrono::Utc::now(),
    )
    .await;
    assert!(
        matches!(refused, Err(StoreError::NotFound)),
        "expired at the real time"
    );

    // L06: the revocation at the threshold is stamped with the mocked now.
    state
        .db
        .with_conn(move |c| {
            c.execute(
                "UPDATE sessions SET expires_at = ?1, step_up_failure_count = 4 WHERE id = ?2",
                rusqlite::params![
                    chrono::Utc::now() + chrono::Duration::hours(1),
                    session_id.to_string()
                ],
            )?;
            Ok(())
        })
        .await
        .expect("reset");
    let outcome = sui_id_core::step_up::record_step_up_failure(&state.db, &clock, user, session_id)
        .await
        .expect("L06");
    assert!(outcome.session_revoked);
    let row = sui_id_store::repos::sessions::get(&state.db, session_id)
        .await
        .expect("session");
    assert_eq!(row.revoked_at, Some(mocked), "revoked_at is the mocked now");
}
