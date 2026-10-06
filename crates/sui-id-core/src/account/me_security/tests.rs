use super::*;
use sui_id_shared::ids::UserId;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::UserRow;
use sui_id_store::repos::{audit, users};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

async fn create_user_with_password(db: &Database, password: &str) -> UserId {
    let id = UserId::new();
    let now = Utc::now();
    users::create(
        db,
        &UserRow {
            id,
            username: "alice".into(),
            display_name: None,
            is_admin: true,
            role: if true {
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
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("create user");
    let phc = password::hash_password(password).await.expect("hash");
    credentials::upsert(
        db,
        &CredentialRow {
            user_id: id,
            password_hash: phc,
            updated_at: now,
        },
    )
    .await
    .expect("set credential");
    id
}

fn self_actor_for(user_id: UserId) -> crate::actor::SelfActor {
    crate::actor::Actor::from_session(
        user_id,
        sui_id_store::models::Role::User,
        sui_id_shared::ids::SessionId::new(),
    )
    .into_self()
}

#[tokio::test]
async fn happy_path_replaces_hash_and_returns_zero_sweep_when_box_unchecked() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user_with_password(&db, "the-old-tester-password").await;
    let actor = self_actor_for(uid);
    let r = change_password_self(
        &db,
        &clock,
        None,
        sui_id_store::models::HibpMode::Off,
        &actor,
        "the-old-tester-password",
        "the-new-tester-password",
        None,
        false,
        crate::security::SecurityLevel::Standard.password_min_len(),
    )
    .await
    .expect("change");
    assert_eq!(r.sessions_revoked, 0);
    assert_eq!(r.refresh_tokens_revoked, 0);
    // Old password no longer verifies; new one does.
    let stored = credentials::get(&db, uid)
        .await
        .expect("cred")
        .password_hash;
    assert!(
        password::verify_password("the-old-tester-password", &stored)
            .await
            .is_err()
    );
    assert!(
        password::verify_password("the-new-tester-password", &stored)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn wrong_current_password_is_rejected_as_invalid_credentials() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user_with_password(&db, "the-old-tester-password").await;
    let actor = self_actor_for(uid);
    let r = change_password_self(
        &db,
        &clock,
        None,
        sui_id_store::models::HibpMode::Off,
        &actor,
        "wrong-current-tester-password",
        "the-new-tester-password",
        None,
        false,
        crate::security::SecurityLevel::Standard.password_min_len(),
    )
    .await;
    assert!(matches!(r, Err(CoreError::InvalidCredentials)));
    // Stored hash is unchanged.
    let stored = credentials::get(&db, uid)
        .await
        .expect("cred")
        .password_hash;
    assert!(
        password::verify_password("the-old-tester-password", &stored)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn weak_new_password_is_rejected_after_current_is_verified() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user_with_password(&db, "the-old-tester-password").await;
    let actor = self_actor_for(uid);
    let r = change_password_self(
        &db,
        &clock,
        None,
        sui_id_store::models::HibpMode::Off,
        &actor,
        "the-old-tester-password",
        "short",
        None,
        false,
        crate::security::SecurityLevel::Standard.password_min_len(),
    )
    .await;
    assert!(matches!(r, Err(CoreError::BadRequest(_))), "{r:?}");
    // Stored hash unchanged — failure must not partially apply.
    let stored = credentials::get(&db, uid)
        .await
        .expect("cred")
        .password_hash;
    assert!(
        password::verify_password("the-old-tester-password", &stored)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn audit_event_is_appended() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user_with_password(&db, "the-old-tester-password").await;
    let actor = self_actor_for(uid);
    change_password_self(
        &db,
        &clock,
        None,
        sui_id_store::models::HibpMode::Off,
        &actor,
        "the-old-tester-password",
        "the-new-tester-password",
        None,
        false,
        crate::security::SecurityLevel::Standard.password_min_len(),
    )
    .await
    .expect("change");
    let rows = audit::recent(&db, 50).await.expect("audit");
    assert!(
        rows.iter()
            .any(|r| r.action == "auth.password.changed_self"),
        "expected auth.password.changed_self in audit log; got: {:?}",
        rows.iter().map(|r| r.action.as_str()).collect::<Vec<_>>()
    );
}
