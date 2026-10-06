use super::*;
use crate::time::system_clock;
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::UserRow;
use sui_id_store::repos::users;

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
async fn enroll_then_confirm_completes_and_returns_8_recovery_codes() {
    let (db, uid) = fresh_db_with_user().await;
    let clock = system_clock();
    let ticket = start_enrollment(&db, "sui-id", uid, "alice")
        .await
        .expect("start");
    assert_eq!(ticket.secret.len(), 20);
    let now = clock.now().timestamp();
    let step = now / 30;
    let code = crate::totp::code_for_step(&ticket.secret, step).await;
    let codes = confirm_enrollment(&db, &clock, uid, code)
        .await
        .expect("confirm");
    assert_eq!(codes.len(), 8);
    // The user should now report MFA enabled.
    assert!(is_mfa_enabled(&db, uid).await.unwrap());
}

#[tokio::test]
async fn confirm_with_wrong_code_returns_bad_request() {
    let (db, uid) = fresh_db_with_user().await;
    let clock = system_clock();
    let _ = start_enrollment(&db, "sui-id", uid, "alice")
        .await
        .expect("start");
    let r = confirm_enrollment(&db, &clock, uid, 000000).await;
    assert!(matches!(r, Err(crate::CoreError::BadRequest(_))));
}

#[tokio::test]
async fn disable_then_re_enroll_works() {
    let (db, uid) = fresh_db_with_user().await;
    let clock = system_clock();
    let ticket = start_enrollment(&db, "sui-id", uid, "alice")
        .await
        .expect("start");
    let step = clock.now().timestamp() / 30;
    let code = crate::totp::code_for_step(&ticket.secret, step).await;
    let _ = confirm_enrollment(&db, &clock, uid, code)
        .await
        .expect("confirm");
    disable(&db, uid).await.expect("disable");
    assert!(!is_mfa_enabled(&db, uid).await.unwrap());
    // Re-enrol from scratch should succeed.
    let _ = start_enrollment(&db, "sui-id", uid, "alice")
        .await
        .expect("re-start");
}
