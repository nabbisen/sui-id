use super::*;
use crate::time::system_clock;
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::UserRow;
use sui_id_store::repos::{users, webauthn_pending};

async fn fresh_db_with_user() -> (Database, UserId) {
    let key = MasterKey::generate();
    let db = Database::open_in_memory(key).expect("db");
    let uid = UserId::new();
    users::create(
        &db,
        &UserRow {
            id: uid,
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
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
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
    .expect("insert user");
    (db, uid)
}

#[tokio::test]
async fn start_registration_persists_pending_row_and_returns_challenge_json() {
    let (db, uid) = fresh_db_with_user().await;
    let clock = system_clock();
    let started = start_registration(&db, &clock, "https://idp.example", uid)
        .await
        .expect("start");
    // Pending row must exist and be of kind Register.
    let row = webauthn_pending::get(&db, started.pending_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(
        row.kind,
        sui_id_store::models::WebauthnPendingKind::Register
    );
    assert_eq!(row.user_id, Some(uid));
    // Challenge JSON should parse and contain a publicKey.challenge.
    let v: serde_json::Value = serde_json::from_str(&started.challenge_json).expect("json");
    assert!(v.get("publicKey").is_some(), "got: {v}");
}

#[tokio::test]
async fn start_authentication_rejects_users_with_no_credentials() {
    let (db, uid) = fresh_db_with_user().await;
    let clock = system_clock();
    let r = start_authentication(
        &db,
        &clock,
        "https://idp.example",
        uid,
        WebauthnPendingKind::Authenticate,
    )
    .await;
    assert!(matches!(r, Err(crate::errors::CoreError::BadRequest(_))));
}

#[tokio::test]
async fn finish_registration_rejects_expired_pending_row() {
    // Manufacture a pending row that has already expired and verify
    // finish_registration refuses it (returns Unauthenticated).
    use sui_id_store::models::{WebauthnPendingKind, WebauthnPendingRow};
    let (db, uid) = fresh_db_with_user().await;
    let clock = system_clock();
    let now = clock.now();
    let pending_id = sui_id_shared::ids::WebauthnPendingId::new();
    webauthn_pending::insert(
        &db,
        &WebauthnPendingRow {
            id: pending_id,
            kind: WebauthnPendingKind::Register,
            user_id: Some(uid),
            state_json: "{}".into(),
            expires_at: now - chrono::Duration::seconds(1),
            created_at: now - chrono::Duration::seconds(601),
        },
    )
    .await
    .expect("insert");
    // Build a dummy credential JSON; we never get past the expiry
    // check, so its content does not matter — but it must
    // syntactically deserialise (the `rawId`/binary fields parse as
    // base64url-no-pad).
    let dummy: webauthn_rs::prelude::RegisterPublicKeyCredential = serde_json::from_str(
            r#"{"id":"AA","rawId":"AA","type":"public-key","response":{"attestationObject":"AA","clientDataJSON":"AA"},"extensions":{}}"#,
        )
        .expect("parse dummy");
    let r = finish_registration(
        &db,
        &clock,
        "https://idp.example",
        pending_id,
        uid,
        "test",
        &dummy,
    )
    .await;
    assert!(matches!(r, Err(crate::errors::CoreError::Unauthenticated)));
}
