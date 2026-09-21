//! RFC 103 stage 4 — the web and CLI surfaces of administrator-issued
//! recovery. The web operation is a dangerous operation (confirm screen,
//! `_confirmed=1`, fresh step-up, a required reason) that renders the link and
//! the token once, in the POST response; the CLI prints them to stdout only.
//! Both are redeemed at the existing `/reset-password` **with SMTP off**, and
//! neither leaks the token into a log line, a redirect or a request URI.

use super::common::*;
use super::r102_stage1::sign_in;
use super::r102_stage7::SteppingClock;
use super::r103_stage1::{
    Captured, NEW_PASSWORD, Resp, complete, events, exec, get, is_invalid_link_page, send,
};
use super::r103_stage3::{Admin, admin, stepped_up, target_user};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::path::Path;
use std::process::{Command, Output};
use sui_id::AppState;
use sui_id_core::time::SharedClock;
use sui_id_i18n::{Locale, Strings};
use sui_id_shared::ids::UserId;
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;

const REASON: &str = "caller verified by call-back, ticket 4711";
const LOCALES: [Locale; 3] = [Locale::Ja, Locale::En, Locale::ZhHans];

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

async fn token_rows(state: &AppState) -> i64 {
    scalar(state, "SELECT COUNT(*) FROM password_reset_tokens".into()).await
}

/// The admin's own request for a page.
async fn get_as(a: &Admin, uri: &str) -> Resp {
    send(
        &a.state,
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .header(header::COOKIE, format!("sui_id_session={}", a.session))
            .body(Body::empty())
            .expect("req"),
    )
    .await
}

/// `POST /admin/users/{id}/recovery-link` with a real CSRF pair. `confirmed`
/// is the value of `_confirmed` (`None` omits the field).
async fn post_issue(a: &Admin, id: UserId, confirmed: Option<&str>, reason: &str) -> Resp {
    let csrf = fetch_csrf(&a.state, &a.session).await;
    let mut body = format!("_csrf={csrf}&reason={}", urlencode(reason));
    if let Some(c) = confirmed {
        body.push_str(&format!("&_confirmed={c}"));
    }
    send(
        &a.state,
        Request::builder()
            .method(Method::POST)
            .uri(format!("/admin/users/{id}/recovery-link"))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                header::COOKIE,
                format!("sui_id_session={}; sui_id_csrf={csrf}", a.session),
            )
            .body(Body::from(body))
            .expect("req"),
    )
    .await
}

async fn issue(a: &Admin, id: UserId) -> Resp {
    post_issue(a, id, Some("1"), REASON).await
}

/// The token shown on the issuance page: the text after `#t=` in the link.
fn token_on_page(body: &str) -> String {
    let start = body
        .find("/reset-password#t=")
        .expect("the link is on the page")
        + "/reset-password#t=".len();
    let end = body[start..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .map(|i| start + i)
        .expect("the link ends");
    body[start..end].to_owned()
}

fn shows(body: &str, pick: fn(&'static Strings) -> &'static str) -> bool {
    LOCALES.iter().any(|l| body.contains(pick(l.strings())))
}

fn redirected_to_step_up(r: &Resp, target: UserId) -> bool {
    r.status.is_redirection()
        && r.location.as_deref()
            == Some(&format!(
                "/me/security/step-up?return_to=%2Fadmin%2Fusers%2F{target}%2Frecovery-link-confirm"
            ))
}

// ── the web operation, end to end, with SMTP off ─────────────────────

#[tokio::test]
async fn r103_s4_web_issue_then_complete_with_smtp_off() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    // The premise of the ruling: no SMTP is configured.
    assert_eq!(
        get(&a.state, "/forgot-password").await.status,
        StatusCode::NOT_FOUND
    );

    let issued = issue(&a, bob).await;
    assert_eq!(issued.status, StatusCode::OK, "{}", issued.body);
    assert!(issued.location.is_none(), "not a redirect");
    let token = token_on_page(&issued.body);
    assert!(
        issued
            .body
            .contains(&format!("https://idp.test/reset-password#t={token}")),
        "the link is built from server.issuer"
    );
    assert!(
        issued.body.matches(&token).count() >= 3,
        "the token is on the page in the link and on its own (each in a field and a copy button)"
    );

    // The completion page opens with SMTP off, and the link works once.
    assert_eq!(
        get(&a.state, "/reset-password").await.status,
        StatusCode::OK
    );
    let done = complete(&a.state, &token).await;
    assert!(done.status.is_redirection(), "{}", done.status);
    assert_eq!(done.location.as_deref(), Some("/admin/login?reset=ok"));
    // bob chose the password: it now works.
    let session = sign_in(&a.state, "bob", NEW_PASSWORD).await;
    assert!(
        !session.is_empty(),
        "bob signs in with the password he chose"
    );
    // The second use is refused.
    let replay = complete(&a.state, &token).await;
    assert_eq!(replay.status, StatusCode::BAD_REQUEST);
    assert!(is_invalid_link_page(&replay));

    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 1);
    let note = scalar(
        &a.state,
        "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.password.reset_completed' \
         AND note = 'origin=web'"
            .into(),
    )
    .await;
    assert_eq!(note, 1, "origin=web joins the completion to the issuance");
}

#[tokio::test]
async fn r103_s4_issuance_response_is_not_cacheable_sends_no_referrer_and_is_not_a_redirect() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let r = issue(&a, bob).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        r.headers
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        r.headers
            .get(header::REFERRER_POLICY)
            .and_then(|v| v.to_str().ok()),
        Some("no-referrer")
    );
    assert!(r.headers.get(header::LOCATION).is_none());
    let token = token_on_page(&r.body);
    for (name, value) in &r.headers {
        assert!(
            !value.to_str().unwrap_or_default().contains(&token),
            "the token is in no response header ({name})"
        );
    }
    // The page loads nothing from another origin.
    assert!(
        !r.body.contains("http://") && !r.body.contains("src=\"https://"),
        "no third-party resources"
    );
}

// ── the surface: who is offered it, and what the screens say ─────────

#[tokio::test]
async fn r103_s4_the_operation_is_offered_only_where_it_could_succeed() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let link = |id: UserId| format!("/admin/users/{id}/recovery-link-confirm");
    let offered = |body: &str, id: UserId| body.contains(&link(id));

    assert!(offered(
        &get_as(&a, &format!("/admin/users/{bob}")).await.body,
        bob
    ));

    // Not for the viewer's own account, another administrator, a disabled,
    // deleted or non-local user.
    assert!(
        !offered(
            &get_as(&a, &format!("/admin/users/{}", a.id)).await.body,
            a.id
        ),
        "self"
    );
    let other_admin = sui_id_core::admin::create_user(
        &a.state.db,
        &a.state.clock,
        None,
        sui_id_store::models::HibpMode::Off,
        &admin_actor_for(a.id),
        sui_id_core::admin::CreateUserSpec {
            username: "carol",
            password: "target-very-strong-password",
            min_password_len: 12,
            display_name: None,
            email: None,
            is_admin: true,
        },
    )
    .await
    .expect("second admin")
    .id;
    assert!(
        !offered(
            &get_as(&a, &format!("/admin/users/{other_admin}"))
                .await
                .body,
            other_admin
        ),
        "an administrator"
    );
    for (label, sql) in [
        ("disabled", "is_disabled = 1"),
        ("non-local", "source = 'ldap'"),
    ] {
        let dave = target_user(&a, &format!("dave-{label}")).await;
        exec(
            &a.state,
            format!("UPDATE users SET {sql} WHERE id = '{dave}'"),
        )
        .await;
        assert!(
            !offered(
                &get_as(&a, &format!("/admin/users/{dave}")).await.body,
                dave
            ),
            "{label}"
        );
    }
}

#[tokio::test]
async fn r103_s4_confirm_screen_requires_a_reason_and_shows_the_handover_guidance() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let r = get_as(&a, &format!("/admin/users/{bob}/recovery-link-confirm")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body.contains(r#"name="_confirmed" value="1""#));
    assert!(
        r.body
            .contains(&format!(r#"action="/admin/users/{bob}/recovery-link""#))
    );
    let textarea = &r.body[r.body.find("<textarea").expect("reason field")..];
    let textarea = &textarea[..textarea.find('>').expect("end of tag")];
    assert!(
        textarea.contains("required"),
        "the reason is required: {textarea}"
    );
    assert!(textarea.contains(r#"maxlength="200""#), "{textarea}");
    assert!(
        shows(&r.body, |t| t.recovery_link_handover),
        "the handover guidance (D11)"
    );
    assert!(shows(&r.body, |t| t.confirm_recovery_link_button));
    // Viewing the screen issues nothing.
    assert_eq!(token_rows(&a.state).await, 0);
}

// ── the gates, in order ──────────────────────────────────────────────

#[tokio::test]
async fn r103_s4_a_post_without_the_confirm_marker_is_refused_and_writes_nothing() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    for confirmed in [None, Some("0"), Some("")] {
        let r = post_issue(&a, bob, confirmed, REASON).await;
        assert_eq!(
            r.status,
            StatusCode::BAD_REQUEST,
            "_confirmed={confirmed:?}"
        );
        assert!(!r.body.contains("/reset-password#t="));
    }
    assert_eq!(token_rows(&a.state).await, 0);
    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 0);
}

#[tokio::test]
async fn r103_s4_a_post_without_a_csrf_pair_is_refused() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let r = send(
        &a.state,
        Request::builder()
            .method(Method::POST)
            .uri(format!("/admin/users/{bob}/recovery-link"))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::COOKIE, format!("sui_id_session={}", a.session))
            .body(Body::from(format!(
                "_confirmed=1&reason={}",
                urlencode(REASON)
            )))
            .expect("req"),
    )
    .await;
    assert!(r.status.is_client_error(), "{}", r.status);
    assert_eq!(token_rows(&a.state).await, 0);
}

#[tokio::test]
async fn r103_s4_no_step_up_and_a_stale_step_up_are_redirected_before_anything_else() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let stale = chrono::Utc::now() - chrono::Duration::minutes(10);
    for (label, at) in [("never stepped up", None), ("a stale step-up", Some(stale))] {
        let session = a.session.clone();
        a.state
            .db
            .with_conn(move |c| {
                c.execute(
                    "UPDATE sessions SET last_step_up_at = ?1 WHERE id = ?2",
                    rusqlite::params![at, session],
                )?;
                Ok(())
            })
            .await
            .expect("arrange");

        let confirm = get_as(&a, &format!("/admin/users/{bob}/recovery-link-confirm")).await;
        assert!(
            redirected_to_step_up(&confirm, bob),
            "{label}: confirm GET: {:?}",
            confirm.location
        );

        // The gate runs before the reason or the target is looked at, so a
        // session that has not stepped up learns nothing else.
        for (case, target, reason) in [
            ("a valid request", bob, REASON),
            ("an empty reason", bob, ""),
            ("an unknown target", UserId::new(), REASON),
        ] {
            let r = post_issue(&a, target, Some("1"), reason).await;
            assert!(
                r.status.is_redirection()
                    && r.location
                        .as_deref()
                        .is_some_and(|l| l.starts_with("/me/security/step-up?return_to=")),
                "{label}, {case}: {} {:?}",
                r.status,
                r.location
            );
        }
        assert_eq!(token_rows(&a.state).await, 0, "{label}");
        assert_eq!(
            events(&a.state, "user.recovery_link.issued").await,
            0,
            "{label}"
        );
    }
    // Stepping up again (the recorded step-up is fresh) lets it through.
    stepped_up(&a.state, &a.session).await;
    assert_eq!(issue(&a, bob).await.status, StatusCode::OK);
}

/// An administrator with a second factor whose application clock is `clock`,
/// and a target user, with the step-up recorded `age_secs` before `base`.
async fn admin_on_clock(
    clock: &std::sync::Arc<SteppingClock>,
    base: chrono::DateTime<chrono::Utc>,
    age_secs: i64,
) -> (Admin, UserId) {
    let mut a = admin().await;
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

/// The step-up lapses between the HTTP gate and the command's commit (RFC
/// 103 D6, RFC 102 B4): the command rolls back with `StepUpRequired`, and the
/// handler sends an administrator who holds a factor to the step-up page. The
/// gate and the command read the same application clock, so the lapse needs a
/// clock that moves between two of its own reads; where the gate's read falls
/// is measured, not assumed (as in `r102_b4_lapse_after_the_gate_...`).
#[tokio::test]
async fn r103_s4_a_lapse_between_the_gate_and_the_commit_redirects_to_step_up_and_writes_nothing() {
    let base = chrono::Utc::now();
    let jump = chrono::Duration::seconds(120);

    // Calibration: with a step-up already 10 minutes old the gate itself
    // refuses, so the reads counted in that POST are those up to and
    // including the gate's.
    let clock = SteppingClock::new(base, jump);
    let (a, bob) = admin_on_clock(&clock, base, 600).await;
    clock.arm(usize::MAX);
    let r = issue(&a, bob).await;
    assert!(
        redirected_to_step_up(&r, bob),
        "the gate refuses: {:?}",
        r.location
    );
    let through_the_gate = clock.reads();
    assert!(through_the_gate >= 1, "the gate reads the clock");

    // The run: a step-up 4 minutes old (fresh at `base`). The clock holds
    // through the gate's read and jumps 2 minutes after it, so the command
    // finds the step-up 6 minutes old.
    let clock = SteppingClock::new(base, jump);
    let (a, bob) = admin_on_clock(&clock, base, 240).await;
    clock.arm(through_the_gate);
    let r = issue(&a, bob).await;
    assert!(
        clock.reads() > through_the_gate,
        "the request passed the gate and read the clock again ({} reads, gate at {through_the_gate})",
        clock.reads()
    );
    assert!(
        redirected_to_step_up(&r, bob),
        "the command's rollback becomes the step-up redirect: {} {:?}",
        r.status,
        r.location
    );
    assert!(!r.body.contains("/reset-password#t="));
    assert_eq!(token_rows(&a.state).await, 0, "no token");
    assert_eq!(
        events(&a.state, "user.recovery_link.issued").await,
        0,
        "no event"
    );

    // Control: the same request on a clock that never jumps commits, and the
    // event records the age as the caller's clock saw it.
    let clock = SteppingClock::new(base, jump);
    let (a, bob) = admin_on_clock(&clock, base, 240).await;
    clock.arm(usize::MAX);
    let r = issue(&a, bob).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let note: Option<String> = a
        .state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'user.recovery_link.issued'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("note");
    assert!(
        note.as_deref()
            .is_some_and(|n| n.ends_with(" step_up=fresh:totp:240")),
        "{note:?}"
    );
}

#[tokio::test]
async fn r103_s4_an_administrator_with_no_second_factor_is_told_so_not_bounced() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let a = Admin { state, id, session };
    let bob = target_user(&a, "bob").await;

    let confirm = get_as(&a, &format!("/admin/users/{bob}/recovery-link-confirm")).await;
    assert_eq!(confirm.status, StatusCode::FORBIDDEN);
    assert!(shows(&confirm.body, |t| t.recovery_refused_needs_second_factor));
    assert!(
        !confirm.body.contains(r#"name="_confirmed""#),
        "no confirm form"
    );

    let r = issue(&a, bob).await;
    assert_eq!(
        r.status,
        StatusCode::FORBIDDEN,
        "not a redirect to a step-up page that cannot succeed"
    );
    assert!(r.location.is_none());
    assert!(shows(&r.body, |t| t.recovery_refused_needs_second_factor));
    assert_eq!(token_rows(&a.state).await, 0);
    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 0);
}

// ── refusals: each has its own message and writes nothing ────────────

#[tokio::test]
async fn r103_s4_each_refusal_has_its_own_message_and_writes_nothing() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let carol = sui_id_core::admin::create_user(
        &a.state.db,
        &a.state.clock,
        None,
        sui_id_store::models::HibpMode::Off,
        &admin_actor_for(a.id),
        sui_id_core::admin::CreateUserSpec {
            username: "carol",
            password: "target-very-strong-password",
            min_password_len: 12,
            display_name: None,
            email: None,
            is_admin: true,
        },
    )
    .await
    .expect("second admin")
    .id;
    let ldap = target_user(&a, "ldap-user").await;
    exec(
        &a.state,
        format!("UPDATE users SET source = 'ldap' WHERE id = '{ldap}'"),
    )
    .await;
    let disabled = target_user(&a, "disabled-user").await;
    exec(
        &a.state,
        format!("UPDATE users SET is_disabled = 1 WHERE id = '{disabled}'"),
    )
    .await;
    let deleted = target_user(&a, "deleted-user").await;
    exec(
        &a.state,
        format!("UPDATE users SET is_deleted = 1, is_disabled = 1 WHERE id = '{deleted}'"),
    )
    .await;
    let tokens_before = token_rows(&a.state).await;

    type Pick = fn(&'static Strings) -> &'static str;
    let long = "r".repeat(201);
    let long_bytes = "あ".repeat(171);
    let cases: Vec<(&str, UserId, String, StatusCode, Pick)> = vec![
        (
            "an administrator",
            carol,
            REASON.into(),
            StatusCode::FORBIDDEN,
            |t| t.recovery_refused_target_admin,
        ),
        (
            "the issuer",
            a.id,
            REASON.into(),
            StatusCode::FORBIDDEN,
            |t| t.recovery_refused_target_self,
        ),
        (
            "a non-local user",
            ldap,
            REASON.into(),
            StatusCode::CONFLICT,
            |t| t.recovery_refused_target_non_local,
        ),
        (
            "a disabled user",
            disabled,
            REASON.into(),
            StatusCode::CONFLICT,
            |t| t.recovery_refused_target_disabled,
        ),
        (
            "a deleted user",
            deleted,
            REASON.into(),
            StatusCode::CONFLICT,
            |t| t.recovery_refused_target_deleted,
        ),
        (
            "an unknown user",
            UserId::new(),
            REASON.into(),
            StatusCode::NOT_FOUND,
            |t| t.recovery_refused_target_unknown,
        ),
        (
            "no reason",
            bob,
            String::new(),
            StatusCode::BAD_REQUEST,
            |t| t.recovery_refused_reason_required,
        ),
        (
            "a blank reason",
            bob,
            "  \t ".into(),
            StatusCode::BAD_REQUEST,
            |t| t.recovery_refused_reason_required,
        ),
        ("201 characters", bob, long, StatusCode::BAD_REQUEST, |t| {
            t.recovery_refused_reason_too_long
        }),
        (
            "171 three-byte characters",
            bob,
            long_bytes,
            StatusCode::BAD_REQUEST,
            |t| t.recovery_refused_reason_too_long,
        ),
        (
            "a newline in the reason",
            bob,
            "line one\nline two".into(),
            StatusCode::BAD_REQUEST,
            |t| t.recovery_refused_reason_control,
        ),
        (
            "a tab in the reason",
            bob,
            "a\tb".into(),
            StatusCode::BAD_REQUEST,
            |t| t.recovery_refused_reason_control,
        ),
    ];
    let all: [Pick; 9] = [
        |t| t.recovery_refused_target_admin,
        |t| t.recovery_refused_target_self,
        |t| t.recovery_refused_target_non_local,
        |t| t.recovery_refused_target_disabled,
        |t| t.recovery_refused_target_deleted,
        |t| t.recovery_refused_target_unknown,
        |t| t.recovery_refused_reason_required,
        |t| t.recovery_refused_reason_too_long,
        |t| t.recovery_refused_reason_control,
    ];
    for (label, target, reason, status, pick) in &cases {
        let r = post_issue(&a, *target, Some("1"), reason).await;
        assert_eq!(r.status, *status, "{label}");
        assert!(r.location.is_none(), "{label}: not a redirect");
        assert!(shows(&r.body, *pick), "{label}: its own message");
        for other in all {
            if other(Locale::En.strings()) != pick(Locale::En.strings()) {
                assert!(!shows(&r.body, other), "{label}: only its own message");
            }
        }
        assert!(!r.body.contains("/reset-password#t="), "{label}: no link");
    }
    assert_eq!(
        token_rows(&a.state).await,
        tokens_before,
        "no token for any refusal"
    );
    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 0);
}

#[tokio::test]
async fn r103_s4_the_sixth_issuance_in_an_hour_is_refused_and_logged_at_warn_without_the_token() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();

    let mut tokens = Vec::new();
    for i in 0..5 {
        let r = issue(&a, bob).await;
        assert_eq!(r.status, StatusCode::OK, "issuance {i}");
        tokens.push(token_on_page(&r.body));
    }
    tracing::callsite::rebuild_interest_cache();
    let sixth = issue(&a, bob).await;
    assert_eq!(sixth.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(shows(&sixth.body, |t| t.recovery_refused_throttled));
    assert!(!sixth.body.contains("/reset-password#t="));
    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 5);

    let logged = String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned();
    let line = logged
        .lines()
        .find(|l| l.contains("recovery link refused: the hourly limit was reached"))
        .unwrap_or_else(|| panic!("no warn line for the throttle:\n{logged}"));
    assert!(line.contains(" WARN "), "{line}");
    assert!(line.contains("request_id="), "{line}");
    assert!(line.contains("U37"), "{line}");
    assert!(
        line.contains(&a.id.to_string()),
        "the acting user is named: {line}"
    );
    for t in &tokens {
        assert!(!logged.contains(t), "no issued token is in the log");
    }
}

// ── the token is in no log line, no redirect and no request URI ──────

#[tokio::test]
async fn r103_s4_the_token_is_in_no_log_line_at_any_level_and_in_no_request_uri() {
    let mut a = admin().await;
    let mut config = (*a.state.config).clone();
    config.log.access_log = true;
    a.state.config = std::sync::Arc::new(config);
    let bob = target_user(&a, "bob").await;

    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();

    let confirm = get_as(&a, &format!("/admin/users/{bob}/recovery-link-confirm")).await;
    assert_eq!(confirm.status, StatusCode::OK);
    let issued = issue(&a, bob).await;
    assert_eq!(issued.status, StatusCode::OK);
    let token = token_on_page(&issued.body);
    let page = get(&a.state, "/reset-password").await;
    assert_eq!(page.status, StatusCode::OK);
    let done = complete(&a.state, &token).await;
    assert!(done.status.is_redirection());
    tracing::callsite::rebuild_interest_cache();
    let replay = complete(&a.state, &token).await;
    assert_eq!(replay.status, StatusCode::BAD_REQUEST);

    let logged = String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned();
    assert!(
        logged.contains(&format!("uri=/admin/users/{bob}/recovery-link")),
        "the access log is active and saw the issuance:\n{logged}"
    );
    assert!(logged.contains("uri=/reset-password"), "and the completion");
    let hash = {
        use sha2::{Digest, Sha256};
        Sha256::digest(token.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    for (what, needle) in [
        ("the token", token.as_str()),
        ("the link", "reset-password#t="),
        ("the token's hash", hash.as_str()),
    ] {
        assert!(
            !logged.contains(needle),
            "{what} is in a log line:\n{logged}"
        );
    }
    // No redirect anywhere on the way carried it.
    for r in [&confirm, &issued, &page, &done, &replay] {
        assert!(
            r.location.as_deref().is_none_or(|l| !l.contains(&token)),
            "a redirect carries the token: {:?}",
            r.location
        );
    }
}

// ── the CLI, with the real binary ────────────────────────────────────

/// A migrated on-disk database with an administrator `root`, a user `bob`, a
/// directory user `dave` and a disabled user `erin`; returns the directory,
/// the config path and the database handle (kept open by the tests that run
/// the CLI "while the server is running").
async fn on_disk(dir: &Path) -> (std::path::PathBuf, Database) {
    let db_path = dir.join("sui-id.sqlite");
    let key_file = dir.join("sui-id.key");
    let key = MasterKey::generate();
    std::fs::write(&key_file, key.to_base64()).expect("key");
    let db = Database::open(&db_path, key).expect("open db");
    let now = chrono::Utc::now();
    for (name, admin, source, disabled, deleted) in [
        ("root", true, "local", false, false),
        ("bob", false, "local", false, false),
        ("dave", false, "ldap", false, false),
        ("erin", false, "local", true, false),
        ("frank", false, "local", true, true),
    ] {
        let id = UserId::new();
        let (uuid, role) = (uuid::Uuid::new_v4(), if admin { "admin" } else { "user" });
        let sql = format!(
            "INSERT INTO users(id, username, is_admin, role, is_disabled, is_deleted, created_at, \
             updated_at, user_uuid, failed_login_count, source) VALUES('{id}', '{name}', {}, '{role}', \
             {}, {}, '{}', '{}', '{uuid}', 0, '{source}')",
            admin as i32,
            disabled as i32,
            deleted as i32,
            now.to_rfc3339(),
            now.to_rfc3339()
        );
        db.with_conn(move |c| Ok(c.execute_batch(&sql)?))
            .await
            .expect("seed user");
    }
    let toml = format!(
        "[server]\nlisten_addr = \"127.0.0.1:0\"\nissuer = \"https://idp.test\"\n\
         [storage]\ndb_path = \"{}\"\nkey_file = \"{}\"\n",
        db_path.display(),
        key_file.display()
    );
    let cfg = dir.join("sui-id.toml");
    std::fs::write(&cfg, toml).expect("config");
    (cfg, db)
}

fn cli(cfg: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sui-id"))
        .args(["admin", "issue-recovery-link", "--config"])
        .arg(cfg)
        .args(extra)
        .env_remove("SUI_ID_MASTER_KEY")
        .output()
        .expect("run sui-id admin issue-recovery-link")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

async fn count(db: &Database, sql: &str) -> i64 {
    let sql = sql.to_owned();
    db.with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("count")
}

#[tokio::test]
async fn r103_s4_cli_issues_a_link_that_completes_over_http_with_smtp_off() {
    let dir = tempfile::tempdir().expect("tempdir");
    // The database is held open here for the whole run: the CLI works while
    // the server is running.
    let (cfg, db) = on_disk(dir.path()).await;
    let o = cli(&cfg, &["--username", "bob", "--reason", REASON]);
    assert!(
        o.status.success(),
        "exit: {:?}\nstderr: {}",
        o.status,
        err(&o)
    );

    let stdout = out(&o);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        4,
        "stdout is the link and the token, labelled:\n{stdout}"
    );
    let token = lines[3].trim().to_owned();
    assert_eq!(
        lines[1],
        format!("https://idp.test/reset-password#t={token}")
    );
    assert!(token.len() >= 40, "a 256-bit token: {token}");
    let stderr = err(&o);
    assert!(
        !stderr.contains(&token),
        "the token is never on stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("reset-password"),
        "nor the link:\n{stderr}"
    );
    assert!(
        stderr.contains("bob"),
        "stderr says what was done: {stderr}"
    );

    assert_eq!(count(&db, "SELECT COUNT(*) FROM audit_log WHERE action = 'user.recovery_link.issued' AND actor IS NULL").await, 1);
    // Redeem it over HTTP, on the same database, with no SMTP configured.
    let (state, _mailer) = app_over_db(db);
    assert_eq!(
        get(&state, "/forgot-password").await.status,
        StatusCode::NOT_FOUND
    );
    let done = complete(&state, &token).await;
    assert!(done.status.is_redirection(), "{}", done.status);
    assert!(!sign_in(&state, "bob", NEW_PASSWORD).await.is_empty());
    let replay = complete(&state, &token).await;
    assert!(is_invalid_link_page(&replay), "the second use is refused");
    assert_eq!(
        scalar(
            &state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.password.reset_completed' \
             AND note = 'origin=cli'"
                .into()
        )
        .await,
        1
    );
}

#[tokio::test]
async fn r103_s4_cli_may_issue_for_an_administrator_the_web_may_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (cfg, db) = on_disk(dir.path()).await;
    let o = cli(
        &cfg,
        &[
            "--username",
            "root",
            "--reason",
            "sole administrator lost their password",
        ],
    );
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM password_reset_tokens WHERE issued_via = 'cli'"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn r103_s4_cli_refusals_exit_non_zero_with_a_message_and_write_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (cfg, db) = on_disk(dir.path()).await;
    let long = "r".repeat(201);
    let cases: Vec<(&str, Vec<&str>, &str)> = vec![
        (
            "an unknown user",
            vec!["--username", "nobody", "--reason", REASON],
            "no such user",
        ),
        (
            "a directory user",
            vec!["--username", "dave", "--reason", REASON],
            "not a local account",
        ),
        (
            "a disabled user",
            vec!["--username", "erin", "--reason", REASON],
            "disabled",
        ),
        (
            "a deleted user",
            vec!["--username", "frank", "--reason", REASON],
            "deleted",
        ),
        (
            "an empty reason",
            vec!["--username", "bob", "--reason", ""],
            "a reason is required",
        ),
        (
            "a blank reason",
            vec!["--username", "bob", "--reason", "   "],
            "a reason is required",
        ),
        (
            "201 characters",
            vec!["--username", "bob", "--reason", &long],
            "too long",
        ),
        (
            "a newline",
            vec!["--username", "bob", "--reason", "a\nb"],
            "control characters",
        ),
        (
            "no --reason",
            vec!["--username", "bob"],
            "requires --reason",
        ),
        (
            "no --username",
            vec!["--reason", REASON],
            "requires --username",
        ),
    ];
    for (label, args, message) in &cases {
        let o = cli(&cfg, args);
        assert!(!o.status.success(), "{label}: must exit non-zero");
        assert!(
            err(&o).contains(message),
            "{label}: stderr should say {message:?}:\n{}",
            err(&o)
        );
        assert_eq!(out(&o), "", "{label}: nothing on stdout");
    }
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM password_reset_tokens").await,
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'user.recovery_link.issued'"
        )
        .await,
        0
    );
}

#[tokio::test]
async fn r103_s4_cli_sixth_issuance_in_an_hour_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (cfg, db) = on_disk(dir.path()).await;
    for i in 0..5 {
        let o = cli(&cfg, &["--username", "bob", "--reason", REASON]);
        assert!(o.status.success(), "issuance {i}: {}", err(&o));
    }
    let sixth = cli(&cfg, &["--username", "bob", "--reason", REASON]);
    assert!(!sixth.status.success());
    assert!(err(&sixth).contains("hourly limit"), "{}", err(&sixth));
    assert_eq!(out(&sixth), "");
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM password_reset_tokens WHERE issued_via = 'cli'"
        )
        .await,
        5
    );
}

#[test]
fn r103_s4_cli_help_and_the_unknown_subaction_message_name_the_command() {
    let help = Command::new(env!("CARGO_BIN_EXE_sui-id"))
        .arg("--help")
        .output()
        .expect("--help");
    let text = out(&help);
    assert!(
        text.contains("sui-id admin issue-recovery-link --username NAME --reason TEXT"),
        "usage line"
    );
    assert!(text.contains("admin issue-recovery-link"), "description");
    let unknown = Command::new(env!("CARGO_BIN_EXE_sui-id"))
        .args(["admin", "no-such-subaction"])
        .output()
        .expect("unknown subaction");
    assert!(!unknown.status.success());
    assert!(
        err(&unknown).contains("issue-recovery-link"),
        "{}",
        err(&unknown)
    );
}
