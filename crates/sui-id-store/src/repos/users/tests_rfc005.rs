use super::*;
use crate::{Database, crypto::MasterKey};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

#[tokio::test]
async fn upsert_ldap_shadow_creates_on_first_signin() {
    let db = fresh_db();
    let data = LdapShadowData {
        username: "alice".into(),
        display_name: Some("Alice Test".into()),
        email: Some("alice@example.com".into()),
        external_stable_id: "uid=alice,dc=test".into(),
    };
    let id = upsert_ldap_shadow(&db, data, chrono::Utc::now())
        .await
        .expect("upsert");
    let row =
        find_by_external_stable_id(&db, &crate::models::UserSource::Ldap, "uid=alice,dc=test")
            .await
            .expect("find");
    assert_eq!(row.id, id);
    assert_eq!(row.source, crate::models::UserSource::Ldap);
    assert_eq!(row.external_stable_id.as_deref(), Some("uid=alice,dc=test"));
    assert_eq!(row.display_name.as_deref(), Some("Alice Test"));
    assert_eq!(row.email.as_deref(), Some("alice@example.com"));
}

#[tokio::test]
async fn upsert_ldap_shadow_updates_on_second_signin() {
    let db = fresh_db();
    let now = chrono::Utc::now();
    upsert_ldap_shadow(
        &db,
        LdapShadowData {
            username: "alice".into(),
            display_name: Some("Old Name".into()),
            email: Some("old@example.com".into()),
            external_stable_id: "uid=alice,dc=test".into(),
        },
        now,
    )
    .await
    .expect("first");
    upsert_ldap_shadow(
        &db,
        LdapShadowData {
            username: "alice".into(),
            display_name: Some("New Name".into()),
            email: Some("new@example.com".into()),
            external_stable_id: "uid=alice,dc=test".into(),
        },
        now + chrono::Duration::seconds(1),
    )
    .await
    .expect("second");
    let row =
        find_by_external_stable_id(&db, &crate::models::UserSource::Ldap, "uid=alice,dc=test")
            .await
            .expect("find");
    assert_eq!(row.display_name.as_deref(), Some("New Name"));
    assert_eq!(row.email.as_deref(), Some("new@example.com"));
}

#[tokio::test]
async fn shadow_row_source_is_ldap() {
    let db = fresh_db();
    let _ = upsert_ldap_shadow(
        &db,
        LdapShadowData {
            username: "bob".into(),
            display_name: None,
            email: None,
            external_stable_id: "uid=bob,dc=test".into(),
        },
        chrono::Utc::now(),
    )
    .await
    .expect("upsert");
    let row = find_by_external_stable_id(&db, &crate::models::UserSource::Ldap, "uid=bob,dc=test")
        .await
        .expect("find");
    assert_eq!(row.source, crate::models::UserSource::Ldap);
    assert!(row.external_stable_id.is_some());
}

#[tokio::test]
async fn find_by_external_stable_id_not_found_for_unknown() {
    let db = fresh_db();
    let err = find_by_external_stable_id(&db, &crate::models::UserSource::Ldap, "nonexistent")
        .await
        .expect_err("must be NotFound");
    assert!(matches!(err, crate::StoreError::NotFound));
}
