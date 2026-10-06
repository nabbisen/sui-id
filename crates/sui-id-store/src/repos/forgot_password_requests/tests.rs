#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;

fn fresh_db() -> Database {
    let key = crate::crypto::MasterKey::generate();
    Database::open_in_memory(key).expect("in-memory db")
}

#[tokio::test]
async fn o_o05_record_is_durable_and_outstanding_until_claimed() {
    let db = fresh_db();
    let now = Utc::now();
    record(
        &db,
        ForgotPasswordRequestId::new(),
        "alice@example.test".into(),
        Some("203.0.113.1".into()),
        now,
    )
    .await
    .expect("record");
    assert_eq!(count_outstanding(&db).await.expect("count"), 1);
}

#[tokio::test]
async fn o_o06_claim_marks_processing_and_requeue_resets_it() {
    let db = fresh_db();
    let now = Utc::now();
    let id = ForgotPasswordRequestId::new();
    record(&db, id, "bob@example.test".into(), None, now)
        .await
        .expect("record");

    let claimed = claim_one(&db, now).await.expect("claim").expect("some");
    assert_eq!(claimed.id, id);
    assert_eq!(claimed.email, "bob@example.test");
    // Still outstanding — claiming is not finishing.
    assert_eq!(count_outstanding(&db).await.expect("count"), 1);
    // Not claimable again while processing.
    assert!(claim_one(&db, now).await.expect("claim2").is_none());

    // Simulating a crash: the row is left `processing`. Startup sweep
    // resets it, and it becomes claimable again.
    let n = requeue_stuck_processing(&db, now).await.expect("requeue");
    assert_eq!(n, 1);
    let reclaimed = claim_one(&db, now).await.expect("claim3").expect("some");
    assert_eq!(reclaimed.id, id);

    delete(&db, id).await.expect("delete");
    assert_eq!(count_outstanding(&db).await.expect("count"), 0);
}

#[tokio::test]
async fn claim_one_returns_none_when_empty() {
    let db = fresh_db();
    assert!(claim_one(&db, Utc::now()).await.expect("claim").is_none());
}
