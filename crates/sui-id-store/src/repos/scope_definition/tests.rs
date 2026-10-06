use super::*;
use crate::{Database, crypto::MasterKey};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

#[tokio::test]
async fn minimum_catalog_seeded_by_migration() {
    let db = fresh_db();
    let scopes = list(&db).await.unwrap();
    let names: Vec<&str> = scopes.iter().map(|s| s.name.as_str()).collect();
    for required in &["openid", "profile", "email", "offline_access"] {
        assert!(
            names.contains(required),
            "scope {required} missing from catalog"
        );
    }
}

#[tokio::test]
async fn openid_does_not_require_consent() {
    let db = fresh_db();
    let openid = get(&db, "openid").await.unwrap();
    assert!(!openid.requires_consent, "openid must not require consent");
}

#[tokio::test]
async fn create_and_delete_scope() {
    let db = fresh_db();
    let row = ScopeDefinitionRow {
        name: "custom:read".into(),
        requires_consent: true,
        is_default: false,
        created_at: chrono::Utc::now(),
    };
    create(&db, &row).await.unwrap();
    let fetched = get(&db, "custom:read").await.unwrap();
    assert_eq!(fetched.name, "custom:read");
    assert!(fetched.requires_consent);
    delete(&db, "custom:read").await.unwrap();
    assert!(matches!(
        get(&db, "custom:read").await,
        Err(StoreError::NotFound)
    ));
}

#[tokio::test]
async fn consented_names_excludes_openid() {
    let db = fresh_db();
    let names = consented_names(&db).await.unwrap();
    assert!(
        !names.contains("openid"),
        "openid must not be in consented set"
    );
    assert!(
        names.contains("profile"),
        "profile must be in consented set"
    );
    assert!(names.contains("email"), "email must be in consented set");
}
