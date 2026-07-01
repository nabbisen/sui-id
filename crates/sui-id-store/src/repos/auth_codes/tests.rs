//! Tests for RFC 079 — Authorization Code Lifecycle Assurance.
//!
//! Validates that `consume` enforces single-use (P1) and expiry (P2) at the
//! SQL-statement level, independently of the connection model.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use chrono::{Duration, Utc};
use sui_id_shared::CodeHash;
use sui_id_shared::ids::{ClientId, UserId};

use crate::{
    Database,
    crypto::MasterKey,
    models::{AuthorizationCodeRow, ClientRow, ConsentPolicy, Role, UserRow},
    repos::{auth_codes, clients, users},
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
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .expect("seed client");

    (db, user_id, client_id)
}

fn code_row(
    code: &str,
    ttl_secs: i64,
    user_id: UserId,
    client_id: ClientId,
) -> AuthorizationCodeRow {
    AuthorizationCodeRow {
        code_hash: CodeHash::of(code),
        client_id,
        user_id,
        redirect_uri: "https://example.com/cb".into(),
        scope: "openid".into(),
        nonce: None,
        code_challenge: "challenge".into(),
        code_challenge_method: "S256".into(),
        expires_at: Utc::now() + Duration::seconds(ttl_secs),
        consumed: false,
        created_at: Utc::now(),
        auth_methods: vec![],
    }
}

// ── P1: single-use (sequential) ──────────────────────────────────────────────

#[tokio::test]
async fn consume_is_single_use_sequential() {
    let (db, uid, cid) = seed_db().await;
    let row = code_row("code-single-use", 60, uid, cid);
    auth_codes::insert(&db, &row).await.expect("insert");
    let now = Utc::now();

    auth_codes::consume(&db, &row.code_hash, now)
        .await
        .expect("first consume must succeed");

    let err = auth_codes::consume(&db, &row.code_hash, now)
        .await
        .expect_err("second consume must fail");
    assert!(
        matches!(err, crate::StoreError::NotFound),
        "second consume must be NotFound; got: {err:?}"
    );
}

#[tokio::test]
async fn consume_sets_consumed_flag_in_db() {
    let (db, uid, cid) = seed_db().await;
    let row = code_row("code-flag-check", 60, uid, cid);
    auth_codes::insert(&db, &row).await.expect("insert");
    let hash_str = row.code_hash.as_str().to_owned();

    auth_codes::consume(&db, &row.code_hash, Utc::now())
        .await
        .expect("consume");

    let consumed: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT consumed FROM auth_codes WHERE code_hash = ?1",
                    [hash_str.as_str()],
                    |r| r.get(0),
                )
                .expect("row"))
        })
        .expect("query");
    assert_eq!(consumed, 1, "consumed flag must be 1 after consume");
}

// ── P2: expiry enforced by SQL predicate ──────────────────────────────────────

#[tokio::test]
async fn consume_rejects_expired_and_does_not_flip_flag() {
    let (db, uid, cid) = seed_db().await;
    let row = code_row("code-expired", -10, uid, cid);
    auth_codes::insert(&db, &row).await.expect("insert");
    let hash_str = row.code_hash.as_str().to_owned();

    let err = auth_codes::consume(&db, &row.code_hash, Utc::now())
        .await
        .expect_err("expired code must fail");
    assert!(
        matches!(err, crate::StoreError::NotFound),
        "expired code must be NotFound; got: {err:?}"
    );

    let consumed: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT consumed FROM auth_codes WHERE code_hash = ?1",
                    [hash_str.as_str()],
                    |r| r.get(0),
                )
                .expect("row"))
        })
        .expect("query");
    assert_eq!(
        consumed, 0,
        "SQL predicate must block the UPDATE; consumed must stay 0"
    );
}

// ── P4: burn-on-failure — consumed stays 1 after replay ──────────────────────

#[tokio::test]
async fn consumed_flag_stays_set_on_replay() {
    let (db, uid, cid) = seed_db().await;
    let row = code_row("code-p4", 60, uid, cid);
    auth_codes::insert(&db, &row).await.expect("insert");
    let hash_str = row.code_hash.as_str().to_owned();

    auth_codes::consume(&db, &row.code_hash, Utc::now())
        .await
        .expect("first consume");
    let _ = auth_codes::consume(&db, &row.code_hash, Utc::now()).await;

    let consumed: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT consumed FROM auth_codes WHERE code_hash = ?1",
                    [hash_str.as_str()],
                    |r| r.get(0),
                )
                .expect("row"))
        })
        .expect("query");
    assert_eq!(consumed, 1, "consumed flag must remain 1 after replay");
}

// ── P5: unknown code → NotFound ───────────────────────────────────────────────

#[tokio::test]
async fn consume_unknown_code_returns_not_found() {
    let (db, _, _) = seed_db().await;
    let hash = CodeHash::of("never-inserted");
    let err = auth_codes::consume(&db, &hash, Utc::now())
        .await
        .expect_err("unknown code must fail");
    assert!(
        matches!(err, crate::StoreError::NotFound),
        "unknown code must be NotFound; got: {err:?}"
    );
}
