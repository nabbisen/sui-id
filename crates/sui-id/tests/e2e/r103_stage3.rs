//! RFC 103 stage 3 — the recovery-link data path: an administrator- or
//! operator-issued link is a `password_reset_tokens` row completed at the
//! existing `/reset-password` (U10); issuing revokes earlier links (D3); the
//! other D3 invalidations; and `auth.password.reset_completed` records the
//! link's `origin`. There is no HTTP or CLI issuing surface yet, so issuing
//! goes through `sui_id_core::account::recovery_link`, the code the later
//! stages call.

use super::common::*;
use super::r102_stage1::sign_in;
use super::r103_stage1::{
    Resp, complete, events, exec, is_invalid_link_page, issue_token, reset_app, send,
};
use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::AppState;
use sui_id_core::errors::CoreError;
use sui_id_core::recovery_link;
use sui_id_shared::ids::UserId;
use sui_id_store::StoreError;
use sui_id_store::errors::RecoveryRefusal;

const REASON: &str = "caller verified by call-back, ticket 4711";

pub(super) struct Admin {
    pub(super) state: AppState,
    pub(super) id: UserId,
    pub(super) session: String,
}

/// The setup administrator with a second factor (a passkey row) and a step-up
/// recorded just now, so U37's in-transaction check finds it fresh. **SMTP is
/// off**: `/reset-password` no longer needs it (RFC 103 stage 4).
pub(super) async fn admin() -> Admin {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    exec(
        &state,
        format!(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at) \
             VALUES ('{}', '{id}', X'0102', X'00', 'k', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;
    stepped_up(&state, &session).await;
    Admin { state, id, session }
}

pub(super) async fn stepped_up(state: &AppState, session: &str) {
    let (at, session) = (chrono::Utc::now(), session.to_owned());
    state
        .db
        .with_conn(move |c| {
            c.execute(
                "UPDATE sessions SET last_step_up_at = ?1, last_step_up_method = 'totp' \
                 WHERE id = ?2",
                rusqlite::params![at, session],
            )?;
            Ok(())
        })
        .await
        .expect("step up");
}

pub(super) async fn target_user(a: &Admin, name: &str) -> UserId {
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

async fn issue_web(a: &Admin, target: UserId) -> Result<recovery_link::RecoveryLink, CoreError> {
    recovery_link::issue_as_admin(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        target,
        REASON,
    )
    .await
}

async fn note_of(state: &AppState, action: &str) -> String {
    let sql = format!(
        "SELECT note FROM audit_log WHERE action = '{action}' ORDER BY at DESC, rowid DESC LIMIT 1"
    );
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get::<_, Option<String>>(0))?))
        .await
        .expect("note")
        .expect("a note")
}

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

fn refused_with(r: &Result<impl std::fmt::Debug, CoreError>, why: RecoveryRefusal) -> bool {
    matches!(r, Err(CoreError::Store(StoreError::RecoveryRefused(w))) if *w == why)
}

fn assert_completed(r: &Resp) {
    assert!(r.completed(), "reset succeeds: {}", r.status);
    assert_eq!(r.location, None, "the confirmation is the response itself");
}

// ── issuing, and completing at the existing page ─────────────────────

#[tokio::test]
async fn r103_s3_a_web_issued_link_completes_and_records_origin_web() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let link = issue_web(&a, bob).await.expect("issue");
    assert_eq!(link.invalidated, 0);

    let issued = note_of(&a.state, "user.recovery_link.issued").await;
    assert!(
        issued.contains("reason=caller%20verified%20by%20call-back,%20ticket%204711"),
        "{issued}"
    );
    assert!(issued.contains(" via=web "), "{issued}");
    assert!(issued.contains(" step_up=fresh:totp:"), "{issued}");
    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 1);

    assert_completed(&complete(&a.state, link.token.expose()).await);
    assert_eq!(
        note_of(&a.state, "auth.password.reset_completed").await,
        "origin=web"
    );
    // The link is single-use.
    assert!(is_invalid_link_page(
        &complete(&a.state, link.token.expose()).await
    ));
}

#[tokio::test]
async fn r103_s3_an_operator_issued_link_completes_and_records_origin_cli() {
    let a = admin().await;
    target_user(&a, "bob").await;
    let (bob, link) = recovery_link::issue_as_operator(&a.state.db, &a.state.clock, "bob", REASON)
        .await
        .expect("issue");
    assert_eq!(
        scalar(
            &a.state,
            format!("SELECT COUNT(*) FROM users WHERE id = '{bob}' AND username = 'bob'")
        )
        .await,
        1
    );
    let issued = note_of(&a.state, "user.recovery_link.issued").await;
    assert!(issued.contains(" via=cli "), "{issued}");
    assert!(
        issued.ends_with(" step_up=not_applicable:system_principal"),
        "{issued}"
    );
    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'user.recovery_link.issued' \
             AND actor IS NULL"
                .into()
        )
        .await,
        1,
        "no actor"
    );

    assert_completed(&complete(&a.state, link.token.expose()).await);
    assert_eq!(
        note_of(&a.state, "auth.password.reset_completed").await,
        "origin=cli"
    );
}

#[tokio::test]
async fn r103_s3_a_forgot_password_link_still_records_origin_email() {
    let (state, mailer, _admin) = reset_app().await;
    let (token, _) = issue_token(&state, &mailer).await;
    assert_completed(&complete(&state, &token).await);
    assert_eq!(
        note_of(&state, "auth.password.reset_completed").await,
        "origin=email"
    );
}

#[tokio::test]
async fn r103_s3_the_token_is_stored_only_as_a_hash_and_never_audited() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let link = issue_web(&a, bob).await.expect("issue");
    let token = link.token.expose().to_owned();
    let hex_of_hash = {
        use sha2::{Digest, Sha256};
        Sha256::digest(token.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let leaks = scalar(
        &a.state,
        format!(
            "SELECT COUNT(*) FROM audit_log WHERE COALESCE(note, '') LIKE '%{token}%' \
             OR COALESCE(note, '') LIKE '%{hex_of_hash}%' \
             OR COALESCE(target, '') LIKE '%{token}%'"
        ),
    )
    .await;
    assert_eq!(
        leaks, 0,
        "neither the token nor its hash is in the audit log"
    );
    let stored_plaintext = scalar(
        &a.state,
        format!(
            "SELECT COUNT(*) FROM password_reset_tokens WHERE CAST(token_hash AS TEXT) = '{token}'"
        ),
    )
    .await;
    assert_eq!(stored_plaintext, 0, "only the hash is stored");
    assert!(!format!("{link:?}").contains(&token), "Debug is redacted");
}

// ── D3: one live link per user ───────────────────────────────────────

#[tokio::test]
async fn r103_s3_issuing_a_second_link_revokes_the_first_at_completion() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let first = issue_web(&a, bob).await.expect("first");
    let second = issue_web(&a, bob).await.expect("second");
    assert_eq!(second.invalidated, 1);
    assert!(
        note_of(&a.state, "user.recovery_link.issued")
            .await
            .contains(" invalidated=1 ")
    );

    let refused = complete(&a.state, first.token.expose()).await;
    assert!(
        is_invalid_link_page(&refused),
        "the revoked link is refused"
    );
    assert_eq!(events(&a.state, "auth.password.reset_completed").await, 0);
    // The refused attempt did not consume anything: the newer link still works.
    assert_completed(&complete(&a.state, second.token.expose()).await);
}

#[tokio::test]
async fn r103_s3_an_administrator_issued_link_revokes_a_forgot_password_link() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    // An email-origin token minted directly for bob (he has no address).
    let token = {
        use sha2::{Digest, Sha256};
        let plaintext = "email-origin-token-for-bob".to_owned();
        let now = chrono::Utc::now();
        let row = sui_id_store::models::PasswordResetTokenRow {
            id: sui_id_shared::ids::PasswordResetTokenId::new(),
            user_id: bob,
            token_hash: Sha256::digest(plaintext.as_bytes()).to_vec(),
            issued_at: now,
            expires_at: now + chrono::Duration::minutes(30),
            consumed_at: None,
            requester_ip: None,
            issued_via: sui_id_store::models::ResetTokenOrigin::Email,
            issued_by: None,
            revoked_at: None,
        };
        sui_id_store::repos::password_reset_tokens::insert(&a.state.db, &row)
            .await
            .expect("insert");
        plaintext
    };
    let link = issue_web(&a, bob).await.expect("issue");
    assert_eq!(link.invalidated, 1);
    assert!(is_invalid_link_page(&complete(&a.state, &token).await));
    assert_completed(&complete(&a.state, link.token.expose()).await);
}

async fn outstanding_link(a: &Admin, name: &str) -> (UserId, String) {
    let user = target_user(a, name).await;
    let link = recovery_link::issue_as_operator(&a.state.db, &a.state.clock, name, REASON)
        .await
        .expect("issue")
        .1;
    (user, link.token.expose().to_owned())
}

#[tokio::test]
async fn r103_s3_disabling_the_user_revokes_the_link() {
    let a = admin().await;
    let (bob, token) = outstanding_link(&a, "bob").await;
    let csrf = fetch_csrf(&a.state, &a.session).await;
    let resp = send(
        &a.state,
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
    .await;
    assert!(resp.status.is_redirection(), "disabled: {}", resp.status);
    assert_eq!(
        scalar(
            &a.state,
            format!("SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = '{bob}' AND revoked_at IS NOT NULL")
        )
        .await,
        1
    );
    // Enabling the account does not bring the link back.
    sui_id_core::admin::set_user_disabled(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        false,
        None,
    )
    .await
    .expect("enable");
    assert!(is_invalid_link_page(&complete(&a.state, &token).await));
}

#[tokio::test]
async fn r103_s3_deleting_the_user_revokes_the_link() {
    let a = admin().await;
    let (bob, token) = outstanding_link(&a, "bob").await;
    sui_id_core::admin::delete_user(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        None,
    )
    .await
    .expect("delete");
    assert_eq!(
        scalar(
            &a.state,
            format!("SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = '{bob}' AND revoked_at IS NOT NULL")
        )
        .await,
        1
    );
    assert!(is_invalid_link_page(&complete(&a.state, &token).await));
}

#[tokio::test]
async fn r103_s3_a_self_service_password_change_revokes_the_link() {
    let (state, mailer, _admin) = reset_app().await;
    let (token, id) = issue_token(&state, &mailer).await;
    let session = sign_in(&state, USERNAME, PASSWORD).await;

    let csrf = fetch_csrf(&state, &session).await;
    let new_pw = "the-new-tester-password";
    let resp = send(
        &state,
        Request::builder()
            .method(Method::POST)
            .uri("/me/security/password")
            .header(
                header::COOKIE,
                format!("sui_id_session={session}; sui_id_csrf={csrf}"),
            )
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "_csrf={csrf}&current_password={}&new_password={}&confirm_password={}",
                urlencode(PASSWORD),
                urlencode(new_pw),
                urlencode(new_pw)
            )))
            .expect("req"),
    )
    .await;
    assert!(resp.status.is_redirection(), "changed: {}", resp.status);
    assert_eq!(
        scalar(
            &state,
            format!("SELECT revoked_at IS NOT NULL FROM password_reset_tokens WHERE id = '{id}'")
        )
        .await,
        1,
        "revoked, not consumed"
    );
    assert!(is_invalid_link_page(&complete(&state, &token).await));
}

#[tokio::test]
async fn r103_s3_completing_one_link_revokes_the_others() {
    let (state, mailer, _admin) = reset_app().await;
    let (older, older_id) = issue_token(&state, &mailer).await;
    let (newer, _) = issue_token(&state, &mailer).await;
    // Two live email-origin links (the forgot-password flow does not revoke).
    assert_completed(&complete(&state, &newer).await);
    assert_eq!(
        scalar(
            &state,
            format!(
                "SELECT revoked_at IS NOT NULL FROM password_reset_tokens WHERE id = '{older_id}'"
            )
        )
        .await,
        1
    );
    assert!(is_invalid_link_page(&complete(&state, &older).await));
}

// ── D5 and D8 through the core entry points ──────────────────────────

#[tokio::test]
async fn r103_s3_refusals_write_no_token_and_no_event() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let carol = target_user(&a, "carol").await;
    sui_id_core::admin::set_user_disabled(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        carol,
        true,
        None,
    )
    .await
    .expect("disable carol");
    let events_before = scalar(&a.state, "SELECT COUNT(*) FROM audit_log".into()).await;

    assert!(refused_with(
        &issue_web(&a, a.id).await,
        RecoveryRefusal::TargetIsSelf
    ));
    assert!(refused_with(
        &issue_web(&a, carol).await,
        RecoveryRefusal::TargetDisabled
    ));
    assert!(refused_with(
        &issue_web(&a, UserId::new()).await,
        RecoveryRefusal::TargetUnknown
    ));
    let r = recovery_link::issue_as_operator(&a.state.db, &a.state.clock, "nobody", REASON).await;
    assert!(refused_with(&r, RecoveryRefusal::TargetUnknown));
    let r = recovery_link::issue_as_admin(
        &a.state.db,
        &a.state.clock,
        &admin_actor_on_session(a.id, &a.session),
        bob,
        "  ",
    )
    .await;
    assert!(refused_with(&r, RecoveryRefusal::ReasonRequired));

    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM password_reset_tokens".into()
        )
        .await,
        0
    );
    assert_eq!(
        scalar(&a.state, "SELECT COUNT(*) FROM audit_log".into()).await,
        events_before,
        "no event for any refusal"
    );
}

#[tokio::test]
async fn r103_s3_an_administrator_with_no_second_factor_cannot_issue_on_the_web() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let a = Admin { state, id, session };
    let bob = target_user(&a, "bob").await;
    let r = issue_web(&a, bob).await;
    assert!(
        matches!(r, Err(CoreError::Store(StoreError::StepUpRequired))),
        "not_required is refused for this operation: {r:?}"
    );
    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM password_reset_tokens".into()
        )
        .await,
        0
    );
    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 0);
    // The operator entry needs no session and no factor.
    recovery_link::issue_as_operator(&a.state.db, &a.state.clock, "bob", REASON)
        .await
        .expect("the CLI entry is not gated on a session");
}

#[tokio::test]
async fn r103_s3_the_sixth_issuance_in_an_hour_is_refused() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    for i in 0..5 {
        issue_web(&a, bob)
            .await
            .unwrap_or_else(|e| panic!("issuance {i}: {e:?}"));
    }
    let sixth = issue_web(&a, bob).await;
    assert!(
        refused_with(&sixth, RecoveryRefusal::Throttled),
        "{sixth:?}"
    );
    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 5);

    for i in 0..5 {
        recovery_link::issue_as_operator(&a.state.db, &a.state.clock, "bob", REASON)
            .await
            .unwrap_or_else(|e| panic!("cli issuance {i}: {e:?}"));
    }
    let sixth = recovery_link::issue_as_operator(&a.state.db, &a.state.clock, "bob", REASON).await;
    assert!(
        refused_with(&sixth, RecoveryRefusal::Throttled),
        "{sixth:?}"
    );
    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 10);
}
