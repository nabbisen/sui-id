//! RFC 124 D4 — a deterministic, structural defence of D1.
//!
//! Not a timing comparison between branches: RFC 124's own stage 1
//! measurement (`rfcs/handoffs/124-the-uniform-response-is-uniform/d1b-measurement.md`)
//! found branches that differ by as little as a few microseconds, which a
//! wall-clock test cannot assert on without becoming flaky. Instead, this
//! test makes the mailer block on an internal gate that only the test
//! itself can open, and proves two things with no timing involved:
//!
//! 1. The HTTP response returns while the gate is still closed — bounded
//!    by an absolute ceiling (50ms) chosen to be far above real
//!    request-handling cost (stage 1 measured the costliest real branch at
//!    well under 1ms in-process) and to make a regression fail fast rather
//!    than hang. If `/forgot-password`'s handler ever went back to
//!    awaiting `request_reset` before responding, this call would block on
//!    the still-closed gate and the timeout would fire.
//! 2. The gated work completes anyway, once the test releases it — so the
//!    hand-off is real work, not a silently dropped one.
//!
//! **Flakiness budget: zero.** The gate opens only when this test calls
//! `release()`, so there is no race to tune a threshold against — the
//! response either returns near-instantly (correct) or hits the 50ms
//! ceiling (regression), and the only number here that is a genuine
//! headroom choice, not a tuned one, is that ceiling itself.

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use tower::ServiceExt;

use sui_id::{AppState, build_router};
use sui_id_core::errors::CoreResult;
use sui_id_core::mail::{MailSendOutcome, MailSender, OutgoingMail};
use sui_id_shared::ids::UserId;
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{CredentialRow, Role, UserRow, UserSource};
use sui_id_store::repos::{credentials, users};

use super::common::*;

/// Blocks every `send` on an internal gate that only `release()` opens.
/// Lets a test prove, deterministically, whether a caller waited for a
/// mail dispatch to complete — no timing involved, no race to lose.
struct GatedMailSender {
    notify: tokio::sync::Notify,
    sent: tokio::sync::Mutex<Vec<OutgoingMail>>,
}

impl GatedMailSender {
    fn new() -> Self {
        Self {
            notify: tokio::sync::Notify::new(),
            sent: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    fn release(&self) {
        self.notify.notify_one();
    }

    async fn count(&self) -> usize {
        self.sent.lock().await.len()
    }
}

impl MailSender for GatedMailSender {
    fn send<'a>(
        &'a self,
        mail: OutgoingMail,
    ) -> Pin<Box<dyn std::future::Future<Output = CoreResult<MailSendOutcome>> + Send + 'a>> {
        Box::pin(async move {
            // Blocks here until the test calls `release()`. Nothing in
            // this file ever calls it before the test has already
            // received and checked the HTTP response.
            self.notify.notified().await;
            let outcome = MailSendOutcome {
                from: "test@sui-id.test".into(),
                to: mail.to.clone(),
                subject: mail.subject.clone(),
            };
            self.sent.lock().await.push(mail);
            Ok(outcome)
        })
    }
}

/// A Local, credentialed, active, under-the-cap account — RFC 124 stage
/// 1's branch 6, the one that actually reaches `MailSender::send`, and the
/// costliest of the six. If the response waited on any branch's work, it
/// would wait on this one: `send` is its last step.
async fn seed_sendable_user(state: &AppState, username: &str, email: &str) -> UserId {
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
            email: Some(email.into()),
            preferred_lang: None,
            email_normalized: Some(sui_id_shared::normalize_email(email)),
            email_verified_at: None,
        },
    )
    .await
    .expect("create user");
    credentials::upsert(
        &state.db,
        &CredentialRow {
            user_id: uid,
            password_hash: sui_id_core::password::hash_password(
                "irrelevant-password-for-this-test",
            )
            .await
            .expect("hash"),
            updated_at: now,
        },
    )
    .await
    .expect("credentials");
    uid
}

#[tokio::test]
async fn rfc124_d1_the_response_does_not_wait_for_a_real_send() {
    // Built via `app_over_db`, not `test_app()`: the latter auto-spawns its
    // own `ForgotPasswordWorker` bound to its own `InMemoryMailSender`, and
    // this test needs the *only* worker in play to hold the gated one.
    let (base_state, _unused_mailer) =
        app_over_db(Database::open_in_memory(MasterKey::generate()).expect("db"));
    let gated = Arc::new(GatedMailSender::new());
    let state = AppState {
        mailer: gated.clone() as Arc<dyn MailSender>,
        ..base_state
    };
    spawn_forgot_password_worker(&state);
    enable_smtp(&state).await;
    seed_sendable_user(&state, "gated-target", "gated-target@example.test").await;

    let get_resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET forgot-password");
    let csrf = extract_set_cookie(get_resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!(
        "_csrf={csrf}&email={}",
        urlencode("gated-target@example.test")
    );
    let post = Request::builder()
        .method(Method::POST)
        .uri("/forgot-password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
        .body(Body::from(body))
        .expect("req");

    // The gate is still closed. If the handler awaited `request_reset`
    // (and therefore the gated `send`) before responding, this would hang
    // past the timeout, deterministically — nothing releases the gate
    // before this call returns.
    let resp = tokio::time::timeout(
        Duration::from_millis(50),
        build_router(state.clone()).oneshot(post),
    )
    .await
    .expect("RFC 124 D1 regression: the response waited on the gated send")
    .expect("POST forgot-password");
    assert_eq!(resp.status(), StatusCode::OK);

    // The gate is still closed at this point by construction (`release`
    // has not been called yet), so nothing could have reached the mailer's
    // push, regardless of scheduling.
    assert_eq!(
        gated.count().await,
        0,
        "the gated send cannot have completed before this test released it"
    );

    // Release, and confirm the detached work is real — it completes, it
    // isn't silently dropped.
    gated.release();
    wait_until(
        Duration::from_secs(2),
        "gated mail, after release",
        || async { gated.count().await == 1 },
    )
    .await;
}

// ---------- D5: the response is identical across all six branches ----------

async fn seed_user(
    state: &AppState,
    username: &str,
    email: &str,
    source: UserSource,
    password: Option<&str>,
    disabled: bool,
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
            is_disabled: disabled,
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
                password_hash: sui_id_core::password::hash_password(password)
                    .await
                    .expect("hash"),
                updated_at: now,
            },
        )
        .await
        .expect("credentials");
    }
    uid
}

async fn insert_active_token(state: &AppState, user_id: UserId, salt: u8) {
    let now = chrono::Utc::now();
    sui_id_store::repos::password_reset_tokens::insert(
        &state.db,
        &sui_id_store::models::PasswordResetTokenRow {
            id: sui_id_shared::ids::PasswordResetTokenId::new(),
            user_id,
            token_hash: vec![salt; 32],
            issued_at: now,
            expires_at: now + chrono::Duration::minutes(30),
            consumed_at: None,
            requester_ip: None,
            issued_via: sui_id_store::models::ResetTokenOrigin::Email,
            issued_by: None,
            revoked_at: None,
        },
    )
    .await
    .expect("insert token");
}

struct Resp {
    status: StatusCode,
    headers: Vec<(String, String)>,
    body: String,
}

/// Headers that must not differ, minus the two that legitimately vary per
/// request (the CSRF cookie and the request id) — same filter
/// `r115_stage1.rs`'s `stable_headers` uses.
async fn post_forgot_password(state: &AppState, email: &str) -> Resp {
    let get_resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET forgot-password");
    let csrf = extract_set_cookie(get_resp.headers(), "sui_id_csrf").expect("csrf");
    let body = format!("_csrf={csrf}&email={}", urlencode(email));
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/forgot-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST forgot-password");
    let status = resp.status();
    let mut headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .filter(|(k, _)| *k != header::SET_COOKIE && k.as_str() != "x-request-id")
        .map(|(k, v)| {
            (
                k.as_str().to_owned(),
                v.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    headers.sort();
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    Resp {
        status,
        headers,
        body,
    }
}

/// One branch's response, from its own fresh app instance — not six POSTs
/// against one instance, which would run into `/forgot-password`'s own
/// per-IP throttle (five per sixty seconds; these oneshot-driven tests have
/// no `ConnectInfo`, so every request in one instance shares one simulated
/// IP). A fresh instance per branch sidesteps that without touching the
/// throttle itself, which is D1's own amplification bound and is exercised
/// elsewhere, not here.
async fn branch_response(seed: impl AsyncFnOnce(&AppState) -> String) -> Resp {
    let state = test_app();
    enable_smtp(&state).await;
    let email = seed(&state).await;
    post_forgot_password(&state, &email).await
}

#[tokio::test]
async fn rfc124_d5_the_response_is_identical_across_all_six_branches() {
    let unknown = branch_response(async |_state| "d5-nobody@example.test".to_owned()).await;

    let non_local = branch_response(async |state| {
        seed_user(
            state,
            "d5-ldap",
            "d5-ldap@example.test",
            UserSource::Ldap,
            None,
            false,
        )
        .await;
        "d5-ldap@example.test".to_owned()
    })
    .await;

    let never_activated = branch_response(async |state| {
        seed_user(
            state,
            "d5-never-activated",
            "d5-never-activated@example.test",
            UserSource::Local,
            None,
            false,
        )
        .await;
        "d5-never-activated@example.test".to_owned()
    })
    .await;

    let disabled = branch_response(async |state| {
        seed_user(
            state,
            "d5-disabled",
            "d5-disabled@example.test",
            UserSource::Local,
            Some("irrelevant-1"),
            true,
        )
        .await;
        "d5-disabled@example.test".to_owned()
    })
    .await;

    let at_cap = branch_response(async |state| {
        let uid = seed_user(
            state,
            "d5-at-cap",
            "d5-at-cap@example.test",
            UserSource::Local,
            Some("irrelevant-2"),
            false,
        )
        .await;
        for salt in 0..3u8 {
            insert_active_token(state, uid, salt).await;
        }
        "d5-at-cap@example.test".to_owned()
    })
    .await;

    let under_cap = branch_response(async |state| {
        seed_user(
            state,
            "d5-under-cap",
            "d5-under-cap@example.test",
            UserSource::Local,
            Some("irrelevant-3"),
            false,
        )
        .await;
        "d5-under-cap@example.test".to_owned()
    })
    .await;

    for (name, r) in [
        ("non-local", &non_local),
        ("never activated", &never_activated),
        ("disabled", &disabled),
        ("at cap", &at_cap),
        ("under cap (a real send)", &under_cap),
    ] {
        assert_eq!(r.status, unknown.status, "{name}: status");
        assert_eq!(r.headers, unknown.headers, "{name}: headers");
        assert_eq!(r.body, unknown.body, "{name}: body");
    }
}

// ---------- The restart proof ----------
//
// The property the rejected spawned-task alternative would have lost: a
// recovery request survives between the response and the worker draining
// it. Simulated by never starting a worker until after the response has
// been received and the row's durable presence confirmed directly against
// the database — "restart" is standing in for "the process that handled
// the request is not the process that eventually processes the request,"
// which is the only part of a real restart this property depends on.

#[tokio::test]
async fn rfc124_a_recovery_request_survives_a_restart_before_the_worker_drains_it() {
    // Built via `app_over_db`, deliberately not `test_app()`: no worker
    // runs yet, which is the point — this proves the row's durability does
    // not depend on any worker instance being alive at the moment of the
    // request.
    let (state, mailer) = app_over_db(Database::open_in_memory(MasterKey::generate()).expect("db"));
    enable_smtp(&state).await;
    seed_user(
        &state,
        "restart-target",
        "restart-target@example.test",
        UserSource::Local,
        Some("irrelevant-password"),
        false,
    )
    .await;

    let resp = post_forgot_password(&state, "restart-target@example.test").await;
    assert_eq!(resp.status, StatusCode::OK);

    // No worker has run at all yet. The request is not lost: it is sitting
    // in the database as a durable row, exactly as it would be across a
    // real process restart — this is the check a detached `tokio::spawn`
    // could never have passed, because it has no row to check.
    assert_eq!(
        sui_id_store::repos::forgot_password_requests::count_outstanding(&state.db)
            .await
            .expect("count"),
        1,
        "the request is durably recorded, not merely in-flight in a task"
    );
    assert_eq!(mailer.count().await, 0, "nothing has processed it yet");

    // "Restart": a fresh worker, over the same database, that never
    // existed when the request was accepted.
    spawn_forgot_password_worker(&state);
    // Synchronise on the worker's *last* observable effect, not a proxy for
    // it: `process_row` calls `request_reset` (which awaits the send) before
    // it calls `delete`, so `mailer.count() == 1` can already be true while
    // the row is still present — a window a wait on the mail count alone
    // would race. Waiting for the row to be gone is sound in one direction:
    // once it's gone, `request_reset` has already returned, so the mail is
    // already there too.
    wait_until(
        Duration::from_secs(2),
        "the restarted worker drains the surviving request",
        || async {
            sui_id_store::repos::forgot_password_requests::count_outstanding(&state.db)
                .await
                .expect("count")
                == 0
        },
    )
    .await;
    assert_eq!(
        mailer.count().await,
        1,
        "the row's mail was sent before it was deleted"
    );
}

// ---------- The hand-off must not be conditional on the address ----------
//
// "The thing to check hardest," per the dispatch: a record for an unknown
// address and a record for a known one must both land, unconditionally,
// before any worker ever looks at either — not merely produce the same
// *response* (D5 above), which a handler that quietly skipped recording
// for one of them would still pass, because the response template never
// depended on whether the row was written. The only place that regression
// is visible is the row itself.

#[tokio::test]
async fn rfc124_d1_recording_is_unconditional_on_whether_the_address_exists() {
    // No worker in either state — the point is what the request path
    // itself wrote, before anything downstream could touch it.
    let (unknown_state, _m1) =
        app_over_db(Database::open_in_memory(MasterKey::generate()).expect("db"));
    enable_smtp(&unknown_state).await;
    let resp = post_forgot_password(&unknown_state, "nobody-at-all@example.test").await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(
        sui_id_store::repos::forgot_password_requests::count_outstanding(&unknown_state.db)
            .await
            .expect("count"),
        1,
        "an unknown address is recorded exactly like a known one"
    );

    let (known_state, _m2) =
        app_over_db(Database::open_in_memory(MasterKey::generate()).expect("db"));
    enable_smtp(&known_state).await;
    seed_user(
        &known_state,
        "known-target",
        "known-target@example.test",
        UserSource::Local,
        Some("irrelevant-password"),
        false,
    )
    .await;
    let resp = post_forgot_password(&known_state, "known-target@example.test").await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(
        sui_id_store::repos::forgot_password_requests::count_outstanding(&known_state.db)
            .await
            .expect("count"),
        1,
        "a known address is recorded exactly like an unknown one"
    );
}
