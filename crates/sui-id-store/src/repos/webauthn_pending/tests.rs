use super::*;
use crate::Database;
use crate::crypto::MasterKey;
use crate::models::UserRow;

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

async fn seed_user(db: &Database) -> UserId {
    let id = UserId::new();
    let now = Utc::now();
    crate::repos::users::create(
        db,
        &UserRow {
            id,
            username: id.to_string(),
            display_name: None,
            is_admin: false,
            role: crate::models::Role::User,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
            last_login_at: None,
            source: crate::models::UserSource::Local,
            external_stable_id: None,
        },
    )
    .await
    .unwrap();
    id
}

fn pending_row(user_id: UserId, expires_at: DateTime<Utc>) -> WebauthnPendingRow {
    WebauthnPendingRow {
        id: WebauthnPendingId::new(),
        kind: WebauthnPendingKind::Authenticate,
        user_id: Some(user_id),
        state_json: "{}".into(),
        expires_at,
        created_at: Utc::now(),
    }
}

// ── U33: delete removes only its own row ────────────────────────────────────

#[tokio::test]
async fn delete_removes_only_its_own_row() {
    let db = open_test_db();
    let user = seed_user(&db).await;
    let target = pending_row(user, Utc::now() + chrono::Duration::hours(1));
    let target_id = target.id;
    insert(&db, &target).await.unwrap();
    let sibling = pending_row(user, Utc::now() + chrono::Duration::hours(1));
    let sibling_id = sibling.id;
    insert(&db, &sibling).await.unwrap();

    delete(&db, target_id).await.unwrap();

    assert!(
        get(&db, target_id).await.unwrap().is_none(),
        "the named row must be gone"
    );
    assert!(
        get(&db, sibling_id).await.unwrap().is_some(),
        "another pending row must be left alone"
    );
}

// ── O03: purge_expired removes only rows whose expiry has passed ───────────

#[tokio::test]
async fn purge_expired_removes_only_rows_whose_expiry_has_passed() {
    let db = open_test_db();
    let user = seed_user(&db).await;
    let expired = pending_row(user, Utc::now() - chrono::Duration::days(2));
    let expired_id = expired.id;
    insert(&db, &expired).await.unwrap();
    let live = pending_row(user, Utc::now() + chrono::Duration::days(2));
    let live_id = live.id;
    insert(&db, &live).await.unwrap();

    let n = purge_expired(&db).await.unwrap();
    assert_eq!(n, 1);

    assert!(
        get(&db, expired_id).await.unwrap().is_none(),
        "an expired row must be purged"
    );
    assert!(
        get(&db, live_id).await.unwrap().is_some(),
        "an unexpired row must be left alone"
    );
}
