use super::*;

// ── U07 — admin MFA reset ──────────────────────────────────────

async fn seed_passkey(db: &Database, user_id: UserId) {
    repos::user_webauthn_credentials::create(
        db,
        &crate::models::UserWebauthnCredentialRow {
            id: sui_id_shared::ids::WebauthnCredentialId::new(),
            user_id,
            credential_id: format!("cred-{}", uuid::Uuid::new_v4()).into_bytes(),
            passkey_enc: vec![],
            nickname: "test passkey".into(),
            created_at: Utc::now(),
            last_used_at: None,
        },
        b"passkey-json-placeholder",
    )
    .await
    .expect("seed passkey");
}

#[tokio::test]
async fn u07_reset_removes_totp_and_passkeys_and_appends_event() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::user_totp::upsert_pending(&db, user.id, b"totp-secret-placeholder")
        .await
        .expect("seed totp");
    seed_passkey(&db, user.id).await;
    seed_passkey(&db, user.id).await;

    let audited = admin_reset_mfa(
        &db,
        admin,
        session,
        user.id,
        Some("lost authenticator".to_string()),
        Utc::now(),
    )
    .await
    .expect("reset");
    let (totp_removed, passkeys_removed) = audited.into_inner();
    assert!(totp_removed);
    assert_eq!(passkeys_removed, 2);

    assert!(
        repos::user_totp::get(&db, user.id)
            .await
            .expect("get totp")
            .is_none(),
        "TOTP enrollment must be gone"
    );
    assert!(
        repos::user_webauthn_credentials::list_for_user(&db, user.id)
            .await
            .expect("list passkeys")
            .is_empty(),
        "every passkey must be gone"
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "mfa.admin_reset");
    assert_eq!(
        tail.note.as_deref(),
        Some(
            "totp=removed passkeys=2 reason=lost authenticator step_up=not_required:no_second_factor"
        )
    );
}

#[tokio::test]
async fn u07_reset_with_no_factors_reports_absent_and_zero() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");

    let audited = admin_reset_mfa(&db, admin, session, user.id, None, Utc::now())
        .await
        .expect("reset");
    let (totp_removed, passkeys_removed) = audited.into_inner();
    assert!(!totp_removed);
    assert_eq!(passkeys_removed, 0);

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(
        tail.note.as_deref(),
        Some("totp=absent passkeys=0 step_up=not_required:no_second_factor")
    );
}

#[tokio::test]
async fn u07_reset_of_nonexistent_user_returns_not_found_and_appends_nothing() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let before_audit = latest_audit_action(&db).await;

    let result = admin_reset_mfa(&db, admin, session, UserId::new(), None, Utc::now()).await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "the existence probe (get_role_within_tx) must reject a target that \
         was never created, not silently succeed with nothing to remove"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn u07_reset_of_soft_deleted_user_returns_not_found() {
    // Reviewer finding, 2026-09-09 (`.git-exclude/reviewed/
    // 094-wave-b-u07-2026-09-09.md` §2): `get_role_within_tx`
    // (reused here as an existence probe) filters `is_deleted = 0`;
    // `users::get`, the pre-conversion check it replaced, did not.
    // A reset against a soft-deleted user used to succeed; it must
    // now be rejected, deliberately, not as an unstated side effect
    // of borrowing a helper for its query shape.
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::user_totp::upsert_pending(&db, user.id, b"totp-secret-placeholder")
        .await
        .expect("seed totp");
    repos::users::soft_delete(&db, user.id)
        .await
        .expect("soft delete");
    let before_audit = latest_audit_action(&db).await;

    let result = admin_reset_mfa(&db, admin, session, user.id, None, Utc::now()).await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "a soft-deleted target must be rejected, not silently reset"
    );

    assert!(
        repos::user_totp::get(&db, user.id)
            .await
            .expect("get totp")
            .is_some(),
        "the TOTP row must be untouched -- rejection happens before any mutation"
    );
    assert_eq!(
        latest_audit_action(&db).await,
        before_audit,
        "a rejected reset must not emit an audit row"
    );
}

#[tokio::test]
async fn u07_injected_failure_before_append_rolls_back_totp_and_passkey_removal() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::user_totp::upsert_pending(&db, user.id, b"totp-secret-placeholder")
        .await
        .expect("seed totp");
    seed_passkey(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = admin_reset_mfa(&db, admin, session, user.id, None, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        repos::user_totp::get(&db, user.id)
            .await
            .expect("get totp")
            .is_some(),
        "the TOTP delete rolled back"
    );
    assert_eq!(
        repos::user_webauthn_credentials::list_for_user(&db, user.id)
            .await
            .expect("list passkeys")
            .len(),
        1,
        "the passkey delete rolled back too"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

// ── U14, U15 — an injected append failure leaves the factor unchanged ───
// (RFC 102 closure prerequisite 4; U12's counterpart is the e2e test
// `r102_factor_added_rolls_back_with_its_write`.) Each has a control that
// runs the same call with no injected failure, so the test cannot pass
// because the call was never able to change the factor.

async fn user_with_confirmed_totp(db: &Database) -> UserId {
    let user = a_user();
    repos::users::create(db, &user).await.expect("create user");
    repos::user_totp::upsert_pending(db, user.id, b"totp-secret-placeholder")
        .await
        .expect("seed totp");
    repos::user_totp::confirm_with_recovery(db, user.id, br#"["old-hash-1","old-hash-2"]"#)
        .await
        .expect("confirm");
    user.id
}

async fn recovery_blob(db: &Database, user: UserId) -> Vec<u8> {
    repos::user_totp::get(db, user)
        .await
        .expect("get totp")
        .expect("enrolment")
        .recovery_codes_enc
        .expect("recovery codes")
}

#[tokio::test]
async fn u14_injected_failure_before_append_leaves_the_recovery_codes_unchanged() {
    let db = fresh_db();
    let user = user_with_confirmed_totp(&db).await;
    let before = recovery_blob(&db, user).await;
    let before_audit = latest_audit_action(&db).await;
    let sealed = repos::user_totp::seal_recovery_codes(&db, br#"["new-hash-1"]"#).expect("seal");

    db.fault_injector().fail_before_next_append();
    let result = regenerate_recovery_codes(&db, user, sealed.clone()).await;
    assert!(result.is_err(), "injected failure must surface as Err");
    assert_eq!(
        recovery_blob(&db, user).await,
        before,
        "the old codes are still the stored ones"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit, "no event");

    // Control: the same call, with no failure injected, replaces the codes
    // and writes the event.
    regenerate_recovery_codes(&db, user, sealed.clone())
        .await
        .expect("the control succeeds");
    assert_eq!(recovery_blob(&db, user).await, sealed);
    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("auth.mfa.factor_added")
    );
}

#[tokio::test]
async fn u15_injected_failure_before_append_registers_no_passkey() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let row = || crate::models::UserWebauthnCredentialRow {
        id: sui_id_shared::ids::WebauthnCredentialId::new(),
        user_id: user.id,
        credential_id: b"cred-u15".to_vec(),
        passkey_enc: b"sealed-passkey-placeholder".to_vec(),
        nickname: "u15".into(),
        created_at: Utc::now(),
        last_used_at: None,
    };
    let count = |db: &Database| {
        let db = db.clone();
        async move {
            repos::user_webauthn_credentials::list_for_user(&db, user.id)
                .await
                .expect("list passkeys")
                .len()
        }
    };
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = register_passkey(&db, row()).await;
    assert!(result.is_err(), "injected failure must surface as Err");
    assert_eq!(count(&db).await, 0, "no passkey was registered");
    assert_eq!(latest_audit_action(&db).await, before_audit, "no event");

    // Control: with no failure injected the same registration lands.
    register_passkey(&db, row())
        .await
        .expect("the control succeeds");
    assert_eq!(count(&db).await, 1);
    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("auth.mfa.factor_added")
    );
}
