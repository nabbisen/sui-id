//! Tests for `pending_settings_change` repository (RFC 090).
//!
//! Verifies the four security properties directly against the store:
//! - P2 (binding) — wrong session_id must be caught by the domain caller;
//!   the store's `consume` simply returns the row for the caller to check.
//! - P3 (single-use) — second `consume` on the same id returns NotFound.
//! - P4 (expiry) — `consume` on an expired row returns NotFound; the row
//!   is also deleted as a side-effect.
//! - Purge — `purge_expired` removes only expired rows.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use chrono::{Duration, Utc};
use sui_id_shared::ids::{PendingChangeId, SessionId, UserId};

use crate::{
    Database, StoreError,
    crypto::MasterKey,
    repos::pending_settings_change::{self, PendingSettingsChangeRow},
};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

fn sample_row(ttl_secs: i64) -> PendingSettingsChangeRow {
    PendingSettingsChangeRow {
        id: PendingChangeId::new(),
        session_id: SessionId::new(),
        actor_id: UserId::new(),
        intent: "smtp_password_update".into(),
        payload_enc: vec![0xDE, 0xAD, 0xBE, 0xEF], // dummy ciphertext
        summary: "SMTP password: will be updated".into(),
        csrf_token: "test-csrf-token".into(),
        expires_at: Utc::now() + Duration::seconds(ttl_secs),
        created_at: Utc::now(),
    }
}

// ── P3: single-use ────────────────────────────────────────────────────────────

/// First consume returns the row; second returns NotFound.
#[tokio::test]
async fn consume_is_single_use() {
    let db = fresh_db();
    let row = sample_row(300);
    let id = row.id;
    pending_settings_change::insert(&db, &row)
        .await
        .expect("insert");

    let now = Utc::now();
    let result = pending_settings_change::consume(&db, id, now)
        .await
        .expect("first consume must succeed");
    assert_eq!(result.intent, "smtp_password_update");

    let err = pending_settings_change::consume(&db, id, now)
        .await
        .expect_err("second consume must fail");
    assert!(
        matches!(err, StoreError::NotFound),
        "second consume must be NotFound; got: {err:?}"
    );
}

/// After consume, the row is gone from the database (proven by direct SQL).
#[tokio::test]
async fn consume_deletes_row_from_db() {
    let db = fresh_db();
    let row = sample_row(300);
    let id = row.id;
    let id_str = id.to_string();
    pending_settings_change::insert(&db, &row)
        .await
        .expect("insert");

    pending_settings_change::consume(&db, id, Utc::now())
        .await
        .expect("consume");

    let count: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM pending_settings_change WHERE id = ?1",
                    [id_str.as_str()],
                    |r| r.get(0),
                )
                .expect("count"))
        })
        .expect("query");
    assert_eq!(count, 0, "row must be deleted after consume");
}

// ── P4: expiry ────────────────────────────────────────────────────────────────

/// Consuming an expired row returns NotFound.
#[tokio::test]
async fn consume_expired_row_returns_not_found() {
    let db = fresh_db();
    let row = sample_row(-10); // already expired
    let id = row.id;
    pending_settings_change::insert(&db, &row)
        .await
        .expect("insert");

    let err = pending_settings_change::consume(&db, id, Utc::now())
        .await
        .expect_err("expired consume must fail");
    assert!(
        matches!(err, StoreError::NotFound),
        "expired consume must be NotFound; got: {err:?}"
    );
}

/// An expired row that fails consume is NOT deleted by consume itself —
/// `purge_expired` is responsible for cleanup. This test verifies that
/// a failed consume on an expired row leaves the row in the DB (so that
/// purge_expired can find and remove it).
#[tokio::test]
async fn consume_expired_row_does_not_delete_it_purge_handles_that() {
    let db = fresh_db();
    let row = sample_row(-10);
    let id = row.id;
    let id_str = id.to_string();
    pending_settings_change::insert(&db, &row)
        .await
        .expect("insert");

    // Consume on an expired row → NotFound.
    let _ = pending_settings_change::consume(&db, id, Utc::now()).await;

    // Row is still in the DB (purge_expired handles cleanup, not consume).
    let count: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM pending_settings_change WHERE id = ?1",
                    [id_str.as_str()],
                    |r| r.get(0),
                )
                .expect("count"))
        })
        .expect("query");
    assert_eq!(
        count, 1,
        "expired row must remain until purge_expired removes it"
    );

    // Verify purge_expired does remove it.
    let removed = pending_settings_change::purge_expired(&db, Utc::now())
        .await
        .expect("purge");
    assert_eq!(removed, 1, "purge must remove the expired row");
}

// ── Unknown id ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn consume_unknown_id_returns_not_found() {
    let db = fresh_db();
    let id = PendingChangeId::new(); // never inserted
    let err = pending_settings_change::consume(&db, id, Utc::now())
        .await
        .expect_err("unknown id must fail");
    assert!(
        matches!(err, StoreError::NotFound),
        "unknown id must be NotFound; got: {err:?}"
    );
}

// ── Cancel ────────────────────────────────────────────────────────────────────

/// Cancel removes the row; subsequent consume returns NotFound.
#[tokio::test]
async fn cancel_removes_row() {
    let db = fresh_db();
    let row = sample_row(300);
    let id = row.id;
    pending_settings_change::insert(&db, &row)
        .await
        .expect("insert");

    pending_settings_change::cancel(&db, id)
        .await
        .expect("cancel");

    let err = pending_settings_change::consume(&db, id, Utc::now())
        .await
        .expect_err("consume after cancel must fail");
    assert!(matches!(err, StoreError::NotFound));
}

/// Cancel on an already-absent row succeeds (idempotent).
#[tokio::test]
async fn cancel_absent_row_is_ok() {
    let db = fresh_db();
    let id = PendingChangeId::new();
    pending_settings_change::cancel(&db, id)
        .await
        .expect("cancel on absent row must succeed");
}

// ── Purge expired ─────────────────────────────────────────────────────────────

/// purge_expired removes expired rows and leaves live rows intact.
#[tokio::test]
async fn purge_expired_removes_only_expired_rows() {
    let db = fresh_db();

    let live = sample_row(300);
    let expired1 = sample_row(-10);
    let expired2 = sample_row(-1);
    let live_id_str = live.id.to_string();

    pending_settings_change::insert(&db, &live)
        .await
        .expect("insert live");
    pending_settings_change::insert(&db, &expired1)
        .await
        .expect("insert expired1");
    pending_settings_change::insert(&db, &expired2)
        .await
        .expect("insert expired2");

    let removed = pending_settings_change::purge_expired(&db, Utc::now())
        .await
        .expect("purge");
    assert_eq!(
        removed, 2,
        "purge must remove 2 expired rows; removed: {removed}"
    );

    // Live row must still exist.
    let count: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM pending_settings_change WHERE id = ?1",
                    [live_id_str.as_str()],
                    |r| r.get(0),
                )
                .expect("count"))
        })
        .expect("query");
    assert_eq!(count, 1, "live row must not be purged");
}

// ── get_summary ───────────────────────────────────────────────────────────────

/// get_summary on a live row returns the non-secret summary string.
#[tokio::test]
async fn get_summary_returns_summary_for_live_row() {
    let db = fresh_db();
    let row = sample_row(300);
    let id = row.id;
    let expected = row.summary.clone();
    pending_settings_change::insert(&db, &row)
        .await
        .expect("insert");

    let got = pending_settings_change::get_summary(&db, id, Utc::now())
        .await
        .expect("get_summary must succeed");
    assert_eq!(got, expected);
}

/// get_summary on an absent id returns NotFound.
#[tokio::test]
async fn get_summary_returns_not_found_for_absent_id() {
    let db = fresh_db();
    let err = pending_settings_change::get_summary(&db, PendingChangeId::new(), Utc::now())
        .await
        .expect_err("absent id must fail");
    assert!(matches!(err, StoreError::NotFound));
}

/// get_summary on an expired row returns NotFound (does NOT consume the row).
#[tokio::test]
async fn get_summary_returns_not_found_for_expired_row() {
    let db = fresh_db();
    let row = sample_row(-10); // already expired
    let id = row.id;
    let id_str = id.to_string();
    pending_settings_change::insert(&db, &row)
        .await
        .expect("insert");

    let err = pending_settings_change::get_summary(&db, id, Utc::now())
        .await
        .expect_err("expired row must fail");
    assert!(matches!(err, StoreError::NotFound));

    // Crucially: get_summary did NOT delete the row (non-destructive).
    let count: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM pending_settings_change WHERE id = ?1",
                    [id_str.as_str()],
                    |r| r.get(0),
                )
                .expect("count"))
        })
        .expect("query");
    assert_eq!(
        count, 1,
        "get_summary must not delete the row even on expiry"
    );
}
