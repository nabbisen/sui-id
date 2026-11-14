use super::*;
use crate::Database;
use crate::crypto::MasterKey;

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

fn row(jti: &str, exp: DateTime<Utc>) -> RevokedAccessTokenRow {
    RevokedAccessTokenRow {
        jti: jti.to_owned(),
        revoked_at: Utc::now(),
        exp,
        revoked_by_user: None,
        revoked_by_client: None,
    }
}

// ── O03: purge_expired removes only rows whose exp has passed ──────────────

#[tokio::test]
async fn purge_expired_removes_only_rows_whose_exp_has_passed() {
    let db = open_test_db();
    let expired = row("expired-jti", Utc::now() - chrono::Duration::days(2));
    insert(&db, &expired).await.unwrap();
    let live = row("live-jti", Utc::now() + chrono::Duration::days(2));
    insert(&db, &live).await.unwrap();

    let n = purge_expired(&db).await.unwrap();
    assert_eq!(n, 1);

    assert!(
        !is_revoked(&db, "expired-jti").await.unwrap(),
        "an expired entry must be purged"
    );
    assert!(
        is_revoked(&db, "live-jti").await.unwrap(),
        "an unexpired entry must be left alone"
    );
}
