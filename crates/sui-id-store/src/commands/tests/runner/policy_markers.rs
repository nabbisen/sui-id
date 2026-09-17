use super::*;

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
