use super::*;
use crate::password::hash_password;
use chrono::Utc;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{CredentialRow, UserRow};

fn fresh_db() -> Database {
    let key = MasterKey::generate();
    Database::open_in_memory(key).expect("db")
}

async fn make_user(db: &Database, preferred_lang: Option<&str>) -> UserId {
    let id = UserId::new();
    let now = Utc::now();
    users::create(
        db,
        &UserRow {
            id,
            username: "alice".into(),
            display_name: None,
            is_admin: false,
            role: if false {
                sui_id_store::models::Role::Admin
            } else {
                sui_id_store::models::Role::User
            },
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: sui_id_store::models::UserSource::Local,
            external_stable_id: None,
            email: None,
            preferred_lang: preferred_lang.map(str::to_owned),
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("user");
    let _ = hash_password("alice-the-tester-password").await;
    let _ = CredentialRow {
        user_id: id,
        password_hash: String::new(),
        updated_at: now,
    };
    id
}

#[tokio::test]
async fn user_preference_wins() {
    let db = fresh_db();
    let uid = make_user(&db, Some("en")).await;
    let loc = resolve(
        &db,
        &LocaleInputs {
            user_id: Some(uid),
            cookie: Some("ja"),
            accept_language: Some("ja"),
        },
    )
    .await
    .expect("resolve");
    assert_eq!(loc, Locale::En);
}

#[tokio::test]
async fn cookie_wins_over_accept_language() {
    let db = fresh_db();
    let loc = resolve(
        &db,
        &LocaleInputs {
            user_id: None,
            cookie: Some("en"),
            accept_language: Some("ja"),
        },
    )
    .await
    .expect("resolve");
    assert_eq!(loc, Locale::En);
}

#[tokio::test]
async fn accept_language_used_when_no_user_or_cookie() {
    let db = fresh_db();
    let loc = resolve(
        &db,
        &LocaleInputs {
            user_id: None,
            cookie: None,
            accept_language: Some("en-US,en;q=0.9"),
        },
    )
    .await
    .expect("resolve");
    assert_eq!(loc, Locale::En);
}

#[tokio::test]
async fn falls_back_to_server_default_when_nothing_matches() {
    let db = fresh_db();
    // Migration default is "ja"; nothing else matches.
    let loc = resolve(
        &db,
        &LocaleInputs {
            user_id: None,
            cookie: None,
            accept_language: Some("xx"), // unknown locale → fall back
        },
    )
    .await
    .expect("resolve");
    assert_eq!(loc, Locale::Ja);
}

#[tokio::test]
async fn user_preference_with_unknown_tag_falls_through() {
    // After a hypothetical downgrade, a row could hold a tag
    // we don't recognise. Resolution should not error; it
    // should fall through to subsequent tiers.
    let db = fresh_db();
    // Use a locale tag that will remain unknown even after zh is added.
    let uid = make_user(&db, Some("xx-UnknownLocale")).await;
    let loc = resolve(
        &db,
        &LocaleInputs {
            user_id: Some(uid),
            cookie: Some("en"),
            accept_language: None,
        },
    )
    .await
    .expect("resolve");
    assert_eq!(loc, Locale::En);
}
