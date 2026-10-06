use super::*;
use crate::password;
use chrono::Utc;
use sui_id_shared::ids::UserId;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{CredentialRow, UserRow};
use sui_id_store::repos::{credentials, user_totp, users};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

async fn create_user(db: &Database) -> UserId {
    let id = UserId::new();
    let now = Utc::now();
    users::create(
        db,
        &UserRow {
            id,
            username: "u".into(),
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
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("create user");
    let phc = password::hash_password("the-tester-password")
        .await
        .expect("hash");
    credentials::upsert(
        db,
        &CredentialRow {
            user_id: id,
            password_hash: phc,
            updated_at: now,
        },
    )
    .await
    .expect("cred");
    id
}

async fn enrol_totp(db: &Database, user_id: UserId) {
    // Enrolment goes through pending-then-confirm; for tests we
    // just need an "MFA enrolled" row, so do both halves.
    user_totp::upsert_pending(db, user_id, b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09")
        .await
        .expect("upsert pending");
    user_totp::confirm_with_recovery(db, user_id, b"[]")
        .await
        .expect("confirm");
}

#[tokio::test]
async fn user_with_no_mfa_is_always_allowed() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user(&db).await;
    let r = policy_for_session(&db, &clock, uid, None, STEP_UP_FRESHNESS_SECS)
        .await
        .unwrap();
    assert_eq!(r, StepUpDecision::Allow);
    let r = policy_for_session(
        &db,
        &clock,
        uid,
        Some(Utc::now() - Duration::days(7)),
        STEP_UP_FRESHNESS_SECS,
    )
    .await
    .unwrap();
    assert_eq!(r, StepUpDecision::Allow);
}

#[tokio::test]
async fn mfa_user_with_no_step_up_must_challenge() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user(&db).await;
    enrol_totp(&db, uid).await;
    let r = policy_for_session(&db, &clock, uid, None, STEP_UP_FRESHNESS_SECS)
        .await
        .unwrap();
    assert_eq!(r, StepUpDecision::Challenge);
}

#[tokio::test]
async fn mfa_user_with_fresh_step_up_is_allowed() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user(&db).await;
    enrol_totp(&db, uid).await;
    let now = clock.now();
    let r = policy_for_session(&db, &clock, uid, Some(now), STEP_UP_FRESHNESS_SECS)
        .await
        .unwrap();
    assert_eq!(r, StepUpDecision::Allow);
}

#[tokio::test]
async fn mfa_user_with_stale_step_up_must_challenge_again() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user(&db).await;
    enrol_totp(&db, uid).await;
    let stale = clock.now() - Duration::seconds(STEP_UP_FRESHNESS_SECS + 60);
    let r = policy_for_session(&db, &clock, uid, Some(stale), STEP_UP_FRESHNESS_SECS)
        .await
        .unwrap();
    assert_eq!(r, StepUpDecision::Challenge);
}

async fn fresh_session(db: &Database, clock: &SharedClock, uid: UserId) -> SessionId {
    use sui_id_store::models::SessionRow;
    let session_id = SessionId::new();
    let now = clock.now();
    // RFC 102 A4: a session is created by signing in (L01).
    sui_id_store::commands::sign_in_with_password(
        db,
        SessionRow {
            id: session_id,
            user_id: uid,
            expires_at: now + Duration::hours(8),
            created_at: now,
            revoked_at: None,
            auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
            last_step_up_at: None,
            last_used_at: None,
        },
    )
    .await
    .expect("sign in");
    session_id
}

#[tokio::test]
async fn verify_totp_code_with_correct_code_marks_session_fresh() {
    use crate::totp;
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user(&db).await;
    // Enrol TOTP with a known secret so we can compute the
    // expected code locally.
    let secret = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09";
    user_totp::upsert_pending(&db, uid, secret)
        .await
        .expect("pending");
    user_totp::confirm_with_recovery(&db, uid, b"[]")
        .await
        .expect("confirm");

    let session_id = fresh_session(&db, &clock, uid).await;
    let now = clock.now().timestamp();
    let step = now / 30;
    let code = totp::code_for_step(secret, step).await;

    verify_totp_code(
        &db,
        &clock,
        uid,
        session_id,
        &code.to_string(),
        "/me/security/mfa",
    )
    .await
    .expect("verify ok");

    let row = sui_id_store::repos::sessions::get(&db, session_id)
        .await
        .expect("get");
    assert!(row.last_step_up_at.is_some(), "session should be fresh");
}

#[tokio::test]
async fn verify_totp_code_with_wrong_code_does_not_touch_session() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user(&db).await;
    let secret = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09";
    user_totp::upsert_pending(&db, uid, secret)
        .await
        .expect("pending");
    user_totp::confirm_with_recovery(&db, uid, b"[]")
        .await
        .expect("confirm");

    let session_id = fresh_session(&db, &clock, uid).await;

    // Pass a code that's almost certainly wrong (a fixed value
    // that's unlikely to coincide with the real one — and even
    // if it did, the next pass would still be wrong).
    let result = verify_totp_code(&db, &clock, uid, session_id, "000000", "/me/security/mfa").await;
    assert!(matches!(
        result,
        Err(crate::errors::CoreError::InvalidCredentials)
    ));

    let row = sui_id_store::repos::sessions::get(&db, session_id)
        .await
        .expect("get");
    assert!(
        row.last_step_up_at.is_none(),
        "session must NOT be marked fresh on a failed verify"
    );
}

#[tokio::test]
async fn verify_totp_code_for_user_without_totp_returns_invalid_credentials() {
    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user(&db).await;
    let session_id = fresh_session(&db, &clock, uid).await;

    let result = verify_totp_code(&db, &clock, uid, session_id, "123456", "/me/security/mfa").await;
    // Same error shape as a wrong code: a step-up form should
    // not leak whether MFA is enrolled.
    assert!(matches!(
        result,
        Err(crate::errors::CoreError::InvalidCredentials)
    ));
}

#[tokio::test]
async fn finish_webauthn_refuses_pending_with_wrong_kind() {
    // A pending row tagged `Authenticate` (i.e. a login-MFA
    // ceremony) must NOT satisfy a step-up gate, even if the
    // user_id matches and the row hasn't expired. This test
    // pins that invariant — the kind check is the *whole*
    // reason migration 0013 widened the CHECK constraint.
    use sui_id_shared::ids::WebauthnPendingId;
    use sui_id_store::models::{WebauthnPendingKind, WebauthnPendingRow};
    use sui_id_store::repos::webauthn_pending;

    let db = fresh_db();
    let clock = crate::time::system_clock();
    let uid = create_user(&db).await;
    let session_id = fresh_session(&db, &clock, uid).await;
    let pending_id = WebauthnPendingId::new();
    let now = clock.now();
    webauthn_pending::insert(
        &db,
        &WebauthnPendingRow {
            id: pending_id,
            kind: WebauthnPendingKind::Authenticate, // wrong kind
            user_id: Some(uid),
            state_json: "{}".into(),
            expires_at: now + Duration::seconds(60),
            created_at: now,
        },
    )
    .await
    .expect("insert");

    let issuer = "https://test.example";
    // The credential JSON is moot — the kind check fails first.
    let result = finish_webauthn(
        &db,
        &clock,
        issuer,
        uid,
        session_id,
        pending_id,
        r#"{"id":"x","rawId":"x","type":"public-key","response":{}}"#,
        "/me/security/mfa",
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::errors::CoreError::InvalidCredentials)
    ));

    // The pending row is intact — refusing a step-up finish on
    // an Authenticate row must not consume it, so the legitimate
    // login-MFA flow that owns the row can still complete.
    let still_there = webauthn_pending::get(&db, pending_id)
        .await
        .expect("query")
        .expect("row preserved");
    assert_eq!(still_there.kind, WebauthnPendingKind::Authenticate);
}

#[tokio::test]
async fn finish_webauthn_refuses_pending_for_other_user() {
    // Even a kind = StepUp pending must be refused if it
    // belongs to a different user. Prevents pending-id
    // smuggling across sessions.
    use sui_id_shared::ids::WebauthnPendingId;
    use sui_id_store::models::{WebauthnPendingKind, WebauthnPendingRow};
    use sui_id_store::repos::webauthn_pending;

    let db = fresh_db();
    let clock = crate::time::system_clock();
    let real_owner = create_user(&db).await;
    // Create a *different* user we'll pretend is the one
    // signed in.
    let imposter = {
        let id = UserId::new();
        let now = Utc::now();
        users::create(
            &db,
            &sui_id_store::models::UserRow {
                id,
                username: "imposter".into(),
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
                preferred_lang: None,
                email_normalized: None,
                email_verified_at: None,
            },
        )
        .await
        .expect("imposter");
        id
    };
    let session_id = fresh_session(&db, &clock, imposter).await;
    let pending_id = WebauthnPendingId::new();
    let now = clock.now();
    webauthn_pending::insert(
        &db,
        &WebauthnPendingRow {
            id: pending_id,
            kind: WebauthnPendingKind::StepUp,
            user_id: Some(real_owner), // the rightful owner
            state_json: "{}".into(),
            expires_at: now + Duration::seconds(60),
            created_at: now,
        },
    )
    .await
    .expect("insert");

    let result = finish_webauthn(
        &db,
        &clock,
        "https://test.example",
        imposter,
        session_id,
        pending_id,
        r#"{"id":"x","rawId":"x","type":"public-key","response":{}}"#,
        "/me/security/mfa",
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::errors::CoreError::InvalidCredentials)
    ));

    // Pending row was NOT consumed — owner can still complete.
    assert!(
        webauthn_pending::get(&db, pending_id)
            .await
            .expect("query")
            .is_some()
    );
}
