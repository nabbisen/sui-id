//! RFC 094 M2a — rollback coverage for the seven sign-in/step-up commands
//! (RFC 102 Part A/B), L01-L07. Each proves, by injected append failure,
//! that the whole transaction (the domain mutation and the audit append
//! together) rolls back — never merely that the call returned `Err`.

use super::*;
use sui_id_shared::ids::{PendingMfaId, SessionId};

fn a_shadow() -> crate::repos::users::LdapShadowData {
    crate::repos::users::LdapShadowData {
        username: format!("ldap-{}", uuid::Uuid::new_v4()),
        display_name: None,
        email: None,
        external_stable_id: format!("dn-{}", uuid::Uuid::new_v4()),
    }
}

fn a_pending_session(user_id: UserId) -> crate::models::SessionRow {
    crate::models::SessionRow {
        id: SessionId::new(),
        user_id,
        expires_at: Utc::now() + TimeDelta::hours(1),
        created_at: Utc::now(),
        revoked_at: None,
        auth_methods: vec![],
        last_step_up_at: None,
        last_used_at: None,
    }
}

async fn user_with_confirmed_totp(db: &Database) -> UserId {
    let user = a_user();
    repos::users::create(db, &user).await.expect("create user");
    repos::user_totp::upsert_pending(db, user.id, b"totp-secret-placeholder")
        .await
        .expect("seed totp");
    repos::user_totp::confirm_with_recovery(db, user.id, br#"["code-1","code-2"]"#)
        .await
        .expect("confirm totp");
    user.id
}

async fn mfa_failure_count(db: &Database, id: UserId) -> i64 {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT mfa_failure_count FROM users WHERE id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("count")
}

async fn step_up_failure_count(db: &Database, id: SessionId) -> i64 {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT step_up_failure_count FROM sessions WHERE id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("count")
}

// ── L01 — password sign-in ───────────────────────────────────────────

#[tokio::test]
async fn l01_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let mut user = a_user();
    user.failed_login_count = 3;
    repos::users::create(&db, &user).await.expect("create user");
    let session = a_pending_session(user.id);
    let session_id = session.id;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = sign_in_with_password(&db, session).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert_eq!(row.failed_login_count, 3, "the counter reset rolled back");
    assert!(
        repos::sessions::get(&db, session_id).await.is_err(),
        "no session row was inserted"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // Control: the same call, with no failure injected, really does
    // reset the counter and insert a session -- without this, the
    // assertions above would pass just as well if the mutation never ran
    // at all.
    let control_session = a_pending_session(user.id);
    let control_id = control_session.id;
    let before_count = audit_rows(&db).await;
    sign_in_with_password(&db, control_session)
        .await
        .expect("the control succeeds");
    let row = repos::users::get(&db, user.id).await.expect("get");
    assert_eq!(row.failed_login_count, 0);
    assert!(repos::sessions::get(&db, control_id).await.is_ok());
    assert_eq!(audit_rows(&db).await, before_count + 1, "exactly one event");
    record_rollback_coverage("L01");
    record_exactly_once_coverage("L01");
}

// ── L02 — second-factor sign-in ──────────────────────────────────────

#[tokio::test]
async fn l02_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let user_id = user_with_confirmed_totp(&db).await;
    let pending_id = PendingMfaId::new();
    repos::login_pending_mfa::insert(
        &db,
        &crate::models::LoginPendingMfaRow {
            id: pending_id,
            user_id,
            expires_at: Utc::now() + TimeDelta::minutes(5),
            created_at: Utc::now(),
        },
    )
    .await
    .expect("seed pending mfa");
    let session = a_pending_session(user_id);
    let session_id = session.id;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = complete_second_factor(
        &db,
        pending_id,
        session,
        SecondFactorProof::Totp { step: 1 },
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        repos::login_pending_mfa::get(&db, pending_id)
            .await
            .expect("get pending")
            .is_some(),
        "the pending-row consume rolled back"
    );
    assert!(
        repos::sessions::get(&db, session_id).await.is_err(),
        "no session row was inserted"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // Control: the same call, with no failure injected, really does
    // consume the pending row and insert a session -- without this, the
    // assertions above would pass just as well if the mutation never ran
    // at all.
    let control_session = a_pending_session(user_id);
    let control_id = control_session.id;
    let before_count = audit_rows(&db).await;
    complete_second_factor(
        &db,
        pending_id,
        control_session,
        SecondFactorProof::Totp { step: 1 },
    )
    .await
    .expect("the control succeeds");
    assert!(
        repos::login_pending_mfa::get(&db, pending_id)
            .await
            .expect("get pending")
            .is_none(),
        "the control really consumed the pending row"
    );
    assert!(repos::sessions::get(&db, control_id).await.is_ok());
    assert_eq!(audit_rows(&db).await, before_count + 1, "exactly one event");
    record_rollback_coverage("L02");
    record_exactly_once_coverage("L02");
}

// ── L03 — directory sign-in ──────────────────────────────────────────

#[tokio::test]
async fn l03_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let user_id = UserId::new();
    let session = a_pending_session(user_id);
    let session_id = session.id;
    let shadow = a_shadow();
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = sign_in_from_directory(&db, shadow, "corp-ldap".into(), session).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        repos::users::get(&db, user_id).await.is_err(),
        "no shadow user row was created"
    );
    assert!(
        repos::sessions::get(&db, session_id).await.is_err(),
        "no session row was inserted"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // Control: the same call, with no failure injected, really does
    // create the shadow user and insert a session -- without this, the
    // assertions above would pass just as well if the mutation never ran
    // at all.
    let control_session = a_pending_session(user_id);
    let control_id = control_session.id;
    let before_count = audit_rows(&db).await;
    sign_in_from_directory(&db, a_shadow(), "corp-ldap".into(), control_session)
        .await
        .expect("the control succeeds");
    assert!(repos::users::get(&db, user_id).await.is_ok());
    assert!(repos::sessions::get(&db, control_id).await.is_ok());
    assert_eq!(audit_rows(&db).await, before_count + 1, "exactly one event");
    record_rollback_coverage("L03");
    record_exactly_once_coverage("L03");
}

// ── L04 — federated sign-in ───────────────────────────────────────────

#[tokio::test]
async fn l04_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let session = a_pending_session(user.id);
    let session_id = session.id;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = sign_in_federated(
        &db,
        "upstream-provider".into(),
        "upstream-sub".into(),
        session,
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert!(
        row.last_login_at.is_none(),
        "the last_login_at write rolled back"
    );
    assert!(
        repos::sessions::get(&db, session_id).await.is_err(),
        "no session row was inserted"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // Control: the same call, with no failure injected, really does set
    // last_login_at and insert a session -- without this, the assertions
    // above would pass just as well if the mutation never ran at all.
    let control_session = a_pending_session(user.id);
    let control_id = control_session.id;
    let before_count = audit_rows(&db).await;
    sign_in_federated(
        &db,
        "upstream-provider".into(),
        "upstream-sub".into(),
        control_session,
    )
    .await
    .expect("the control succeeds");
    let row = repos::users::get(&db, user.id).await.expect("get");
    assert!(row.last_login_at.is_some());
    assert!(repos::sessions::get(&db, control_id).await.is_ok());
    assert_eq!(audit_rows(&db).await, before_count + 1, "exactly one event");
    record_rollback_coverage("L04");
    record_exactly_once_coverage("L04");
}

// ── L05 — step-up success ────────────────────────────────────────────

#[tokio::test]
async fn l05_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let user_id = user_with_confirmed_totp(&db).await;
    let session_id = seed_active_session(&db, user_id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = complete_step_up(
        &db,
        user_id,
        session_id,
        StepUpProof::Totp { step: 1 },
        "/me/security".into(),
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.last_step_up_at.is_none(),
        "the step-up stamp rolled back"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // Control: the same call, with no failure injected, really does
    // stamp last_step_up_at -- without this, the assertion above would
    // pass just as well if the mutation never ran at all.
    let before_count = audit_rows(&db).await;
    complete_step_up(
        &db,
        user_id,
        session_id,
        StepUpProof::Totp { step: 1 },
        "/me/security".into(),
        Utc::now(),
    )
    .await
    .expect("the control succeeds");
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(session.last_step_up_at.is_some());
    assert_eq!(audit_rows(&db).await, before_count + 1, "exactly one event");
    record_rollback_coverage("L05");
    record_exactly_once_coverage("L05");
}

// ── L06 — step-up failure ────────────────────────────────────────────

#[tokio::test]
async fn l06_injected_failure_before_append_rolls_back_the_counter() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = record_step_up_failure(&db, user.id, session_id, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert_eq!(
        step_up_failure_count(&db, session_id).await,
        0,
        "the counter bump rolled back"
    );
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(session.revoked_at.is_none());
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // Control: the same call, with no failure injected, really does
    // bump the counter -- without this, the assertion above would pass
    // just as well if the mutation never ran at all.
    let before_count = audit_rows(&db).await;
    record_step_up_failure(&db, user.id, session_id, Utc::now())
        .await
        .expect("the control succeeds");
    assert_eq!(step_up_failure_count(&db, session_id).await, 1);
    assert_eq!(audit_rows(&db).await, before_count + 1, "exactly one event");
    record_rollback_coverage("L06");
    record_exactly_once_coverage("L06");
}

// ── L07 — second-factor failure ──────────────────────────────────────

#[tokio::test]
async fn l07_injected_failure_before_append_rolls_back_the_counter() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = record_second_factor_failure(&db, user.id, |_count| None).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert_eq!(
        mfa_failure_count(&db, user.id).await,
        0,
        "the counter bump rolled back"
    );
    let row = repos::users::get(&db, user.id).await.expect("get");
    assert!(row.locked_until.is_none());
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // Control: the same call, with no failure injected, really does
    // bump the counter -- without this, the assertion above would pass
    // just as well if the mutation never ran at all.
    let before_count = audit_rows(&db).await;
    record_second_factor_failure(&db, user.id, |_count| None)
        .await
        .expect("the control succeeds");
    assert_eq!(mfa_failure_count(&db, user.id).await, 1);
    assert_eq!(audit_rows(&db).await, before_count + 1, "exactly one event");
    record_rollback_coverage("L07");
    record_exactly_once_coverage("L07");
}
