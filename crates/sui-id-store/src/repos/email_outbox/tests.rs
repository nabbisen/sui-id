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

/// Raw read, using the module's own private `map` -- there is no public
/// getter for a single row by id.
fn fetch(db: &Database, id: EmailOutboxId) -> EmailOutboxRow {
    db.with_conn_sync(move |conn| {
        Ok(conn.query_row(
            "SELECT id, state, template, recipient_enc, payload_enc, \
                    attempt_count, next_attempt_at, last_error, locale, created_at, updated_at \
             FROM email_outbox WHERE id = ?1",
            rusqlite::params![id.to_string()],
            map,
        )?)
    })
    .unwrap()
}

// ── O02: mark_permanently_failed touches only its own row ──────────────────

#[tokio::test]
async fn mark_permanently_failed_touches_only_its_own_row() {
    let db = fresh_db();
    let target = sample_row();
    let target_id = enqueue(&db, target).await.expect("enqueue target");
    let sibling = sample_row();
    let sibling_id = enqueue(&db, sibling).await.expect("enqueue sibling");
    let sibling_before = fetch(&db, sibling_id);

    let now = Utc::now();
    mark_permanently_failed(&db, target_id, "smtp gave up".into(), now)
        .await
        .expect("mark permanently failed");

    let target_after = fetch(&db, target_id);
    assert_eq!(target_after.state, EmailOutboxState::Failed);
    assert_eq!(target_after.attempt_count, 1);
    assert_eq!(target_after.last_error, Some("smtp gave up".to_string()));
    assert_eq!(target_after.updated_at, now);

    let sibling_after = fetch(&db, sibling_id);
    assert_eq!(sibling_after.state, sibling_before.state);
    assert_eq!(sibling_after.attempt_count, sibling_before.attempt_count);
    assert_eq!(sibling_after.last_error, sibling_before.last_error);
    assert_eq!(
        sibling_after.updated_at, sibling_before.updated_at,
        "a different row must not be touched"
    );
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
