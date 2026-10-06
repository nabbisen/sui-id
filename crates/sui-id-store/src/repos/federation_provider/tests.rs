use super::*;
use crate::{Database, crypto::MasterKey};
use chrono::Utc;

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

fn sample(slug: &str) -> FederationProviderRow {
    let now = Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Test IdP".into(),
        issuer: "https://idp.example.com".into(),
        client_id: "client-abc".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: false,
        allowed_origins: String::new(),
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn create_and_get_by_slug() {
    let db = fresh_db();
    let row = sample("google");
    create(&db, &row, Some("supersecret")).await.unwrap();
    let fetched = get_by_slug(&db, "google").await.unwrap();
    assert_eq!(fetched.slug, "google");
    assert!(fetched.client_secret_enc.is_some());
    let plain = decrypt_secret(db.key(), &fetched).unwrap();
    assert_eq!(plain.as_deref(), Some("supersecret"));
}

#[tokio::test]
async fn list_enabled_filters_disabled() {
    let db = fresh_db();
    create(&db, &sample("g1"), None).await.unwrap();
    let mut row2 = sample("g2");
    row2.enabled = true;
    create(&db, &row2, None).await.unwrap();
    let enabled = list_enabled(&db).await.unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].slug, "g2");
}

#[tokio::test]
async fn update_allowed_origins_touches_only_that_column() {
    let db = fresh_db();
    let row = sample("origin-test");
    let id = row.id;
    create(&db, &row, None).await.unwrap();
    update_allowed_origins(&db, id, "https://a.example https://b.example", Utc::now())
        .await
        .unwrap();
    let fetched = get(&db, id).await.unwrap();
    assert_eq!(
        fetched.allowed_origins,
        "https://a.example https://b.example"
    );
    // Every other column is unchanged from what `create` wrote.
    assert_eq!(fetched.display_name, row.display_name);
    assert_eq!(fetched.issuer, row.issuer);
    assert_eq!(fetched.client_id, row.client_id);
    assert_eq!(fetched.scopes, row.scopes);
    assert_eq!(fetched.provision_mode, row.provision_mode);
    assert_eq!(fetched.enabled, row.enabled);
}

#[tokio::test]
async fn update_allowed_origins_on_missing_id_is_not_found() {
    let db = fresh_db();
    let bogus = FederationProviderId::new();
    assert!(matches!(
        update_allowed_origins(&db, bogus, "https://a.example", Utc::now()).await,
        Err(StoreError::NotFound)
    ));
}

#[tokio::test]
async fn set_enabled_and_delete() {
    let db = fresh_db();
    let row = sample("test");
    let id = row.id;
    create(&db, &row, None).await.unwrap();
    set_enabled(&db, id, true, Utc::now()).await.unwrap();
    let fetched = get(&db, id).await.unwrap();
    assert!(fetched.enabled);
    delete(&db, id).await.unwrap();
    assert!(matches!(get(&db, id).await, Err(StoreError::NotFound)));
}
