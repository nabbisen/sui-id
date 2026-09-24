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
    create_user_with_password(
        &a.state.db,
        &a.state.clock,
        &admin_actor_for(a.id),
        sui_id_core::admin::CreateUserSpec {
            username: name,
            display_name: None,
            email: None,
            is_admin: false,
        },
        "target-very-strong-password",
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

    sui_id_core::admin::set_user_disabled(db, &a.state.clock, &actor, bob, true, Some("r".into()))
        .await
        .expect("U02");
    sui_id_core::admin::set_user_disabled(db, &a.state.clock, &actor, bob, false, None)
        .await
        .expect("U03");
    sui_id_core::admin::delete_user(db, &a.state.clock, &actor, carol, None)
        .await
        .expect("U04");
    sui_id_core::admin::admin_reset_mfa(db, &a.state.clock, &actor, dave, None)
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
        lapsed(
            sui_id_core::admin::set_user_disabled(db, &a.state.clock, &actor, bob, true, None)
                .await
        ),
        "U02"
    );
    assert!(
        lapsed(sui_id_core::admin::delete_user(db, &a.state.clock, &actor, bob, None).await),
        "U04"
    );
    assert!(
        lapsed(
            sui_id_core::admin::admin_reset_mfa(db, &a.state.clock, &actor, bob, None)
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

/// A clock that reads `base` for its first `hold` reads and `base + jump`
/// for every read after that. With `hold` left at `usize::MAX` it never
/// jumps, and it just counts reads. RFC 102 stage 8: the gate and the
/// command read the same application clock, so a lapse between them needs a
/// clock that moves between two of its own reads.
pub(super) struct SteppingClock {
    base: chrono::DateTime<chrono::Utc>,
    jump: chrono::Duration,
    hold: std::sync::atomic::AtomicUsize,
    reads: std::sync::atomic::AtomicUsize,
}

impl SteppingClock {
    pub(super) fn new(base: chrono::DateTime<chrono::Utc>, jump: chrono::Duration) -> Arc<Self> {
        Arc::new(Self {
            base,
            jump,
            hold: std::sync::atomic::AtomicUsize::new(usize::MAX),
            reads: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// Restart the read count and jump after `hold` more reads.
    pub(super) fn arm(&self, hold: usize) {
        use std::sync::atomic::Ordering::SeqCst;
        self.reads.store(0, SeqCst);
        self.hold.store(hold, SeqCst);
    }

    pub(super) fn reads(&self) -> usize {
        self.reads.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl sui_id_core::time::Clock for SteppingClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        use std::sync::atomic::Ordering::SeqCst;
        let n = self.reads.fetch_add(1, SeqCst) + 1;
        if n <= self.hold.load(SeqCst) {
            self.base
        } else {
            self.base + self.jump
        }
    }
}

/// POST the disable of `bob`, with the admin's session, through the router.
async fn post_disable(a: &Admin, bob: UserId) -> axum::http::Response<Body> {
    let csrf = fetch_csrf(&a.state, &a.session).await;
    build_router(a.state.clone())
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
        .expect("disable")
}

const STEP_UP_REDIRECT: &str = "/me/security/step-up?return_to=%2Fadmin%2Fusers";

fn location(resp: &axum::http::Response<Body>) -> Option<&str> {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
}

/// An admin with a factor whose application clock is `clock`, and a target
/// user, with the step-up recorded `age` before `base`.
async fn admin_on_clock(
    clock: &Arc<SteppingClock>,
    base: chrono::DateTime<chrono::Utc>,
    age_secs: i64,
) -> (Admin, UserId) {
    let mut a = admin().await;
    give_second_factor(&a).await;
    let at = base - chrono::Duration::seconds(age_secs);
    let session = a.session.clone();
    a.state
        .db
        .with_conn(move |c| {
            c.execute(
                "UPDATE sessions SET last_step_up_at = ?1, last_step_up_method = 'totp' WHERE id = ?2",
                rusqlite::params![at, session],
            )?;
            Ok(())
        })
        .await
        .expect("step up");
    let bob = target_user(&a, "bob").await;
    a.state.clock = clock.clone() as SharedClock;
    (a, bob)
}

async fn disabled_and_events(a: &Admin, bob: UserId) -> (bool, i64) {
    let row = sui_id_store::repos::users::get(&a.state.db, bob)
        .await
        .expect("bob");
    let events = scalar(
        &a.state,
        "SELECT COUNT(*) FROM audit_log WHERE action = 'user.disable'".into(),
    )
    .await;
    (row.is_disabled, events)
}

#[tokio::test]
async fn r102_b4_lapse_after_the_gate_redirects_to_step_up() {
    // A stepping clock (not two mocked instants): the HTTP gate and the
    // command read the same application clock, so the lapse is produced by a
    // clock that holds still through the gate's read and moves before the
    // command's. Where the gate's read falls is measured, not assumed.
    let base = chrono::Utc::now();
    let jump = chrono::Duration::seconds(120);

    // Calibration: with a step-up already stale (10 min), the gate itself
    // refuses, so the reads counted in the POST are exactly those up to and
    // including the gate's.
    let clock = SteppingClock::new(base, jump);
    let (a, bob) = admin_on_clock(&clock, base, 600).await;
    clock.arm(usize::MAX);
    let resp = post_disable(&a, bob).await;
    assert_eq!(location(&resp), Some(STEP_UP_REDIRECT), "the gate refuses");
    let through_the_gate = clock.reads();
    assert!(through_the_gate >= 1, "the gate reads the clock");

    // The run: a step-up 4 minutes old (fresh at `base`, inside the 5-minute
    // window). The clock holds through the gate's read and jumps 2 minutes
    // after it, so the command finds the step-up 6 minutes old.
    let clock = SteppingClock::new(base, jump);
    let (a, bob) = admin_on_clock(&clock, base, 240).await;
    clock.arm(through_the_gate);
    let resp = post_disable(&a, bob).await;
    assert!(
        clock.reads() > through_the_gate,
        "the request passed the gate and read the clock again ({} reads, gate at {through_the_gate})",
        clock.reads()
    );
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        location(&resp),
        Some(STEP_UP_REDIRECT),
        "the command's rollback becomes the step-up redirect"
    );
    assert_eq!(
        disabled_and_events(&a, bob).await,
        (false, 0),
        "no mutation and no event"
    );

    // Control: the same request with a clock that never jumps commits, and
    // records the age as the caller's clock saw it.
    let clock = SteppingClock::new(base, jump);
    let (a, bob) = admin_on_clock(&clock, base, 240).await;
    clock.arm(usize::MAX);
    let resp = post_disable(&a, bob).await;
    assert_eq!(
        location(&resp),
        Some("/admin/users"),
        "commits: {}",
        resp.status()
    );
    assert_eq!(disabled_and_events(&a, bob).await, (true, 1));
    assert_eq!(
        text(
            &a.state,
            "SELECT note FROM audit_log WHERE action = 'user.disable'".into()
        )
        .await
        .as_deref(),
        Some("step_up=fresh:totp:240")
    );
}

#[tokio::test]
async fn r102_stage8_gated_commands_judge_freshness_at_the_callers_now() {
    // A clock one hour behind real time. By that clock the step-up (4
    // minutes before it) is fresh; by real time it is over an hour old. So a
    // command that read `Utc::now()` would roll back, and one that takes the
    // caller's `now` commits and records the age the caller's clock saw.
    let a = admin().await;
    give_second_factor(&a).await;
    let now = chrono::Utc::now() - chrono::Duration::hours(1);
    let clock: SharedClock = Arc::new(MockClock::at(now));
    let at = now - chrono::Duration::seconds(240);
    let session = a.session.clone();
    a.state
        .db
        .with_conn(move |c| {
            c.execute(
                "UPDATE sessions SET last_step_up_at = ?1, last_step_up_method = 'totp' WHERE id = ?2",
                rusqlite::params![at, session],
            )?;
            Ok(())
        })
        .await
        .expect("step up");
    let actor = admin_actor_on_session(a.id, &a.session);
    let db = &a.state.db;
    let bob = target_user(&a, "bob").await;
    let carol = target_user(&a, "carol").await;
    let dave = target_user(&a, "dave").await;

    sui_id_core::admin::set_user_disabled(db, &clock, &actor, bob, true, None)
        .await
        .expect("U02");
    sui_id_core::admin::set_user_disabled(db, &clock, &actor, bob, false, None)
        .await
        .expect("U03");
    sui_id_core::admin::delete_user(db, &clock, &actor, carol, None)
        .await
        .expect("U04");
    sui_id_core::admin::admin_reset_mfa(db, &clock, &actor, dave, None)
        .await
        .expect("U07");
    sui_id_core::admin::rotate_signing_key(db, &clock, "unused", &actor, None, &a.state.caches)
        .await
        .expect("K01");

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
        .await
        .unwrap_or_else(|| panic!("{action}: no note"));
        assert_eq!(step_up_of(&note), "fresh:totp:240", "{action}");
    }
    // K01's timestamps are the caller's too.
    let active = sui_id_store::repos::signing_keys::active(db)
        .await
        .expect("active key");
    assert_eq!(active.created_at, now, "the new key's created_at");
    let retired: Vec<chrono::DateTime<chrono::Utc>> = a
        .state
        .db
        .with_conn(|c| {
            let mut stmt = c.prepare("SELECT rotated_at FROM signing_keys WHERE is_active = 0")?;
            let rows = stmt
                .query_map([], |r| r.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .expect("retired keys");
    assert_eq!(retired, vec![now], "the retired key's rotated_at");
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
