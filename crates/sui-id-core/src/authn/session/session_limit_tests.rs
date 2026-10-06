//! Idle-session timeout and concurrent-session cap (v0.25.0).
use super::*;
use crate::time::{MockClock, SharedClock, system_clock};
use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use sui_id_store::Database;
use sui_id_store::crypto::MasterKey;

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

async fn make_user(db: &Database) -> UserId {
    use sui_id_store::models::UserRow;
    use sui_id_store::repos::users;
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
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("user");
    id
}

/// A session created at `created_at` by signing in through L01 (RFC 102
/// A4: no raw session insert outside the store).
async fn insert_session(
    db: &Database,
    user_id: UserId,
    created_at: chrono::DateTime<Utc>,
    last_used_at: Option<chrono::DateTime<Utc>>,
) -> SessionId {
    let id = SessionId::new();
    sui_id_store::commands::sign_in_with_password(
        db,
        SessionRow {
            id,
            user_id,
            expires_at: created_at + ChronoDuration::hours(24),
            created_at,
            revoked_at: None,
            auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
            last_step_up_at: None,
            last_used_at,
        },
    )
    .await
    .expect("sign in");
    id
}

#[tokio::test]
async fn resolve_passes_when_idle_timeout_disabled() {
    let db = fresh_db();
    let clock = system_clock();
    let uid = make_user(&db).await;
    // last_used_at is far in the past; default settings have
    // idle_session_timeout_secs = 0 = disabled, so resolve
    // must succeed.
    let stale = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    let sid = insert_session(&db, uid, Utc::now(), Some(stale)).await;
    assert_eq!(resolve(&db, &clock, sid).await.expect("resolve"), uid);
}

#[tokio::test]
async fn resolve_revokes_after_idle_window() {
    let db = fresh_db();
    let uid = make_user(&db).await;
    // Configure a 60-second idle timeout.
    sui_id_store::repos::server_settings::update_idle_session_timeout(&db, 60, Utc::now())
        .await
        .expect("set timeout");
    // Make a session that was last used 2 minutes ago and a
    // mock clock at "now", so elapsed = 120s > 60s.
    let now = Utc::now();
    let stale = now - ChronoDuration::seconds(120);
    let sid = insert_session(&db, uid, now - ChronoDuration::hours(1), Some(stale)).await;
    let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
    // First call: idle window exceeded → revoke + Unauth.
    assert!(matches!(
        resolve(&db, &clock, sid).await,
        Err(CoreError::Unauthenticated)
    ));
    // The session is now revoked in the DB.
    let row = sessions::get(&db, sid).await.expect("get");
    assert!(row.revoked_at.is_some());
}

#[tokio::test]
async fn resolve_passes_within_idle_window() {
    let db = fresh_db();
    let uid = make_user(&db).await;
    sui_id_store::repos::server_settings::update_idle_session_timeout(&db, 300, Utc::now())
        .await
        .expect("set timeout");
    let now = Utc::now();
    let recent = now - ChronoDuration::seconds(10);
    let sid = insert_session(&db, uid, now - ChronoDuration::hours(1), Some(recent)).await;
    let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
    assert_eq!(resolve(&db, &clock, sid).await.expect("resolve"), uid);
}

#[tokio::test]
async fn resolve_treats_null_last_used_at_as_created_at() {
    let db = fresh_db();
    let uid = make_user(&db).await;
    sui_id_store::repos::server_settings::update_idle_session_timeout(&db, 60, Utc::now())
        .await
        .expect("set timeout");
    // last_used_at = None: created 2 minutes ago, so falling
    // back to created_at means 120 > 60 = revoked.
    let now = Utc::now();
    let sid = insert_session(&db, uid, now - ChronoDuration::seconds(120), None).await;
    let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
    assert!(matches!(
        resolve(&db, &clock, sid).await,
        Err(CoreError::Unauthenticated)
    ));
}

#[tokio::test]
async fn touch_last_used_throttles_within_window() {
    let db = fresh_db();
    let uid = make_user(&db).await;
    let now = Utc::now();
    let original = now - ChronoDuration::seconds(10);
    let sid = insert_session(&db, uid, now - ChronoDuration::hours(1), Some(original)).await;
    let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
    // Throttle window is 60s; 10s old should not write.
    touch_last_used(&db, &clock, sid).await.expect("touch");
    let row = sessions::get(&db, sid).await.expect("get");
    assert_eq!(row.last_used_at, Some(original));
}

#[tokio::test]
async fn touch_last_used_writes_when_stale() {
    let db = fresh_db();
    let uid = make_user(&db).await;
    let now = Utc::now();
    let stale = now - ChronoDuration::seconds(120);
    let sid = insert_session(&db, uid, now - ChronoDuration::hours(1), Some(stale)).await;
    let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
    touch_last_used(&db, &clock, sid).await.expect("touch");
    let row = sessions::get(&db, sid).await.expect("get");
    // The new value should be ~now, definitely not the stale one.
    let updated = row.last_used_at.expect("set");
    assert!(updated > stale);
}

/// L01's in-transaction eviction (RFC 102): one password sign-in for
/// the user, with a fresh session row.
async fn sign_in(db: &Database, uid: UserId) -> SessionId {
    let now = Utc::now();
    let row = SessionRow {
        id: SessionId::new(),
        user_id: uid,
        expires_at: now + ChronoDuration::hours(12),
        created_at: now,
        revoked_at: None,
        auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
        last_step_up_at: None,
        last_used_at: None,
    };
    sui_id_store::commands::sign_in_with_password(db, row.clone())
        .await
        .expect("L01");
    row.id
}

#[tokio::test]
async fn sign_in_evicts_nothing_when_cap_zero() {
    let db = fresh_db();
    let uid = make_user(&db).await;
    // Insert 5 active sessions; cap = 0 = disabled.
    for i in 0..5 {
        let _ = insert_session(&db, uid, Utc::now() - ChronoDuration::seconds(i + 1), None).await;
    }
    sign_in(&db, uid).await;
    let active = sessions::count_active_for_user(&db, uid, Utc::now())
        .await
        .expect("count");
    assert_eq!(active, 6);
}

#[tokio::test]
async fn sign_in_evicts_oldest_in_fifo_order() {
    let db = fresh_db();
    let uid = make_user(&db).await;
    // Three older sessions with distinct created_at, then cap = 2 and a
    // sign-in: 4 active, so the 2 oldest (s1, s2) are revoked. The cap is
    // set after the first three, which are themselves sign-ins.
    let base = Utc::now() - ChronoDuration::hours(1);
    let s1 = insert_session(&db, uid, base, None).await;
    let s2 = insert_session(&db, uid, base + ChronoDuration::seconds(1), None).await;
    let s3 = insert_session(&db, uid, base + ChronoDuration::seconds(2), None).await;
    sui_id_store::repos::server_settings::update_max_concurrent_sessions(&db, 2, Utc::now())
        .await
        .expect("set cap");
    let s4 = sign_in(&db, uid).await;
    for (id, revoked, label) in [
        (s1, true, "s1 should be revoked"),
        (s2, true, "s2 should be revoked"),
        (s3, false, "s3 should remain"),
        (s4, false, "the new session should remain"),
    ] {
        assert_eq!(
            sessions::get(&db, id)
                .await
                .expect("get")
                .revoked_at
                .is_some(),
            revoked,
            "{label}"
        );
    }
}
