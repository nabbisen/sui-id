#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_arguments,
    clippy::panic
)]
//! Tests for RFC 096-B1 stage 8 — migration 0047, `federation_mfa_pending`
//! and `federation_webauthn_ceremony`.
//!
//! Stage 8 writes no repo functions, so these tests exercise the schema
//! directly through `rusqlite`, the same way `tests_rfc096_b1_stage1.rs`
//! exercises migration 0046.

use crate::migrations;
use rusqlite::Connection;

fn conn_at(version: i32) -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, version).expect("run migrations");
    conn
}

fn conn_at_47() -> Connection {
    conn_at(47)
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

/// RFC 096-B1 stage 8 fix round, R3: `federation_mfa_pending` now carries a
/// `FOREIGN KEY (user_id, provider_id) REFERENCES federation_link`, so a
/// pending row (and transitively, deleting the user or the provider) all
/// require a real link row to exist first.
fn seed_link(conn: &Connection, user_id: &str, provider_id: &str) {
    conn.execute(
        "INSERT INTO federation_link(user_id, provider_id, upstream_sub, linked_at, last_seen_at) \
             VALUES (?1, ?2, 'fixture-sub', datetime('now'), datetime('now'))",
        [user_id, provider_id],
    )
    .expect("link");
}

fn seed_user_provider_and_link(conn: &Connection, user_id: &str, provider_id: &str) {
    seed_user(conn, user_id);
    seed_provider(conn, provider_id);
    seed_link(conn, user_id, provider_id);
}

/// A 32-byte value, distinguishable per `tag` so the two hash columns on
/// the same row don't collide by accident.
fn hash32(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

const PENDING_INSERT_SQL: &str = "INSERT INTO federation_mfa_pending( \
    id, provider_id, provider_config_version, provider_activation_generation, user_id, \
    continuation_sealed, next_path, browser_binding_sha256, csrf_binding_sha256, \
    primary_method, failure_count, status, created_at, expires_at \
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";

#[allow(clippy::too_many_arguments)]
fn try_insert_pending(
    conn: &Connection,
    id: &str,
    provider_id: &str,
    provider_config_version: i64,
    provider_activation_generation: i64,
    user_id: &str,
    continuation_sealed: Option<&[u8]>,
    browser_binding_sha256: &[u8],
    csrf_binding_sha256: &[u8],
    primary_method: &str,
    failure_count: i64,
    status: &str,
    created_at: &str,
    expires_at: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        PENDING_INSERT_SQL,
        rusqlite::params![
            id,
            provider_id,
            provider_config_version,
            provider_activation_generation,
            user_id,
            continuation_sealed,
            browser_binding_sha256,
            csrf_binding_sha256,
            primary_method,
            failure_count,
            status,
            created_at,
            expires_at,
        ],
    )
}

/// Timestamps in this project's exact stored `DateTime<Utc>` format
/// (rusqlite's chrono `ToSql`: `"%F %T%.f%:z"`), not a hand-picked
/// approximation — a correctly-computed row must satisfy the `expires_at`
/// `CHECK` using the real format or this suite would prove nothing about
/// it.
const T0: &str = "2026-10-13 10:00:00.000000+00:00";
const T0_PLUS_300: &str = "2026-10-13 10:05:00.000000+00:00";

fn try_insert_pending_valid(
    conn: &Connection,
    id: &str,
    user_id: &str,
    provider_id: &str,
    status: &str,
    continuation_sealed: Option<&[u8]>,
) -> rusqlite::Result<usize> {
    try_insert_pending(
        conn,
        id,
        provider_id,
        0,
        0,
        user_id,
        continuation_sealed,
        &hash32(1),
        &hash32(2),
        "fed",
        0,
        status,
        T0,
        T0_PLUS_300,
    )
}

// ---- federation_mfa_pending ------------------------------------------

#[test]
fn a_valid_pending_row_is_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("valid pending row");
    let stored: String = conn
        .query_row(
            "SELECT status FROM federation_mfa_pending WHERE id = 'm1'",
            [],
            |r| r.get(0),
        )
        .expect("row");
    assert_eq!(stored, "pending");
}

#[test]
fn a_valid_terminal_row_is_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "completed", None)
        .expect("valid completed row, continuation cleared");
}

/// RFC 096-B1 stage 8, item 3: an unknown `status` cannot be stored.
#[test]
fn an_unknown_status_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    let err = try_insert_pending_valid(&conn, "m1", "u1", "p1", "revoked", None)
        .expect_err("'revoked' is not a valid status");
    assert!(format!("{err}").contains("CHECK"));
}

/// RFC 096-B1 stage 8, item 3: "terminal rows retain no usable
/// continuation" — a pending row without a continuation is not
/// representable.
#[test]
fn a_pending_row_without_a_continuation_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", None)
        .expect_err("pending status must still have a usable continuation");
}

/// RFC 096-B1 stage 8, item 3: the converse — a terminal row that still
/// carries a continuation is not representable either.
#[test]
fn a_terminal_row_that_still_carries_a_continuation_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "exhausted", Some(b"sealed"))
        .expect_err("a terminal row must not retain a usable continuation");
}

#[test]
fn a_short_browser_binding_hash_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1)[..31],
        &hash32(2),
        "fed",
        0,
        "pending",
        T0,
        T0_PLUS_300,
    )
    .expect_err("31-byte browser_binding_sha256 must be refused");
}

#[test]
fn a_short_csrf_binding_hash_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2)[..31],
        "fed",
        0,
        "pending",
        T0,
        T0_PLUS_300,
    )
    .expect_err("31-byte csrf_binding_sha256 must be refused");
}

/// RFC 096-B1 stage 8, item 4 ("the `STRICT` demonstration"): a 64-char
/// hex `TEXT` value the same *length* as a 32-byte `BLOB` is still
/// refused outright, because `STRICT` rejects the storage class itself.
#[test]
fn a_text_value_cannot_be_stored_where_a_blob_is_expected() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    let thirty_two_ascii_bytes: &str = "0123456789012345678901234567890a";
    assert_eq!(
        thirty_two_ascii_bytes.len(),
        32,
        "the TEXT value under test must itself be exactly 32 bytes, so a bare \
             length(...) = 32 CHECK (with no STRICT) would have let it through"
    );
    let err = conn
        .execute(
            "INSERT INTO federation_mfa_pending( \
                 id, provider_id, provider_config_version, provider_activation_generation, \
                 user_id, continuation_sealed, next_path, browser_binding_sha256, \
                 csrf_binding_sha256, primary_method, failure_count, status, created_at, \
                 expires_at) \
             VALUES ('m1','p1',0,0,'u1',?1, NULL, ?2, ?3, 'fed',0,'pending',?4,?5)",
            rusqlite::params![
                b"sealed".as_slice(),
                thirty_two_ascii_bytes,
                hash32(2),
                T0,
                T0_PLUS_300
            ],
        )
        .expect_err("a TEXT value must be refused where browser_binding_sha256 (BLOB) is expected");
    assert!(format!("{err}").contains("BLOB"));
}

#[test]
fn a_negative_provider_config_version_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        -1,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        0,
        "pending",
        T0,
        T0_PLUS_300,
    )
    .expect_err("negative provider_config_version must be refused");
}

#[test]
fn a_negative_activation_generation_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        -1,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        0,
        "pending",
        T0,
        T0_PLUS_300,
    )
    .expect_err("negative provider_activation_generation must be refused");
}

/// RFC 096-B1 stage 8, item 3: `failure_count` constrained 0-5 — 6 is
/// refused.
#[test]
fn a_failure_count_of_six_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        6,
        "pending",
        T0,
        T0_PLUS_300,
    )
    .expect_err("failure_count = 6 must be refused");
}

/// RFC 096-B1 stage 8, item 3: `failure_count` constrained 0-5 — -1 is
/// refused.
#[test]
fn a_failure_count_of_negative_one_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        -1,
        "pending",
        T0,
        T0_PLUS_300,
    )
    .expect_err("failure_count = -1 must be refused");
}

#[test]
fn failure_count_five_is_the_allowed_maximum() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        5,
        "pending",
        T0,
        T0_PLUS_300,
    )
    .expect("failure_count = 5 is within range");
}

/// `primary_method` is pinned to `'fed'`; any other value is refused.
#[test]
fn a_primary_method_other_than_fed_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "pwd",
        0,
        "pending",
        T0,
        T0_PLUS_300,
    )
    .expect_err("primary_method other than 'fed' must be refused");
}

/// RFC 096-B1 stage 8, item 3: `expires_at = created_at + 300 seconds`.
/// 301 seconds is refused.
#[test]
fn an_expiry_more_than_300_seconds_after_creation_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("sanity: exactly 300s is accepted");
    try_insert_pending(
        &conn,
        "m2",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        0,
        "pending",
        T0,
        "2026-10-13 10:05:01.000000+00:00",
    )
    .expect_err("expires_at 301 seconds after created_at must be refused");
}

#[test]
fn an_expiry_less_than_300_seconds_after_creation_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        0,
        "pending",
        T0,
        "2026-10-13 10:04:59.000000+00:00",
    )
    .expect_err("expires_at 299 seconds after created_at must be refused");
}

/// The `expires_at` `CHECK` compares whole-second Unix-epoch values, not
/// strings — demonstrated against a timestamp with no fractional part at
/// all and one that crosses a UTC midnight boundary.
#[test]
fn the_expiry_check_tolerates_no_fractional_seconds_and_a_midnight_boundary() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        0,
        "pending",
        "2026-10-13 10:00:00+00:00",
        "2026-10-13 10:05:00+00:00",
    )
    .expect("no fractional part is still a valid 300-second gap");
    try_insert_pending(
        &conn,
        "m2",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(3),
        &hash32(4),
        "fed",
        0,
        "pending",
        "2026-10-13 23:58:00.5+00:00",
        "2026-10-14 00:03:00.5+00:00",
    )
    .expect("a 300-second gap crossing midnight is still valid");
}

/// RFC 096-B1 stage 8 fix round, R1: an unparseable `created_at` makes
/// `strftime('%s', created_at)` NULL, which would otherwise make the
/// whole `+300` comparison NULL — and a SQLite `CHECK` passes on NULL.
/// The `IS NOT NULL` guards are what turns this into a refusal.
#[test]
fn an_unparseable_created_at_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        0,
        "pending",
        "not-a-timestamp",
        "also-garbage",
    )
    .expect_err("an unparseable created_at must be refused, not silently pass the CHECK as NULL");
}

/// RFC 096-B1 stage 8 fix round, R1: the converse — an unparseable
/// `expires_at` alone.
#[test]
fn an_unparseable_expires_at_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        0,
        "pending",
        T0,
        "also-garbage",
    )
    .expect_err("an unparseable expires_at must be refused, not silently pass the CHECK as NULL");
}

/// RFC 096-B1 stage 8 fix round, R1: empty strings are themselves
/// unparseable and must be refused the same way.
#[test]
fn empty_string_timestamps_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending(
        &conn,
        "m1",
        "p1",
        0,
        0,
        "u1",
        Some(b"sealed"),
        &hash32(1),
        &hash32(2),
        "fed",
        0,
        "pending",
        "",
        "",
    )
    .expect_err("empty-string timestamps must be refused");
}

/// RFC 096-B1 stage 8 fix round, R3: `(user_id, provider_id)` now carries
/// a `FOREIGN KEY` to `federation_link`. A pair that was never linked
/// cannot host a pending row.
#[test]
fn a_pending_row_for_an_unlinked_pair_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user(&conn, "u1");
    seed_provider(&conn, "p1");
    // Deliberately no seed_link: user and provider both exist, but are
    // never linked.
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect_err("a pending row for an unlinked (user, provider) pair must be refused");
}

/// FOREIGN KEY on `(user_id, provider_id)`: deleting the user cascades
/// the pending row transitively, through `federation_link`'s own cascade
/// on `user_id` (migration 0038).
#[test]
fn deleting_the_user_cascades_the_pending_row() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("valid row");
    conn.execute("DELETE FROM users WHERE id = 'u1'", [])
        .expect("delete user");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM federation_mfa_pending WHERE id = 'm1'",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(count, 0, "the pending row must cascade with the user");
}

/// FOREIGN KEY on `(user_id, provider_id)`: deleting the provider
/// cascades the pending row transitively, through `federation_link`'s
/// own cascade on `provider_id` (migration 0038). Not hypothetical:
/// `federation_provider::delete` (`repos/federation_provider.rs:207`) is
/// a real, reachable operation.
#[test]
fn deleting_the_provider_cascades_the_pending_row() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("valid row");
    conn.execute("DELETE FROM federation_provider WHERE id = 'p1'", [])
        .expect("delete provider");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM federation_mfa_pending WHERE id = 'm1'",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(count, 0, "the pending row must cascade with the provider");
}

/// RFC 096-B1 stage 8 fix round, R3: unlinking (deleting the
/// `federation_link` row directly) takes the live pending challenge with
/// it — the security-relevant half of this FK. `federation_link::delete`
/// (`repos/federation_link.rs:162`) has no caller today (grepped, not
/// assumed), so this is a property for when the self-service unlink is
/// wired, demonstrated at the schema level regardless.
#[test]
fn unlinking_cascades_the_pending_row() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("valid row");
    conn.execute(
        "DELETE FROM federation_link WHERE user_id = 'u1' AND provider_id = 'p1'",
        [],
    )
    .expect("unlink");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM federation_mfa_pending WHERE id = 'm1'",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(count, 0, "the pending row must cascade with the link");
}

// ---- federation_webauthn_ceremony -------------------------------------

const CEREMONY_INSERT_SQL: &str = "INSERT INTO federation_webauthn_ceremony( \
    id, pending_id, provider_id, provider_config_version, provider_activation_generation, \
    user_id, ceremony_state_sealed, rp_id, origin, created_at, pending_expires_at, expires_at \
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'id.example', 'https://id.example', ?8, ?9, ?10)";

#[allow(clippy::too_many_arguments)]
fn try_insert_ceremony(
    conn: &Connection,
    id: &str,
    pending_id: &str,
    provider_id: &str,
    provider_config_version: i64,
    provider_activation_generation: i64,
    user_id: &str,
    created_at: &str,
    pending_expires_at: &str,
    expires_at: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        CEREMONY_INSERT_SQL,
        rusqlite::params![
            id,
            pending_id,
            provider_id,
            provider_config_version,
            provider_activation_generation,
            user_id,
            b"ceremony-state".as_slice(),
            created_at,
            pending_expires_at,
            expires_at,
        ],
    )
}

/// `pending_expires_at` must agree with the parent's *real, current*
/// `expires_at` (`T0_PLUS_300`) for the composite FK to resolve.
const CEREMONY_EXPIRES_AT: &str = "2026-10-13 10:04:00.000000+00:00";

#[test]
fn a_valid_ceremony_row_is_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect("valid ceremony row");
}

/// `pending_id` is part of the composite `FOREIGN KEY`: a ceremony cannot
/// be stored without a real parent row.
#[test]
fn a_ceremony_without_a_parent_pending_row_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_ceremony(
        &conn,
        "c1",
        "does-not-exist",
        "p1",
        0,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect_err("a ceremony with no matching pending row must be refused");
}

/// RFC 096-B1 stage 8, item 1: "unique parent-pending key" — a second
/// ceremony for the same pending row is refused.
#[test]
fn a_second_ceremony_for_the_same_pending_row_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect("first ceremony");
    let err = try_insert_ceremony(
        &conn,
        "c2",
        "m1",
        "p1",
        0,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect_err("a second ceremony for the same pending row must be refused");
    assert!(format!("{err}").contains("UNIQUE") || format!("{err}").contains("unique"));
}

/// RFC 096-B1 stage 8 fix round, R2: the composite `FOREIGN KEY` refuses
/// a ceremony whose `user_id` does not match the parent's — attempted
/// against a pending row with no ceremony yet, so the failure isolates to
/// the FK rather than colliding with `pending_id`'s own `UNIQUE`.
#[test]
fn a_ceremony_with_a_user_id_other_than_the_parents_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    seed_user(&conn, "attacker");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    let err = try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "attacker",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect_err("a ceremony whose user_id disagrees with its parent's must be refused");
    assert!(format!("{err}").contains("FOREIGN KEY"));
}

/// RFC 096-B1 stage 8 fix round, R2: the composite `FOREIGN KEY` refuses
/// a ceremony whose `provider_config_version` does not match the
/// parent's.
#[test]
fn a_ceremony_with_a_provider_config_version_other_than_the_parents_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    let err = try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        99,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect_err(
        "a ceremony whose provider_config_version disagrees with its parent's must be refused",
    );
    assert!(format!("{err}").contains("FOREIGN KEY"));
}

/// RFC 096-B1 stage 8 fix round, R2: an untruthful `pending_expires_at`
/// (not the parent's real, current `expires_at`) is refused by the
/// composite `FOREIGN KEY` — chosen so the ceremony's own local
/// `expires_at <= pending_expires_at` CHECK still passes, isolating the
/// failure to the FK rather than the CHECK.
#[test]
fn a_ceremony_with_an_untruthful_pending_expires_at_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    let lied_pending_expires_at = "2026-10-13 23:00:00.000000+00:00";
    let err = try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "u1",
        T0,
        lied_pending_expires_at,
        CEREMONY_EXPIRES_AT,
    )
    .expect_err(
        "a ceremony whose pending_expires_at is not the parent's real value must be refused",
    );
    assert!(format!("{err}").contains("FOREIGN KEY"));
}

/// RFC 096-B1 stage 8 fix round, R2: an `expires_at` later than the
/// parent's (here, a truthful `pending_expires_at` but a local `expires_at`
/// that exceeds it) is refused by the ceremony's own `CHECK`, not the FK.
#[test]
fn a_ceremony_expiry_later_than_its_parents_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    let one_minute_after_parent = "2026-10-13 10:06:00.000000+00:00";
    let err = try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        one_minute_after_parent,
    )
    .expect_err("a ceremony expiring after its parent must be refused");
    assert!(format!("{err}").contains("CHECK"));
}

/// RFC 096-B1 stage 8 fix round, R2: an unparseable ceremony `created_at`
/// is refused, the same `IS NOT NULL` treatment as R1.
#[test]
fn an_unparseable_ceremony_created_at_cannot_be_stored() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "u1",
        "not-a-timestamp",
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect_err("an unparseable ceremony created_at must be refused");
}

/// RFC 096-B1 stage 8 fix round, R2: a property that falls out of the
/// composite FK rather than being separately asked for — while a
/// ceremony exists, the parent's `expires_at` cannot be changed, because
/// the ceremony's `pending_expires_at` would then name a tuple that no
/// longer exists on the parent.
#[test]
fn extending_a_pending_rows_expiry_is_refused_while_a_ceremony_exists() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect("ceremony");
    let err = conn
        .execute(
            "UPDATE federation_mfa_pending SET created_at = ?1, expires_at = ?2 WHERE id = 'm1'",
            [
                "2026-10-13 10:01:00.000000+00:00",
                "2026-10-13 10:06:00.000000+00:00",
            ],
        )
        .expect_err(
            "extending expires_at while a live ceremony references the old value must be refused",
        );
    assert!(format!("{err}").contains("FOREIGN KEY"));
}

/// The converse of the above: terminalizing the parent (columns outside
/// the composite FK) succeeds even while a ceremony exists.
#[test]
fn terminalizing_a_pending_row_succeeds_while_a_ceremony_exists() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect("ceremony");
    conn.execute(
        "UPDATE federation_mfa_pending SET status = 'completed', continuation_sealed = NULL \
             WHERE id = 'm1'",
        [],
    )
    .expect("terminalizing the parent must not be blocked by the ceremony's FK");
}

/// Deleting the parent pending row cascades the ceremony.
#[test]
fn deleting_the_pending_row_cascades_the_ceremony() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect("ceremony");
    conn.execute("DELETE FROM federation_mfa_pending WHERE id = 'm1'", [])
        .expect("delete pending row");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM federation_webauthn_ceremony WHERE id = 'c1'",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(
        count, 0,
        "the ceremony must cascade with its parent pending row"
    );
}

/// RFC 096-B1 stage 8 fix round, R3/R2: unlinking cascades the pending
/// row, which transitively cascades the ceremony.
#[test]
fn unlinking_cascades_the_pending_row_and_its_ceremony() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    try_insert_pending_valid(&conn, "m1", "u1", "p1", "pending", Some(b"sealed"))
        .expect("parent pending row");
    try_insert_ceremony(
        &conn,
        "c1",
        "m1",
        "p1",
        0,
        0,
        "u1",
        T0,
        T0_PLUS_300,
        CEREMONY_EXPIRES_AT,
    )
    .expect("ceremony");
    conn.execute(
        "DELETE FROM federation_link WHERE user_id = 'u1' AND provider_id = 'p1'",
        [],
    )
    .expect("unlink");
    let pending: i64 = conn
        .query_row("SELECT COUNT(*) FROM federation_mfa_pending", [], |r| {
            r.get(0)
        })
        .expect("count");
    let ceremony: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM federation_webauthn_ceremony",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(pending, 0, "the pending row must cascade with the link");
    assert_eq!(
        ceremony, 0,
        "the ceremony must cascade transitively with the link"
    );
}

/// RFC 096-B1 stage 8 fix round, added by the architect in review: the
/// whole chain `users -> federation_link -> federation_mfa_pending ->
/// federation_webauthn_ceremony`, measured end to end and with a
/// control. The two single-hop cascade tests above create no ceremony,
/// so "the ceremony goes too, transitively" was composed from separate
/// tests rather than measured in one; and without a second, untouched
/// chain in the same database, three `COUNT(*) = 0` assertions pass
/// equally well if the delete removed everything.
#[test]
fn deleting_one_user_cascades_its_whole_chain_and_leaves_anothers() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    seed_user(&conn, "u2");
    // `seed_link` uses a fixed `upstream_sub`, and `federation_link` has
    // `UNIQUE (provider_id, upstream_sub)` (migration 0038), so the
    // control user's link to the same provider needs its own.
    conn.execute(
        "INSERT INTO federation_link(user_id, provider_id, upstream_sub, linked_at, last_seen_at) \
             VALUES ('u2', 'p1', 'fixture-sub-2', datetime('now'), datetime('now'))",
        [],
    )
    .expect("control link");
    for (pending, ceremony, user) in [("m1", "c1", "u1"), ("m2", "c2", "u2")] {
        try_insert_pending_valid(&conn, pending, user, "p1", "pending", Some(b"sealed"))
            .expect("pending row");
        try_insert_ceremony(
            &conn,
            ceremony,
            pending,
            "p1",
            0,
            0,
            user,
            T0,
            T0_PLUS_300,
            CEREMONY_EXPIRES_AT,
        )
        .expect("ceremony");
    }
    conn.execute("DELETE FROM users WHERE id = 'u1'", [])
        .expect("delete user");
    assert_eq!(
        surviving_chain(&conn),
        vec!["u2/m2/c2".to_string()],
        "u1's link, pending row and ceremony must all go; u2's must all stay"
    );
}

/// The same chain and the same control, driven from the live end:
/// `federation_provider::delete` (`repos/federation_provider.rs:207`) is
/// reachable today, unlike the self-service unlink.
#[test]
fn deleting_one_provider_cascades_its_whole_chain_and_leaves_anothers() {
    let conn = conn_at_47();
    seed_user_provider_and_link(&conn, "u1", "p1");
    seed_user_provider_and_link(&conn, "u2", "p2");
    for (pending, ceremony, user, provider) in [("m1", "c1", "u1", "p1"), ("m2", "c2", "u2", "p2")]
    {
        try_insert_pending_valid(&conn, pending, user, provider, "pending", Some(b"sealed"))
            .expect("pending row");
        try_insert_ceremony(
            &conn,
            ceremony,
            pending,
            provider,
            0,
            0,
            user,
            T0,
            T0_PLUS_300,
            CEREMONY_EXPIRES_AT,
        )
        .expect("ceremony");
    }
    conn.execute("DELETE FROM federation_provider WHERE id = 'p1'", [])
        .expect("delete provider");
    assert_eq!(
        surviving_chain(&conn),
        vec!["u2/m2/c2".to_string()],
        "p1's link, pending row and ceremony must all go; p2's must all stay"
    );
}

/// Every surviving link with the pending row and ceremony still hanging
/// off it, as `user/pending/ceremony`. A join rather than three counts:
/// it shows what remains, so a delete that removed too much and one that
/// removed too little fail differently.
fn surviving_chain(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT l.user_id || '/' || m.id || '/' || c.id \
                 FROM federation_link l \
                 JOIN federation_mfa_pending m \
                   ON m.user_id = l.user_id AND m.provider_id = l.provider_id \
                 JOIN federation_webauthn_ceremony c ON c.pending_id = m.id \
                 ORDER BY l.user_id",
        )
        .expect("prepare");
    stmt.query_map([], |r| r.get::<_, String>(0))
        .expect("query")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("rows")
}

// ---- migration evidence ------------------------------------------------

/// RFC 096-B1 stage 8, item 5: migration evidence — applied forward
/// against a database already at 0046, not just created fresh at 0047.
#[test]
fn the_migration_applies_forward_from_0046() {
    let mut conn = conn_at(46);
    let before: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' \
                 AND name IN ('federation_mfa_pending', 'federation_webauthn_ceremony')",
            [],
            |r| r.get(0),
        )
        .expect("count before");
    assert_eq!(before, 0, "neither table must exist before 0047");

    migrations::run_up_to(&mut conn, 47).expect("apply 0047");

    let after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' \
                 AND name IN ('federation_mfa_pending', 'federation_webauthn_ceremony')",
            [],
            |r| r.get(0),
        )
        .expect("count after");
    assert_eq!(after, 2, "both tables must exist after 0047");

    let stored_version: i64 = conn
        .query_row(
            "SELECT value FROM sui_meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .expect("schema_version row")
        .parse()
        .expect("integer");
    assert_eq!(stored_version, 47);
}

/// RFC 096-B1 stage 8, item 5: "the migration applies to a fresh database
/// and to one at 0046, and the result is identical" — checked by dumping
/// `sqlite_master.sql` for both new tables and all three new indexes out
/// of each database and comparing the two sets of strings directly.
#[test]
fn the_applied_schema_is_identical_fresh_vs_forward_migrated() {
    fn dump(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT sql FROM sqlite_master \
                     WHERE name IN ('federation_mfa_pending', 'federation_webauthn_ceremony', \
                                     'idx_federation_mfa_pending_expires_at', \
                                     'idx_federation_webauthn_ceremony_expires_at', \
                                     'ux_federation_mfa_pending_binding') \
                     ORDER BY name",
            )
            .expect("prepare");
        stmt.query_map([], |r| r.get::<_, String>(0))
            .expect("query")
            .map(|r| r.expect("row"))
            .collect()
    }

    let fresh = conn_at(47);
    let mut forward = conn_at(46);
    migrations::run_up_to(&mut forward, 47).expect("apply 0047 forward");

    let fresh_dump = dump(&fresh);
    let forward_dump = dump(&forward);
    assert_eq!(
        fresh_dump.len(),
        5,
        "all five new schema objects must be present"
    );
    assert_eq!(
        fresh_dump, forward_dump,
        "a fresh database at 47 and a database forward-migrated from 46 to 47 \
             must produce byte-identical schema text"
    );
}

/// Dumps both tables' and all three indexes' own schema (not the whole
/// database) as applied, so the review package can quote the actual,
/// applied `CREATE TABLE` text rather than the source file.
#[test]
fn schema_dump_for_evidence() {
    let conn = conn_at_47();
    for name in [
        "federation_mfa_pending",
        "federation_webauthn_ceremony",
        "idx_federation_mfa_pending_expires_at",
        "idx_federation_webauthn_ceremony_expires_at",
        "ux_federation_mfa_pending_binding",
    ] {
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = ?1",
                [name],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| panic!("{name} missing"));
        eprintln!("--- {name}, as applied ---\n{sql}");
    }
}

/// RFC 096-B1 stage 8 fix round, R4: a replacement for the removed
/// `max_schema_version_is_46`/`_47` canaries that does not itself need
/// editing by every future migration. `>= 47` still fails if `0047` is
/// never registered in `MIGRATIONS` (the thing worth catching), and it
/// stays true once `0048` or later lands.
#[test]
fn max_schema_version_is_at_least_47() {
    // `MAX_SCHEMA_VERSION` is itself a compile-time const, so clippy
    // (rightly) wants this evaluated as one too: a `const { assert!(..) }`
    // fails the whole crate's compilation, not just this test, if a future
    // change ever makes the computed ceiling regress below 47.
    const { assert!(migrations::MAX_SCHEMA_VERSION >= 47) };
}

/// RFC 096-B1 stage 8 fix round, R6: RFC 096 `:738`'s "generic password/
/// WebAuthn pending rows cannot reference it" holds today only because
/// `login_pending_mfa` (migration 0003) and `webauthn_pending` (migration
/// 0004, rebuilt by 0013) have no column of any kind that could name a
/// `federation_mfa_pending`/`federation_webauthn_ceremony` row. Nothing in
/// SQLite stops a future migration from adding one. Pinning the exact
/// column set gives that absence a named, failing test instead of a
/// comment a future migration's author has to remember: a migration that
/// adds a column to either table — `federation_ceremony_id` or anything
/// else — fails this test, which is the point.
#[test]
fn the_two_generic_pending_tables_gain_no_federation_reference() {
    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT name FROM pragma_table_info('{table}') ORDER BY cid"
            ))
            .expect("prepare");
        stmt.query_map([], |r| r.get::<_, String>(0))
            .expect("query")
            .map(|r| r.expect("row"))
            .collect()
    }

    let conn = conn_at_47();
    assert_eq!(
        columns(&conn, "login_pending_mfa"),
        vec!["id", "user_id", "expires_at", "created_at"],
    );
    assert_eq!(
        columns(&conn, "webauthn_pending"),
        vec![
            "id",
            "kind",
            "user_id",
            "state_json",
            "expires_at",
            "created_at"
        ],
    );
}
