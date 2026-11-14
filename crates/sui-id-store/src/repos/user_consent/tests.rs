use super::*;
use crate::crypto::MasterKey;
use crate::models::{ClientRow, ConsentPolicy, UserRow};

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

async fn seed_client(db: &Database) -> ClientId {
    let id = ClientId::new();
    let now = Utc::now();
    crate::repos::clients::create(
        db,
        &ClientRow {
            id,
            name: format!("client-{id}"),
            confidential: false,
            secret_hash: None,
            redirect_uris: vec!["https://example.com/cb".into()],
            allowed_scopes: String::new(),
            post_logout_redirect_uris: vec![],
            is_disabled: false,
            is_deleted: false,
            consent_policy: ConsentPolicy::default(),
            registered_via: crate::models::RegistrationSource::default(),
            logo_uri: None,
            homepage_uri: None,
            privacy_policy_uri: None,
            tos_uri: None,
            token_endpoint_auth_method: None,
            grant_types: None,
            response_types: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();
    id
}

/// Raw read -- `get()`'s own `UserConsentRow` does not carry
/// `last_used_at` (only `list_for_user`'s joined `ConsentGrantView`
/// does), so this reads the column directly.
fn last_used_at(db: &Database, user_id: UserId, client_id: ClientId) -> Option<DateTime<Utc>> {
    db.with_conn_sync(move |conn| {
        Ok(conn.query_row(
            "SELECT last_used_at FROM user_consent WHERE user_id = ?1 AND client_id = ?2",
            rusqlite::params![user_id.to_string(), client_id.to_string()],
            |r| r.get(0),
        )?)
    })
    .unwrap()
}

// ── U36: touch_last_used touches only (user_id, client_id) ──────────────────

#[tokio::test]
async fn touch_last_used_touches_only_its_own_pair() {
    let db = open_test_db();
    let user = seed_user(&db).await;
    let other_user = seed_user(&db).await;
    let client = seed_client(&db).await;
    let other_client = seed_client(&db).await;

    // Three grants, seeded directly (raw SQL, not `upsert_within_tx`,
    // which needs a caller-held transaction this test has no reason to
    // open): the pair under test, same user + another client, and
    // another user + the same client -- one clause's boundary each.
    let pairs = [(user, client), (user, other_client), (other_user, client)];
    for (u, c) in pairs {
        db.with_conn_sync(move |conn| {
            conn.execute(
                "INSERT INTO user_consent (user_id, client_id, granted_scopes, granted_at, last_used_at) \
                 VALUES (?1, ?2, 'openid', ?3, NULL)",
                rusqlite::params![u.to_string(), c.to_string(), Utc::now()],
            )?;
            Ok(())
        })
        .unwrap();
    }

    let now = Utc::now();
    touch_last_used(&db, user, client, now).await.unwrap();

    assert_eq!(
        last_used_at(&db, user, client),
        Some(now),
        "the named pair must be touched with exactly the given instant"
    );
    assert_eq!(
        last_used_at(&db, user, other_client),
        None,
        "same user, a different client: must be left alone"
    );
    assert_eq!(
        last_used_at(&db, other_user, client),
        None,
        "same client, a different user: must be left alone"
    );
}

#[test]
fn covers_returns_true_when_all_requested_are_granted() {
    assert!(covers("openid profile email", "openid profile"));
    assert!(covers("openid profile email", "openid"));
    assert!(covers("openid profile email", "openid profile email"));
}

#[test]
fn covers_returns_false_when_new_scope_requested() {
    assert!(!covers("openid profile", "openid profile email"));
    assert!(!covers("openid", "openid offline_access"));
}

#[test]
fn covers_empty_requested_is_always_covered() {
    assert!(covers("openid", ""));
    assert!(covers("", ""));
}
