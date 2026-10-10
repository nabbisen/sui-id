#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_arguments)]
//! Tests for RFC 096-B1 stage 1 — migration 0046, `federation_login_attempt`.
//!
//! Stage 1 writes no repo functions, so these tests exercise the schema
//! directly through `rusqlite`, the same way `tests_rfc115.rs` exercises
//! migrations 0042/0043.

use crate::migrations;
use rusqlite::Connection;

fn conn_at(version: i32) -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, version).expect("run migrations");
    conn
}

fn conn_at_46() -> Connection {
    conn_at(46)
}

/// A 32-byte value, distinguishable per `tag` so three hash columns in the
/// same row don't collide by accident.
fn hash32(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

const INSERT_SQL: &str = "INSERT INTO federation_login_attempt( \
    id, provider_id, provider_config_version, provider_activation_generation, \
    state_sha256, nonce_sha256, browser_binding_sha256, pkce_verifier_sealed, \
    exact_redirect_uri, next_path, created_at, expires_at, status, claimed_at \
) VALUES (?1, 'p1', ?2, ?3, ?4, ?5, ?6, ?7, 'https://rp.example/cb', NULL, \
          '2026-10-10T00:00:00Z', '2026-10-10T00:10:00Z', ?8, ?9)";

#[allow(clippy::too_many_arguments)]
fn try_insert(
    conn: &Connection,
    id: &str,
    provider_config_version: i64,
    provider_activation_generation: i64,
    state_sha256: &[u8],
    nonce_sha256: &[u8],
    browser_binding_sha256: &[u8],
    status: &str,
    claimed_at: Option<&str>,
) -> rusqlite::Result<usize> {
    conn.execute(
        INSERT_SQL,
        rusqlite::params![
            id,
            provider_config_version,
            provider_activation_generation,
            state_sha256,
            nonce_sha256,
            browser_binding_sha256,
            b"sealed-bytes".as_slice(),
            status,
            claimed_at,
        ],
    )
}

fn try_insert_valid(
    conn: &Connection,
    id: &str,
    status: &str,
    claimed_at: Option<&str>,
) -> rusqlite::Result<usize> {
    try_insert(
        conn,
        id,
        1,
        1,
        &hash32(1),
        &hash32(2),
        &hash32(3),
        status,
        claimed_at,
    )
}

#[test]
fn a_valid_pending_row_is_stored() {
    let conn = conn_at_46();
    try_insert_valid(&conn, "a1", "pending", None).expect("valid pending row");
    let stored: String = conn
        .query_row(
            "SELECT status FROM federation_login_attempt WHERE id = 'a1'",
            [],
            |r| r.get(0),
        )
        .expect("row");
    assert_eq!(stored, "pending");
}

#[test]
fn a_valid_claimed_row_is_stored() {
    let conn = conn_at_46();
    try_insert_valid(&conn, "a1", "exchanging", Some("2026-10-10T00:05:00Z"))
        .expect("valid exchanging row");
}

/// RFC 096-B1 stage 1, item 3: an invalid `status` cannot be stored.
#[test]
fn an_invalid_status_cannot_be_stored() {
    let conn = conn_at_46();
    let err = try_insert_valid(&conn, "a1", "revoked", Some("2026-10-10T00:05:00Z"))
        .expect_err("'revoked' is not a valid status");
    assert!(format!("{err}").contains("CHECK"));
}

#[test]
fn pending_with_a_claimed_at_cannot_be_stored() {
    let conn = conn_at_46();
    try_insert_valid(&conn, "a1", "pending", Some("2026-10-10T00:05:00Z"))
        .expect_err("pending status must not have claimed_at set");
}

#[test]
fn non_pending_without_a_claimed_at_cannot_be_stored() {
    let conn = conn_at_46();
    try_insert_valid(&conn, "a1", "completed", None)
        .expect_err("non-pending status must have claimed_at set");
}

#[test]
fn a_short_state_hash_cannot_be_stored() {
    let conn = conn_at_46();
    try_insert(
        &conn,
        "a1",
        1,
        1,
        &hash32(1)[..31],
        &hash32(2),
        &hash32(3),
        "pending",
        None,
    )
    .expect_err("31-byte state_sha256 must be refused");
}

#[test]
fn a_short_nonce_hash_cannot_be_stored() {
    let conn = conn_at_46();
    try_insert(
        &conn,
        "a1",
        1,
        1,
        &hash32(1),
        &hash32(2)[..31],
        &hash32(3),
        "pending",
        None,
    )
    .expect_err("31-byte nonce_sha256 must be refused");
}

#[test]
fn a_short_browser_binding_hash_cannot_be_stored() {
    let conn = conn_at_46();
    try_insert(
        &conn,
        "a1",
        1,
        1,
        &hash32(1),
        &hash32(2),
        &hash32(3)[..31],
        "pending",
        None,
    )
    .expect_err("31-byte browser_binding_sha256 must be refused");
}

#[test]
fn a_duplicate_state_hash_cannot_be_stored() {
    let conn = conn_at_46();
    try_insert_valid(&conn, "a1", "pending", None).expect("first row");
    let err = try_insert(
        &conn,
        "a2",
        1,
        1,
        &hash32(1), // same state_sha256 as a1
        &hash32(20),
        &hash32(30),
        "pending",
        None,
    )
    .expect_err("a second row with the same state_sha256 must be refused");
    assert!(format!("{err}").contains("UNIQUE") || format!("{err}").contains("unique"));
}

#[test]
fn a_negative_provider_config_version_cannot_be_stored() {
    let conn = conn_at_46();
    try_insert(
        &conn,
        "a1",
        -1,
        1,
        &hash32(1),
        &hash32(2),
        &hash32(3),
        "pending",
        None,
    )
    .expect_err("negative provider_config_version must be refused");
}

#[test]
fn a_negative_activation_generation_cannot_be_stored() {
    let conn = conn_at_46();
    try_insert(
        &conn,
        "a1",
        1,
        -1,
        &hash32(1),
        &hash32(2),
        &hash32(3),
        "pending",
        None,
    )
    .expect_err("negative provider_activation_generation must be refused");
}

/// RFC 096-B1 stage 1, item 2: migration evidence — applied forward against
/// a database already at 0045, not just created fresh at 0046.
#[test]
fn the_migration_applies_forward_from_0045() {
    let mut conn = conn_at(45);
    let before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'federation_login_attempt'",
            [],
            |r| r.get(0),
        )
        .expect("count before");
    assert_eq!(before, 0, "the table must not exist before 0046");

    migrations::run_up_to(&mut conn, 46).expect("apply 0046");

    let after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'federation_login_attempt'",
            [],
            |r| r.get(0),
        )
        .expect("count after");
    assert_eq!(after, 1, "the table must exist after 0046");

    let stored_version: i64 = conn
        .query_row(
            "SELECT value FROM sui_meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .expect("schema_version row")
        .parse()
        .expect("integer");
    assert_eq!(stored_version, 46);
}

/// Dumps the table's own schema (not the whole database) as applied, so the
/// review package can quote the actual, applied `CREATE TABLE` text rather
/// than the source file (which could in principle diverge, e.g. via a typo
/// SQLite itself silently reformats).
#[test]
fn schema_dump_for_evidence() {
    let conn = conn_at_46();
    let sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'federation_login_attempt'",
            [],
            |r| r.get(0),
        )
        .expect("table sql");
    eprintln!("--- federation_login_attempt, as applied ---\n{sql}");
    let index_sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'idx_federation_login_attempt_expires_at'",
            [],
            |r| r.get(0),
        )
        .expect("index sql");
    eprintln!("--- idx_federation_login_attempt_expires_at, as applied ---\n{index_sql}");
}

#[test]
fn max_schema_version_is_46() {
    assert_eq!(migrations::MAX_SCHEMA_VERSION, 46);
}
