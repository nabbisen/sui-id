//! Tests for RFC 080 — Refresh Token Rotation Atomicity.
//!
//! Validates P1 (single winner), P2 (family atomically revoked on reuse),
//! P3 (≤ 1 active per family), plus expired / unknown / client-mismatch.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use chrono::{Duration, Utc};
use std::sync::Arc;
use sui_id_shared::{
    FamilyId, RawRefreshToken, RefreshTokenHash, RefreshTokenId,
    ids::{ClientId, UserId},
};

use crate::{
    Database, StoreError,
    crypto::MasterKey,
    models::{ClientRow, ConsentPolicy, RefreshTokenRow, Role, UserRow},
    repos::{clients, refresh_tokens, refresh_tokens::RotationLookup, users},
};

// ── seed helper ───────────────────────────────────────────────────────────────

async fn seed_db() -> (Database, UserId, ClientId) {
    let db = Database::open_in_memory(MasterKey::generate()).expect("db");
    let now = Utc::now();
    let user_id = UserId::new();
    let client_id = ClientId::new();

    users::create(
        &db,
        &UserRow {
            id: user_id,
            username: format!("test-{user_id}"),
            display_name: None,
            email: None,
            email_normalized: None,
            email_verified_at: None,
            preferred_lang: None,
            is_admin: false,
            role: Role::User,
            is_disabled: false,
            is_deleted: false,
            last_login_at: None,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: crate::models::UserSource::Local,
            external_stable_id: None,
        },
    )
    .await
    .expect("seed user");

    clients::create(
        &db,
        &ClientRow {
            id: client_id,
            name: "test-client".into(),
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
    .expect("seed client");

    (db, user_id, client_id)
}

fn fresh_token() -> RawRefreshToken {
    RawRefreshToken::from_untrusted(uuid::Uuid::new_v4().to_string())
}

async fn insert_active(
    db: &Database,
    token: &RawRefreshToken,
    user_id: UserId,
    client_id: ClientId,
    family_id: Option<FamilyId>,
    ttl_secs: i64,
) -> RefreshTokenRow {
    let id = RefreshTokenId::generate();
    let family = family_id.unwrap_or_else(|| FamilyId::root_of(&id));
    let row = RefreshTokenRow {
        id,
        user_id,
        client_id,
        scope: "openid".into(),
        expires_at: Utc::now() + Duration::seconds(ttl_secs),
        revoked_at: None,
        created_at: Utc::now(),
        auth_methods: vec![],
        family_id: family,
    };
    refresh_tokens::insert(db, &row, token)
        .await
        .expect("insert");
    row
}

// ── P1: single winner (sequential) ───────────────────────────────────────────

#[tokio::test]
async fn sequential_first_wins_second_is_reuse() {
    let (db, uid, cid) = seed_db().await;
    let token = fresh_token();
    insert_active(&db, &token, uid, cid, None, 3600).await;
    let hash = RefreshTokenHash::of(&token);
    let now = Utc::now();

    let first = refresh_tokens::begin_rotation(&db, &hash, &cid, now)
        .await
        .expect("first");
    assert!(
        matches!(first, RotationLookup::RotatedHere(_)),
        "first must be RotatedHere; got: {first:?}"
    );

    let second = refresh_tokens::begin_rotation(&db, &hash, &cid, now)
        .await
        .expect("second");
    assert!(
        matches!(second, RotationLookup::ReuseDetected { .. }),
        "second must be ReuseDetected; got: {second:?}"
    );
}

// ── P1 + P2: concurrent winner/loser arbitration ─────────────────────────────

#[tokio::test]
async fn concurrent_exactly_one_winner() {
    const N: usize = 8;
    let (db, uid, cid) = seed_db().await;
    let token = fresh_token();
    insert_active(&db, &token, uid, cid, None, 3600).await;
    let db = Arc::new(db);
    let hash = Arc::new(RefreshTokenHash::of(&token));
    let cid = Arc::new(cid);
    let now = Utc::now();

    let mut handles = Vec::with_capacity(N);
    for _ in 0..N {
        let db = db.clone();
        let hash = hash.clone();
        let cid = cid.clone();
        handles.push(tokio::spawn(async move {
            refresh_tokens::begin_rotation(&db, &hash, &cid, now)
                .await
                .expect("begin_rotation")
        }));
    }
    let mut results = Vec::with_capacity(N);
    for h in handles {
        results.push(h.await.expect("task"));
    }

    let winners = results
        .iter()
        .filter(|r| matches!(r, RotationLookup::RotatedHere(_)))
        .count();
    let losers = results
        .iter()
        .filter(|r| matches!(r, RotationLookup::ReuseDetected { .. }))
        .count();

    assert_eq!(
        winners, 1,
        "exactly one concurrent winner expected; got {winners}"
    );
    assert_eq!(
        losers,
        N - 1,
        "all others must be ReuseDetected; got {losers}"
    );
}

// ── P2: family revoked atomically on ReuseDetected ───────────────────────────

#[tokio::test]
async fn reuse_detected_revokes_family_atomically() {
    let (db, uid, cid) = seed_db().await;
    let token = fresh_token();
    let row = insert_active(&db, &token, uid, cid, None, 3600).await;
    let hash = RefreshTokenHash::of(&token);

    // First rotation wins; original token is now revoked.
    refresh_tokens::begin_rotation(&db, &hash, &cid, Utc::now())
        .await
        .expect("rotation 1");

    // Insert a successor in the same family — it is still active.
    let token2 = fresh_token();
    insert_active(&db, &token2, uid, cid, Some(row.family_id.clone()), 3600).await;

    // Replay the now-revoked original token.
    let result = refresh_tokens::begin_rotation(&db, &hash, &cid, Utc::now())
        .await
        .expect("replay");
    assert!(
        matches!(result, RotationLookup::ReuseDetected { .. }),
        "replay must be ReuseDetected; got: {result:?}"
    );

    // Direct SQL: no active rows remain in the family.
    let family_str = row.family_id.as_str().to_owned();
    let active: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM refresh_tokens \
                     WHERE family_id = ?1 AND revoked_at IS NULL",
                    [family_str.as_str()],
                    |r| r.get(0),
                )
                .expect("count"))
        })
        .expect("query");
    assert_eq!(
        active, 0,
        "P3: all family members must be revoked after theft detection; got {active}"
    );
}

// ── P3: chain keeps ≤ 1 active per family ────────────────────────────────────

#[tokio::test]
async fn rotation_chain_active_count_never_exceeds_one() {
    let (db, uid, cid) = seed_db().await;

    let t1 = fresh_token();
    let row1 = insert_active(&db, &t1, uid, cid, None, 3600).await;
    let family = row1.family_id.clone();

    let count_active = |db: &Database, fam: String| {
        db.with_conn_sync(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM refresh_tokens \
                     WHERE family_id = ?1 AND revoked_at IS NULL",
                    [fam.as_str()],
                    |r| r.get::<_, i64>(0),
                )
                .expect("count"))
        })
        .expect("query")
    };

    let h1 = RefreshTokenHash::of(&t1);
    refresh_tokens::begin_rotation(&db, &h1, &cid, Utc::now())
        .await
        .expect("rot 1");
    let t2 = fresh_token();
    insert_active(&db, &t2, uid, cid, Some(family.clone()), 3600).await;
    assert!(
        count_active(&db, family.as_str().to_owned()) <= 1,
        "after rot 1"
    );

    let h2 = RefreshTokenHash::of(&t2);
    refresh_tokens::begin_rotation(&db, &h2, &cid, Utc::now())
        .await
        .expect("rot 2");
    let t3 = fresh_token();
    insert_active(&db, &t3, uid, cid, Some(family.clone()), 3600).await;
    assert!(
        count_active(&db, family.as_str().to_owned()) <= 1,
        "after rot 2"
    );

    let h3 = RefreshTokenHash::of(&t3);
    refresh_tokens::begin_rotation(&db, &h3, &cid, Utc::now())
        .await
        .expect("rot 3");
    assert!(
        count_active(&db, family.as_str().to_owned()) <= 1,
        "after rot 3"
    );
}

// ── Expired → Expired variant ─────────────────────────────────────────────────

#[tokio::test]
async fn expired_token_returns_expired_variant() {
    let (db, uid, cid) = seed_db().await;
    let token = fresh_token();
    insert_active(&db, &token, uid, cid, None, -10).await;
    let hash = RefreshTokenHash::of(&token);
    let result = refresh_tokens::begin_rotation(&db, &hash, &cid, Utc::now())
        .await
        .expect("begin_rotation");
    assert!(
        matches!(result, RotationLookup::Expired(_)),
        "expired must return Expired; got: {result:?}"
    );
}

// ── Unknown → Unknown variant ─────────────────────────────────────────────────

#[tokio::test]
async fn unknown_token_returns_unknown_variant() {
    let (db, _, cid) = seed_db().await;
    let token = fresh_token();
    let hash = RefreshTokenHash::of(&token);
    let result = refresh_tokens::begin_rotation(&db, &hash, &cid, Utc::now())
        .await
        .expect("begin_rotation");
    assert!(
        matches!(result, RotationLookup::Unknown),
        "unknown must return Unknown; got: {result:?}"
    );
}

// ── Client mismatch → Conflict, no mutation ───────────────────────────────────

#[tokio::test]
async fn wrong_client_returns_conflict_without_revoking() {
    let (db, uid, cid) = seed_db().await;
    let token = fresh_token();
    insert_active(&db, &token, uid, cid, None, 3600).await;
    let hash = RefreshTokenHash::of(&token);

    // A different ClientId (not the one used at insert — no FK needed for
    // the mismatch check since begin_rotation returns Conflict before any DB write).
    let wrong = ClientId::new();
    let err = refresh_tokens::begin_rotation(&db, &hash, &wrong, Utc::now())
        .await
        .expect_err("wrong client must fail");
    assert!(
        matches!(err, StoreError::Conflict),
        "wrong client must be Conflict; got: {err:?}"
    );

    // Token must still be active (revoked_at IS NULL).
    let hash_bytes = hash.as_bytes().to_vec();
    let revoked_at: Option<String> = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT revoked_at FROM refresh_tokens WHERE token_hash = ?1",
                    [hash_bytes.as_slice()],
                    |r| r.get(0),
                )
                .expect("row"))
        })
        .expect("query");
    assert!(
        revoked_at.is_none(),
        "token must remain active after client mismatch rejection"
    );
}

// ── T06: revoke_all_for_user — only this user's live tokens ──────────────────

#[tokio::test]
async fn revoke_all_for_user_leaves_other_users_and_already_revoked_rows_alone() {
    let (db, uid, cid) = seed_db().await;
    // A second user, as a foreign key in the *same* database -- a second,
    // separate `seed_db()` would open an unrelated in-memory database.
    let other_uid = UserId::new();
    users::create(
        &db,
        &UserRow {
            id: other_uid,
            username: format!("test-{other_uid}"),
            display_name: None,
            email: None,
            email_normalized: None,
            email_verified_at: None,
            preferred_lang: None,
            is_admin: false,
            role: Role::User,
            is_disabled: false,
            is_deleted: false,
            last_login_at: None,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            failed_login_count: 0,
            locked_until: None,
            source: crate::models::UserSource::Local,
            external_stable_id: None,
        },
    )
    .await
    .expect("seed other user");

    // This user's live token -- must be revoked.
    let live_token = fresh_token();
    let live_row = insert_active(&db, &live_token, uid, cid, None, 3600).await;

    // This user's already-revoked token -- its `revoked_at` must keep its
    // old value, not be overwritten with a new timestamp.
    let already_revoked_token = fresh_token();
    let already_revoked_row =
        insert_active(&db, &already_revoked_token, uid, cid, None, 3600).await;
    refresh_tokens::revoke(&db, &already_revoked_row.id)
        .await
        .expect("revoke");
    let revoked_at_before: chrono::DateTime<Utc> = db
        .with_conn_sync({
            let id = already_revoked_row.id.as_str().to_owned();
            move |conn| {
                Ok(conn.query_row(
                    "SELECT revoked_at FROM refresh_tokens WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )?)
            }
        })
        .expect("read revoked_at");

    // Another user's live token -- must be untouched.
    let other_token = fresh_token();
    insert_active(&db, &other_token, other_uid, cid, None, 3600).await;

    let n = refresh_tokens::revoke_all_for_user(&db, uid)
        .await
        .expect("revoke_all_for_user");
    assert_eq!(n, 1, "exactly the one live row for this user");

    let live_revoked_at: Option<chrono::DateTime<Utc>> = db
        .with_conn_sync({
            let id = live_row.id.as_str().to_owned();
            move |conn| {
                Ok(conn.query_row(
                    "SELECT revoked_at FROM refresh_tokens WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )?)
            }
        })
        .expect("read live revoked_at");
    assert!(
        live_revoked_at.is_some(),
        "this user's live token must now be revoked"
    );

    let revoked_at_after: chrono::DateTime<Utc> = db
        .with_conn_sync({
            let id = already_revoked_row.id.as_str().to_owned();
            move |conn| {
                Ok(conn.query_row(
                    "SELECT revoked_at FROM refresh_tokens WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )?)
            }
        })
        .expect("read already-revoked revoked_at again");
    assert_eq!(
        revoked_at_after, revoked_at_before,
        "an already-revoked row's revoked_at must not change"
    );

    let other_revoked_at: Option<chrono::DateTime<Utc>> = db
        .with_conn_sync(move |conn| {
            Ok(conn.query_row(
                "SELECT revoked_at FROM refresh_tokens WHERE user_id = ?1",
                [other_uid.to_string()],
                |r| r.get(0),
            )?)
        })
        .expect("read other user's revoked_at");
    assert!(
        other_revoked_at.is_none(),
        "another user's live token must be untouched"
    );
}

// ── T08: backfill_token_hashes — only rows with no hash yet ──────────────────

#[tokio::test]
async fn backfill_token_hashes_fills_only_null_rows_and_is_idempotent() {
    let (db, uid, cid) = seed_db().await;

    // A row `insert` already gave a hash -- must be left exactly alone.
    let already_hashed_token = fresh_token();
    let already_hashed_row = insert_active(&db, &already_hashed_token, uid, cid, None, 3600).await;
    let hash_before: Vec<u8> = db
        .with_conn_sync({
            let id = already_hashed_row.id.as_str().to_owned();
            move |conn| {
                Ok(conn.query_row(
                    "SELECT token_hash FROM refresh_tokens WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )?)
            }
        })
        .expect("read existing hash");

    // A row with no hash yet -- simulating a pre-migration-0019 row, the
    // one case this function exists for.
    let null_token = fresh_token();
    let null_row = insert_active(&db, &null_token, uid, cid, None, 3600).await;
    db.with_conn_sync({
        let id = null_row.id.as_str().to_owned();
        move |conn| {
            conn.execute(
                "UPDATE refresh_tokens SET token_hash = NULL WHERE id = ?1",
                [id],
            )?;
            Ok(())
        }
    })
    .expect("clear hash");

    let filled = refresh_tokens::backfill_token_hashes(&db)
        .await
        .expect("backfill");
    assert_eq!(filled, 1, "exactly the one NULL row");

    let expected_hash = RefreshTokenHash::of(&null_token).as_bytes().to_vec();
    let hash_after_fill: Vec<u8> = db
        .with_conn_sync({
            let id = null_row.id.as_str().to_owned();
            move |conn| {
                Ok(conn.query_row(
                    "SELECT token_hash FROM refresh_tokens WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )?)
            }
        })
        .expect("read filled hash");
    assert_eq!(
        hash_after_fill, expected_hash,
        "the backfilled hash must be the real hash of the row's own token"
    );

    let hash_unchanged: Vec<u8> = db
        .with_conn_sync({
            let id = already_hashed_row.id.as_str().to_owned();
            move |conn| {
                Ok(conn.query_row(
                    "SELECT token_hash FROM refresh_tokens WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )?)
            }
        })
        .expect("read unchanged hash");
    assert_eq!(
        hash_unchanged, hash_before,
        "a row that already had a hash must not be touched"
    );

    // Run it again: nothing left to fill, nothing changes.
    let filled_again = refresh_tokens::backfill_token_hashes(&db)
        .await
        .expect("second backfill");
    assert_eq!(filled_again, 0, "the second run finds nothing to do");

    let hash_after_second_run: Vec<u8> = db
        .with_conn_sync({
            let id = null_row.id.as_str().to_owned();
            move |conn| {
                Ok(conn.query_row(
                    "SELECT token_hash FROM refresh_tokens WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )?)
            }
        })
        .expect("read hash after second run");
    assert_eq!(
        hash_after_second_run, expected_hash,
        "the second run must not change the already-filled hash"
    );
}
