use super::*;

#[tokio::test]
async fn u30_protocol_inserts_session_with_no_audit_row() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");

    let before = latest_audit_action(&db).await;

    let session = SessionRow {
        id: sui_id_shared::ids::SessionId::new(),
        user_id: user.id,
        expires_at: Utc::now() + TimeDelta::hours(1),
        created_at: Utc::now(),
        revoked_at: None,
        auth_methods: vec![],
        last_step_up_at: None,
        last_used_at: Some(Utc::now()),
    };
    insert_session(&db, session.clone())
        .await
        .expect("insert session");

    let fetched = repos::sessions::get(&db, session.id)
        .await
        .expect("get session");
    assert_eq!(fetched.id, session.id);

    // Protocol commands are not the tamper-evident chain -- no new
    // audit row, by construction (there is no code path from
    // `Database::protocol` to `audit::append_within_tx`).
    assert_eq!(latest_audit_action(&db).await, before);
}

#[tokio::test]
async fn o01_operational_enqueues_email_with_no_audit_row() {
    let db = fresh_db();
    let before = latest_audit_action(&db).await;

    let row = EmailOutboxRow {
        id: sui_id_shared::ids::EmailOutboxId::new(),
        state: EmailOutboxState::Queued,
        template: "forgot_password".into(),
        recipient_enc: vec![1, 2, 3],
        payload_enc: vec![4, 5, 6],
        attempt_count: 0,
        next_attempt_at: Utc::now(),
        last_error: None,
        locale: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    enqueue_email(&db, row).await.expect("enqueue");

    assert_eq!(latest_audit_action(&db).await, before);
}
