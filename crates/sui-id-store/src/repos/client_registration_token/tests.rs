use super::*;
use crate::{Database, crypto::MasterKey};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

fn sample_row(max_uses: i64, expires_at: Option<DateTime<Utc>>) -> RegistrationTokenRow {
    let now = chrono::Utc::now();
    RegistrationTokenRow {
        id: RegistrationTokenId::new(),
        token_hash: "abc123".into(),
        max_uses,
        used_count: 0,
        expires_at,
        revoked_at: None,
        note: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn consume_valid_token_returns_true() {
    let db = fresh_db();
    create(&db, &sample_row(0, None)).await.unwrap();
    let ok = consume(&db, "abc123", chrono::Utc::now()).await.unwrap();
    assert!(ok);
    let row = find_by_hash(&db, "abc123").await.unwrap().unwrap();
    assert_eq!(row.used_count, 1);
}

#[tokio::test]
async fn consume_unknown_token_returns_false() {
    let db = fresh_db();
    let ok = consume(&db, "nosuchtoken", chrono::Utc::now())
        .await
        .unwrap();
    assert!(!ok, "unknown token must return false");
}

#[tokio::test]
async fn consume_exhausted_token_returns_false() {
    let db = fresh_db();
    create(&db, &sample_row(1, None)).await.unwrap();
    assert!(consume(&db, "abc123", chrono::Utc::now()).await.unwrap());
    // Second use: should be rejected (max_uses = 1)
    assert!(!consume(&db, "abc123", chrono::Utc::now()).await.unwrap());
}

#[tokio::test]
async fn revoke_blocks_consume() {
    let db = fresh_db();
    let row = sample_row(0, None);
    let id = row.id;
    create(&db, &row).await.unwrap();
    revoke(&db, id, chrono::Utc::now()).await.unwrap();
    assert!(!consume(&db, "abc123", chrono::Utc::now()).await.unwrap());
}
