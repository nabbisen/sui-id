#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_arguments,
    clippy::panic
)]
//! Tests for RFC 096-B1 stage 11b — migration 0048, the partial
//! `UNIQUE (user_id) WHERE status IN ('pending', 'exhausted')` index on
//! `federation_mfa_pending`, and its own dedup step.
//!
//! These exercise the schema and the migration directly through
//! `rusqlite`, the same way `tests_rfc096_b1_stage8.rs` exercises
//! migration 0047 — this migration adds no new table, only an index and
//! a one-time cleanup `DELETE`, so there is nothing repo-level to test
//! here that `commands::tests::runner::federation`'s own
//! `issue_federated_mfa_pending` tests do not already cover at the
//! command layer.

use crate::migrations;
use rusqlite::Connection;

fn conn_at(version: i32) -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, version).expect("run migrations");
    conn
}

fn conn_at_48() -> Connection {
    conn_at(48)
}

fn seed_user(conn: &Connection, id: &str) {
    conn.execute(
        "INSERT INTO users(id, username, is_admin, is_disabled, is_deleted, created_at, \
             updated_at, user_uuid, failed_login_count) \
             VALUES(?1,?1,0,0,0,datetime('now'),datetime('now'),?2,0)",
        rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
    )
    .expect("user");
}

fn seed_provider(conn: &Connection, id: &str) {
    conn.execute(
        "INSERT INTO federation_provider( \
             id, slug, display_name, issuer, client_id, enabled, created_at, updated_at) \
             VALUES (?1, ?1, 'Provider', 'https://issuer.example', 'client', 1, \
                      datetime('now'), datetime('now'))",
        [id],
    )
    .expect("provider");
}

fn seed_link(conn: &Connection, user_id: &str, provider_id: &str) {
    // `upstream_sub` is unique per `provider_id` — distinct per user so
    // two users linked to the same provider in one test don't collide.
    conn.execute(
        "INSERT INTO federation_link(user_id, provider_id, upstream_sub, linked_at, last_seen_at) \
             VALUES (?1, ?2, ?1 || '-sub', datetime('now'), datetime('now'))",
        [user_id, provider_id],
    )
    .expect("link");
}

fn seed_user_provider_and_link(conn: &Connection, user_id: &str, provider_id: &str) {
    seed_user(conn, user_id);
    seed_provider(conn, provider_id);
    seed_link(conn, user_id, provider_id);
}

fn hash32(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

const T0: &str = "2026-10-15 10:00:00.000000+00:00";
const T0_PLUS_300: &str = "2026-10-15 10:05:00.000000+00:00";

const PENDING_INSERT_SQL: &str = "INSERT INTO federation_mfa_pending( \
    id, provider_id, provider_config_version, provider_activation_generation, user_id, \
    continuation_sealed, next_path, browser_binding_sha256, csrf_binding_sha256, \
    primary_method, failure_count, status, created_at, expires_at \
) VALUES (?1, ?2, 0, 0, ?3, ?4, NULL, ?5, ?6, 'fed', ?7, ?8, ?9, ?10)";

/// `continuation_sealed` is `NULL` for a terminal `status`, `Some` for
/// `pending` — the same table `CHECK` `tests_rfc096_b1_stage8.rs` already
/// exercises directly; mirrored here rather than re-derived so every
/// row this file inserts satisfies it.
fn try_insert_row(
    conn: &Connection,
    id: &str,
    user_id: &str,
    status: &str,
    failure_count: i64,
    created_at: &str,
) -> rusqlite::Result<usize> {
    let continuation_sealed: Option<&[u8]> = (status == "pending").then_some(b"sealed");
    conn.execute(
        PENDING_INSERT_SQL,
        rusqlite::params![
            id,
            "p1",
            user_id,
            continuation_sealed,
            hash32(1),
            hash32(2),
            failure_count,
            status,
            created_at,
            T0_PLUS_300,
        ],
    )
}

fn insert_row(conn: &Connection, id: &str, user_id: &str, status: &str, failure_count: i64) {
    try_insert_row(conn, id, user_id, status, failure_count, T0).expect("valid row");
}

fn row_exists(conn: &Connection, id: &str) -> bool {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM federation_mfa_pending WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .expect("count");
    n == 1
}

// ---- the partial index itself, at 48 ----------------------------------

#[test]
fn two_pending_rows_for_the_same_user_cannot_coexist() {
    let conn = conn_at_48();
    seed_user_provider_and_link(&conn, "u1", "p1");
    insert_row(&conn, "m1", "u1", "pending", 0);
    let err = try_insert_row(&conn, "m2", "u1", "pending", 0, T0).unwrap_err();
    assert!(matches!(
        err,
        rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::ConstraintViolation
    ));
}

#[test]
fn two_exhausted_rows_for_the_same_user_cannot_coexist() {
    let conn = conn_at_48();
    seed_user_provider_and_link(&conn, "u1", "p1");
    insert_row(&conn, "m1", "u1", "exhausted", 5);
    let err = try_insert_row(&conn, "m2", "u1", "exhausted", 5, T0).unwrap_err();
    assert!(matches!(
        err,
        rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::ConstraintViolation
    ));
}

#[test]
fn a_pending_row_cannot_coexist_with_an_exhausted_row_for_the_same_user() {
    let conn = conn_at_48();
    seed_user_provider_and_link(&conn, "u1", "p1");
    insert_row(&conn, "m1", "u1", "exhausted", 5);
    let err = try_insert_row(&conn, "m2", "u1", "pending", 0, T0).unwrap_err();
    assert!(matches!(
        err,
        rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::ConstraintViolation
    ));
}

/// The deliberately narrower half of migration 0048's own scope
/// (`commands::issue_federated_mfa_pending`'s own doc states the
/// reasoning): a `completed` row carries no brute-force budget to
/// protect, so it is invisible to the new index entirely.
#[test]
fn a_completed_and_a_pending_row_for_the_same_user_can_coexist() {
    let conn = conn_at_48();
    seed_user_provider_and_link(&conn, "u1", "p1");
    insert_row(&conn, "m1", "u1", "completed", 0);
    try_insert_row(&conn, "m2", "u1", "pending", 0, T0).expect("not in the partial index's scope");
    assert!(row_exists(&conn, "m1"));
    assert!(row_exists(&conn, "m2"));
}

/// Same reasoning, the other terminal status the index excludes:
/// `invalidated` is not a wrong-guess outcome, so it must not become a
/// de-facto lockout either.
#[test]
fn two_invalidated_rows_for_the_same_user_can_coexist() {
    let conn = conn_at_48();
    seed_user_provider_and_link(&conn, "u1", "p1");
    insert_row(&conn, "m1", "u1", "invalidated", 2);
    try_insert_row(&conn, "m2", "u1", "invalidated", 3, T0)
        .expect("not in the partial index's scope");
    assert!(row_exists(&conn, "m1"));
    assert!(row_exists(&conn, "m2"));
}

#[test]
fn different_users_can_each_have_their_own_pending_row() {
    let conn = conn_at_48();
    seed_user_provider_and_link(&conn, "u1", "p1");
    seed_user(&conn, "u2");
    seed_link(&conn, "u2", "p1");
    insert_row(&conn, "m1", "u1", "pending", 0);
    try_insert_row(&conn, "m2", "u2", "pending", 0, T0).expect("a different user, no conflict");
    assert!(row_exists(&conn, "m1"));
    assert!(row_exists(&conn, "m2"));
}

// ---- the migration's own dedup step, run forward from 47 --------------

#[test]
fn the_dedup_step_keeps_the_exhausted_row_over_a_pending_one() {
    let mut conn = conn_at(47);
    seed_user_provider_and_link(&conn, "u1", "p1");
    // Both storable at 47: the partial index this migration adds does
    // not exist yet, so nothing refuses the second row.
    insert_row(&conn, "pending-row", "u1", "pending", 2);
    insert_row(&conn, "exhausted-row", "u1", "exhausted", 5);

    migrations::run_up_to(&mut conn, 48).expect("apply 0048");

    assert!(
        !row_exists(&conn, "pending-row"),
        "the pending row must be deleted"
    );
    assert!(
        row_exists(&conn, "exhausted-row"),
        "the exhausted row must survive — it is the state RFC 096 :756 \
         says a restart must not be able to undo"
    );
}

#[test]
fn the_dedup_step_keeps_the_higher_failure_count_within_the_same_status() {
    let mut conn = conn_at(47);
    seed_user_provider_and_link(&conn, "u1", "p1");
    insert_row(&conn, "low-count", "u1", "pending", 1);
    insert_row(&conn, "high-count", "u1", "pending", 4);

    migrations::run_up_to(&mut conn, 48).expect("apply 0048");

    assert!(!row_exists(&conn, "low-count"));
    assert!(row_exists(&conn, "high-count"));
}

/// The dedup step's own `WHERE status IN ('pending', 'exhausted')` scope,
/// proven against real pre-existing data rather than only against the
/// index's predicate: a `completed` row sharing a `user_id` with a
/// `pending` one must survive the migration untouched, same as it is
/// allowed to coexist at 48 going forward (the test above this
/// section).
#[test]
fn the_dedup_step_does_not_touch_completed_or_invalidated_rows() {
    let mut conn = conn_at(47);
    seed_user_provider_and_link(&conn, "u1", "p1");
    insert_row(&conn, "completed-row", "u1", "completed", 0);
    insert_row(&conn, "pending-row", "u1", "pending", 3);

    migrations::run_up_to(&mut conn, 48).expect("apply 0048");

    assert!(
        row_exists(&conn, "completed-row"),
        "completed rows are outside this migration's dedup scope entirely"
    );
    assert!(
        row_exists(&conn, "pending-row"),
        "the lone pending row has nothing to conflict with after dedup"
    );
}

#[test]
fn a_single_active_row_per_user_survives_the_migration_untouched() {
    let mut conn = conn_at(47);
    seed_user_provider_and_link(&conn, "u1", "p1");
    insert_row(&conn, "only-row", "u1", "pending", 3);

    migrations::run_up_to(&mut conn, 48).expect("apply 0048");

    assert!(row_exists(&conn, "only-row"));
}

// ---- forward application, matching tests_rfc096_b1_stage8's own shape ----

#[test]
fn the_migration_applies_forward_from_0047() {
    let mut conn = conn_at(47);
    let before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' \
                 AND name = 'idx_federation_mfa_pending_one_active_per_user'",
            [],
            |r| r.get(0),
        )
        .expect("count before");
    assert_eq!(before, 0, "the index must not exist before 0048");

    migrations::run_up_to(&mut conn, 48).expect("apply 0048");

    let after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' \
                 AND name = 'idx_federation_mfa_pending_one_active_per_user'",
            [],
            |r| r.get(0),
        )
        .expect("count after");
    assert_eq!(after, 1, "the index must exist after 0048");

    let stored_version: i64 = conn
        .query_row(
            "SELECT value FROM sui_meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .expect("schema_version row")
        .parse()
        .expect("integer");
    assert_eq!(stored_version, 48);
}

/// RFC 096-B1 stage 8, item 5's own shape, for this migration's one new
/// schema object: a fresh database at 48 and one forward-migrated from
/// 47 must produce byte-identical index text.
#[test]
fn the_applied_schema_is_identical_fresh_vs_forward_migrated() {
    fn dump(conn: &Connection) -> String {
        conn.query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'idx_federation_mfa_pending_one_active_per_user'",
            [],
            |r| r.get(0),
        )
        .expect("index present")
    }

    let fresh = conn_at(48);
    let mut forward = conn_at(47);
    migrations::run_up_to(&mut forward, 48).expect("apply 0048 forward");

    assert_eq!(
        dump(&fresh),
        dump(&forward),
        "a fresh database at 48 and a database forward-migrated from 47 to 48 \
             must produce byte-identical index text"
    );
}
