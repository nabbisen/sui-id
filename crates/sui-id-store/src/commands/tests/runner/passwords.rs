use super::*;

// ── U06 — admin password reset ───────────────────────────────────

#[tokio::test]
async fn u06_reset_swaps_credential_revokes_session_and_appends_event() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let session_id = seed_active_session(&db, user.id).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    let audited = reset_user_password(&db, an_admin(), user.id, new_credential)
        .await
        .expect("reset");
    audited.into_inner();

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(cred.password_hash, "new-hash-placeholder");

    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_some(),
        "the target's session must be revoked in the same transaction as the reset"
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "user.reset_password");
}

#[tokio::test]
async fn u06_injected_failure_before_append_rolls_back_credential_and_session_revoke() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    db.fault_injector().fail_before_next_append();
    let result = reset_user_password(&db, an_admin(), user.id, new_credential).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(
        cred.password_hash, "old-hash-placeholder",
        "the credential swap rolled back"
    );
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_none(),
        "the session revoke rolled back too"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

// ── U09 — self password change ─────────────────────────────────

#[tokio::test]
async fn u09_change_with_sweep_revokes_others_keeps_current_and_appends_counts() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");

    let keep_id = seed_active_session(&db, user.id).await;
    let other_id = seed_active_session(&db, user.id).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    let audited = change_password_self(&db, user.id, new_credential, Some(keep_id), true)
        .await
        .expect("change password");
    let (sessions_revoked, refresh_tokens_revoked) = audited.into_inner();
    assert_eq!(sessions_revoked, 1);
    assert_eq!(refresh_tokens_revoked, 0);

    let kept = repos::sessions::get(&db, keep_id).await.expect("get kept");
    assert!(
        kept.revoked_at.is_none(),
        "the current session must survive"
    );
    let other = repos::sessions::get(&db, other_id)
        .await
        .expect("get other");
    assert!(
        other.revoked_at.is_some(),
        "every other session must be revoked"
    );

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(cred.password_hash, "new-hash-placeholder");

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "auth.password.changed_self");
    assert_eq!(
        tail.note.as_deref(),
        Some("sessions_revoked=1 refresh_tokens_revoked=0")
    );
}

#[tokio::test]
async fn u09_change_without_sweep_revokes_nothing_and_reports_zero() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let session_id = seed_active_session(&db, user.id).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    let audited = change_password_self(&db, user.id, new_credential, None, false)
        .await
        .expect("change password");
    let (sessions_revoked, refresh_tokens_revoked) = audited.into_inner();
    assert_eq!(sessions_revoked, 0);
    assert_eq!(refresh_tokens_revoked, 0);

    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_none(),
        "no sweep was requested; nothing should be revoked"
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(
        tail.note.as_deref(),
        Some("sessions_revoked=0 refresh_tokens_revoked=0")
    );
}

#[tokio::test]
async fn u09_injected_failure_before_append_rolls_back_credential_and_revocations() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    db.fault_injector().fail_before_next_append();
    let result = change_password_self(&db, user.id, new_credential, None, true).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(
        cred.password_hash, "old-hash-placeholder",
        "the credential swap rolled back"
    );
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(session.revoked_at.is_none(), "the sweep rolled back too");
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

// ── U10 — forgot-password completion ───────────────────────────

async fn seed_reset_token(
    db: &Database,
    user_id: UserId,
) -> sui_id_shared::ids::PasswordResetTokenId {
    let id = sui_id_shared::ids::PasswordResetTokenId::new();
    repos::password_reset_tokens::insert(
        db,
        &crate::models::PasswordResetTokenRow {
            id,
            user_id,
            token_hash: b"token-hash-placeholder".to_vec(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + TimeDelta::hours(1),
            consumed_at: None,
            requester_ip: None,
        },
    )
    .await
    .expect("seed reset token");
    id
}

#[tokio::test]
async fn u10_completion_swaps_credential_consumes_token_and_revokes_everything() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let token_id = seed_reset_token(&db, user.id).await;
    let session_id = seed_active_session(&db, user.id).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    let consumed_at = Utc::now();
    let audited = consume_and_reset_password(&db, user.id, token_id, new_credential, consumed_at)
        .await
        .expect("complete reset");
    audited.into_inner();

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(cred.password_hash, "new-hash-placeholder");

    let token = repos::password_reset_tokens::find_by_hash(&db, b"token-hash-placeholder")
        .await
        .expect("find token")
        .expect("token still exists");
    assert!(token.consumed_at.is_some(), "the token must be consumed");

    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_some(),
        "forgot-password completion revokes every session, unlike self-change"
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "auth.password.reset_completed");
    assert_eq!(
        tail.actor, None,
        "the token presenter is not an authenticated actor"
    );
    assert_eq!(tail.target.as_deref(), Some(user.id.to_string().as_str()));
}

#[tokio::test]
async fn u10_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let token_id = seed_reset_token(&db, user.id).await;
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    db.fault_injector().fail_before_next_append();
    let result =
        consume_and_reset_password(&db, user.id, token_id, new_credential, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(
        cred.password_hash, "old-hash-placeholder",
        "the credential swap rolled back"
    );
    let token = repos::password_reset_tokens::find_by_hash(&db, b"token-hash-placeholder")
        .await
        .expect("find token")
        .expect("token still exists");
    assert!(token.consumed_at.is_none(), "the token consume rolled back");
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_none(),
        "the revocation rolled back too"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}
