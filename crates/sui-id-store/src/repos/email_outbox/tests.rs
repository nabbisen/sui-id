#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_copy)]
use super::*;
use crate::Database;
use chrono::Utc;

fn fresh_db() -> Database {
    let key = crate::crypto::MasterKey::generate();
    Database::open_in_memory(key).expect("in-memory db")
}

fn sample_row() -> EmailOutboxRow {
    let now = Utc::now();
    EmailOutboxRow {
        id: EmailOutboxId::new(),
        state: EmailOutboxState::Queued,
        template: "forgot_password".into(),
        recipient_enc: vec![0u8; 32],
        payload_enc: vec![1u8; 64],
        attempt_count: 0,
        next_attempt_at: now,
        last_error: None,
        locale: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn enqueue_and_claim_round_trip() {
    let db = fresh_db();
    let row = sample_row();
    let id = enqueue(&db, row.clone()).await.expect("enqueue");
    assert_eq!(id, row.id);

    let claimed = claim_one_eligible(&db, Utc::now())
        .await
        .expect("claim")
        .expect("some");
    assert_eq!(claimed.id, id);
    assert_eq!(claimed.state, EmailOutboxState::Sending);
}

#[tokio::test]
async fn claim_respects_next_attempt_at() {
    let db = fresh_db();
    let mut row = sample_row();
    // Schedule 1 hour in the future
    row.next_attempt_at = Utc::now() + chrono::Duration::hours(1);
    enqueue(&db, row).await.expect("enqueue");

    let claimed = claim_one_eligible(&db, Utc::now()).await.expect("claim");
    assert!(claimed.is_none(), "should not be eligible yet");
}

#[tokio::test]
async fn mark_sent_after_claim() {
    let db = fresh_db();
    let row = sample_row();
    let id = enqueue(&db, row).await.expect("enqueue");
    claim_one_eligible(&db, Utc::now())
        .await
        .expect("claim")
        .expect("some");
    mark_sent(&db, id.clone(), Utc::now())
        .await
        .expect("mark sent");

    // Should not appear as eligible anymore
    let claimed2 = claim_one_eligible(&db, Utc::now()).await.expect("claim2");
    assert!(claimed2.is_none(), "sent rows must not be re-claimed");
}

#[tokio::test]
async fn record_failure_increments_attempt_count() {
    let db = fresh_db();
    let row = sample_row();
    let id = enqueue(&db, row).await.expect("enqueue");
    claim_one_eligible(&db, Utc::now())
        .await
        .expect("claim")
        .expect("some");

    let next_try = Utc::now() + chrono::Duration::seconds(30);
    record_failure(
        &db,
        id.clone(),
        "connection refused".into(),
        next_try,
        Utc::now(),
    )
    .await
    .expect("record failure");

    // Advance to after next_try
    let claimed2 = claim_one_eligible(&db, next_try + chrono::Duration::seconds(1))
        .await
        .expect("claim2")
        .expect("some");
    assert_eq!(claimed2.attempt_count, 1);
}

#[tokio::test]
async fn requeue_stuck_sending_resets_old_rows() {
    let db = fresh_db();
    let row = sample_row();
    enqueue(&db, row).await.expect("enqueue");
    claim_one_eligible(&db, Utc::now())
        .await
        .expect("claim")
        .expect("some");
    // Threshold is in the future: any sending row older than it gets reset.
    let threshold = Utc::now() + chrono::Duration::seconds(1);
    let n = requeue_stuck_sending(&db, threshold, Utc::now())
        .await
        .expect("requeue");
    assert_eq!(n, 1);
    // Now it should be claimable again
    let claimed = claim_one_eligible(&db, Utc::now()).await.expect("claim2");
    assert!(claimed.is_some());
}
