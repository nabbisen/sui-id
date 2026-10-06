use super::*;
use crate::{
    Database,
    crypto::MasterKey,
    models::{FederationProviderRow, ProvisionMode},
    repos::federation_provider,
};
use chrono::Utc;

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

async fn seed_provider(db: &Database) -> FederationProviderId {
    let now = Utc::now();
    let row = FederationProviderRow {
        id: FederationProviderId::new(),
        slug: "test-idp".into(),
        display_name: "Test".into(),
        issuer: "https://idp.example.com".into(),
        client_id: "abc".into(),
        client_secret_enc: None,
        scopes: "openid".into(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: true,
        allowed_origins: String::new(),
        created_at: now,
        updated_at: now,
    };
    let id = row.id;
    federation_provider::create(db, &row, None).await.unwrap();
    id
}

fn new_user_id() -> UserId {
    UserId::new()
}

#[tokio::test]
async fn upsert_and_find_by_sub() {
    let db = fresh_db();
    let pid = seed_provider(&db).await;
    let uid = new_user_id();
    let now = Utc::now();
    // Need a user row too (FK)
    // Insert minimal user via raw SQL since users::create needs full row
    db.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO users (id, username, is_admin, role, is_disabled, is_deleted, \
                 user_uuid, failed_login_count, source, created_at, updated_at) \
                 VALUES (?1,'fed_user',0,'user',0,0,'00000000-0000-0000-0000-000000000001',0,'local',?2,?2)",
                params![uid.to_string(), now],
            ).map(|_| ()).map_err(crate::errors::StoreError::from)
        }).await.unwrap();

    let link = FederationLinkRow {
        user_id: uid,
        provider_id: pid,
        upstream_sub: "sub-abc".into(),
        upstream_email: Some("alice@example.com".into()),
        linked_at: now,
        last_seen_at: now,
    };
    upsert(&db, link).await.unwrap();

    let found = find_by_sub(&db, pid, "sub-abc").await.unwrap();
    assert!(found.is_some());
    assert_eq!(
        found.unwrap().upstream_email.as_deref(),
        Some("alice@example.com")
    );
}

#[tokio::test]
async fn find_by_sub_returns_none_for_unknown() {
    let db = fresh_db();
    let pid = seed_provider(&db).await;
    let result = find_by_sub(&db, pid, "nonexistent-sub").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn upsert_updates_email_on_second_call() {
    let db = fresh_db();
    let pid = seed_provider(&db).await;
    let uid = new_user_id();
    let now = Utc::now();
    db.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO users (id, username, is_admin, role, is_disabled, is_deleted, \
                 user_uuid, failed_login_count, source, created_at, updated_at) \
                 VALUES (?1,'fed_user2',0,'user',0,0,'00000000-0000-0000-0000-000000000002',0,'local',?2,?2)",
                params![uid.to_string(), now],
            ).map(|_| ()).map_err(crate::errors::StoreError::from)
        }).await.unwrap();

    upsert(
        &db,
        FederationLinkRow {
            user_id: uid,
            provider_id: pid,
            upstream_sub: "sub-xyz".into(),
            upstream_email: Some("old@example.com".into()),
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .unwrap();
    upsert(
        &db,
        FederationLinkRow {
            user_id: uid,
            provider_id: pid,
            upstream_sub: "sub-xyz".into(),
            upstream_email: Some("new@example.com".into()),
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .unwrap();

    let found = find_by_sub(&db, pid, "sub-xyz").await.unwrap().unwrap();
    assert_eq!(found.upstream_email.as_deref(), Some("new@example.com"));
}
