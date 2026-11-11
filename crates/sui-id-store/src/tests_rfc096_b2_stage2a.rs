#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_arguments,
    clippy::panic
)]
//! Tests for RFC 096-B2 stage 2a — migration 0051, `federation_provider`'s
//! table rebuild: `id_token_algs`, and the table `CHECK` that makes
//! "enabled" mean "validated".
//!
//! Seeded through raw `rusqlite` at a real pre-0051 (version 50) schema,
//! the same technique `tests_rfc096_b2_stage1.rs` uses for migration
//! 0050's own upgrade behaviour.

use crate::crypto::{self, MasterKey};
use crate::migrations;
use rusqlite::Connection;

fn conn_at(version: i32) -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, version).expect("run migrations");
    conn
}

fn conn_at_50() -> Connection {
    conn_at(50)
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

fn seed_link(conn: &Connection, user_id: &str, provider_id: &str) {
    conn.execute(
        "INSERT INTO federation_link(user_id, provider_id, upstream_sub, linked_at, last_seen_at) \
             VALUES (?1, ?2, ?1 || '-sub', datetime('now'), datetime('now'))",
        [user_id, provider_id],
    )
    .expect("link");
}

/// Seeds a `federation_provider` row at the version-50 column shape (the
/// table this migration rebuilds *from*). `enabled`/`client_secret_enc`
/// are the only two fields this file's tests vary; every other column
/// takes its own schema `DEFAULT`.
fn seed_provider_v50(conn: &Connection, id: &str, enabled: i64, client_secret_enc: Option<&[u8]>) {
    conn.execute(
        "INSERT INTO federation_provider \
             (id, slug, display_name, issuer, client_id, client_secret_enc, enabled, \
              created_at, updated_at) \
             VALUES (?1, ?1, 'Provider', 'https://issuer.example', 'client', ?2, ?3, \
                      '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        rusqlite::params![id, client_secret_enc, enabled],
    )
    .expect("provider");
}

fn hash32(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

fn seed_attempt(conn: &Connection, id: &str, provider_id: &str, tag: u8) {
    conn.execute(
        "INSERT INTO federation_login_attempt( \
             id, provider_id, provider_config_version, provider_activation_generation, \
             state_sha256, nonce_sha256, browser_binding_sha256, pkce_verifier_sealed, \
             exact_redirect_uri, next_path, created_at, expires_at, status, claimed_at) \
         VALUES (?1, ?2, 0, 0, ?3, ?4, ?5, ?6, 'https://rp.example/cb', NULL, \
                  '2026-01-01T00:00:00Z', '2026-01-01T00:10:00Z', 'pending', NULL)",
        rusqlite::params![
            id,
            provider_id,
            hash32(tag),
            hash32(tag.wrapping_add(100)),
            hash32(tag.wrapping_add(200)),
            b"sealed-pkce".as_slice(),
        ],
    )
    .expect("attempt");
}

const PENDING_EXPIRES_AT: &str = "2026-01-01T00:05:00Z";

fn seed_mfa_pending(conn: &Connection, id: &str, provider_id: &str, user_id: &str, tag: u8) {
    conn.execute(
        "INSERT INTO federation_mfa_pending( \
             id, provider_id, provider_config_version, provider_activation_generation, user_id, \
             continuation_sealed, next_path, browser_binding_sha256, csrf_binding_sha256, \
             primary_method, failure_count, status, created_at, expires_at) \
         VALUES (?1, ?2, 0, 0, ?3, ?4, NULL, ?5, ?6, 'fed', 0, 'pending', \
                  '2026-01-01T00:00:00Z', ?7)",
        rusqlite::params![
            id,
            provider_id,
            user_id,
            b"continuation".as_slice(),
            hash32(tag),
            hash32(tag.wrapping_add(100)),
            PENDING_EXPIRES_AT,
        ],
    )
    .expect("mfa_pending");
}

fn seed_ceremony(conn: &Connection, id: &str, pending_id: &str, provider_id: &str, user_id: &str) {
    conn.execute(
        "INSERT INTO federation_webauthn_ceremony( \
             id, pending_id, provider_id, provider_config_version, provider_activation_generation, \
             user_id, ceremony_state_sealed, rp_id, origin, created_at, pending_expires_at, expires_at) \
         VALUES (?1, ?2, ?3, 0, 0, ?4, ?5, 'id.example', 'https://id.example', \
                  '2026-01-01T00:00:00Z', ?6, '2026-01-01T00:04:00Z')",
        rusqlite::params![
            id,
            pending_id,
            provider_id,
            user_id,
            b"sealed-ceremony".as_slice(),
            PENDING_EXPIRES_AT,
        ],
    )
    .expect("ceremony");
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .expect("count")
}

// ── The rebuild keeps the children ──────────────────────────────────────

/// Fixture: a provider with a sealed secret, links, an attempt, an MFA
/// continuation and a ceremony, all seeded before 0051. Every child row
/// must survive the rebuild, every id unchanged, and the sealed secret
/// must still decrypt to the same plaintext. Counts before and after.
#[test]
fn the_rebuild_keeps_the_children_ids_unchanged_secret_decryptable() {
    let key = MasterKey::generate();
    let plaintext = b"super-secret-client-credential";
    let sealed =
        crypto::seal(&key, plaintext, crate::repos::federation_provider::AAD).expect("seal");

    let mut conn = conn_at_50();
    seed_provider_v50(&conn, "p1", 0, Some(&sealed));
    seed_user(&conn, "u1");
    seed_link(&conn, "u1", "p1");
    seed_attempt(&conn, "a1", "p1", 1);
    seed_mfa_pending(&conn, "m1", "p1", "u1", 10);
    seed_ceremony(&conn, "c1", "m1", "p1", "u1");

    let before = (
        count(&conn, "federation_provider"),
        count(&conn, "federation_link"),
        count(&conn, "federation_login_attempt"),
        count(&conn, "federation_mfa_pending"),
        count(&conn, "federation_webauthn_ceremony"),
    );
    assert_eq!(before, (1, 1, 1, 1, 1), "sanity: every child seeded");

    migrations::run_up_to(&mut conn, 51).expect("apply 0051");

    let after = (
        count(&conn, "federation_provider"),
        count(&conn, "federation_link"),
        count(&conn, "federation_login_attempt"),
        count(&conn, "federation_mfa_pending"),
        count(&conn, "federation_webauthn_ceremony"),
    );
    assert_eq!(before, after, "every child row must survive the rebuild");

    // IDs unchanged.
    assert!(
        conn.query_row(
            "SELECT 1 FROM federation_provider WHERE id = 'p1'",
            [],
            |_| Ok(())
        )
        .is_ok()
    );
    assert!(
        conn.query_row(
            "SELECT 1 FROM federation_link WHERE user_id = 'u1' AND provider_id = 'p1'",
            [],
            |_| Ok(())
        )
        .is_ok()
    );
    assert!(
        conn.query_row(
            "SELECT 1 FROM federation_login_attempt WHERE id = 'a1'",
            [],
            |_| Ok(())
        )
        .is_ok()
    );
    assert!(
        conn.query_row(
            "SELECT 1 FROM federation_mfa_pending WHERE id = 'm1'",
            [],
            |_| Ok(())
        )
        .is_ok()
    );
    assert!(
        conn.query_row(
            "SELECT 1 FROM federation_webauthn_ceremony WHERE id = 'c1'",
            [],
            |_| Ok(())
        )
        .is_ok()
    );

    // The sealed secret still decrypts to the same plaintext.
    let reread_sealed: Vec<u8> = conn
        .query_row(
            "SELECT client_secret_enc FROM federation_provider WHERE id = 'p1'",
            [],
            |r| r.get(0),
        )
        .expect("sealed secret present");
    assert_eq!(reread_sealed, sealed, "sealed bytes must be untouched");
    let opened = crypto::open(&key, &reread_sealed, crate::repos::federation_provider::AAD)
        .expect("the secret must still decrypt");
    assert_eq!(opened, plaintext);
}

// ── The CHECK, both sides ───────────────────────────────────────────────

/// The evidence-shaped half of an enabled row's own columns -- every
/// field the `CHECK`'s `enabled = 0 OR (...)` branch tests, as one
/// struct so each per-conjunct test below can break exactly one with
/// `..Default::default()`.
struct EnabledRow {
    validation_status: &'static str,
    id_token_algs: Option<&'static str>,
    evidence_config_version: Option<i64>,
    evidence_activation_generation: Option<i64>,
    evidence_policy_digest: Option<Vec<u8>>,
    evidence_observed_at: Option<&'static str>,
    evidence_metadata_fingerprint: Option<Vec<u8>>,
    evidence_jwks_fingerprint: Option<Vec<u8>>,
}

impl Default for EnabledRow {
    /// A row that satisfies every conjunct: `config_version`/
    /// `activation_generation` are both `1` (set in the `INSERT` body
    /// below), and the evidence fields here equal them exactly.
    fn default() -> Self {
        Self {
            validation_status: "validated",
            id_token_algs: Some("RS256"),
            evidence_config_version: Some(1),
            evidence_activation_generation: Some(1),
            evidence_policy_digest: Some(hash32(1)),
            evidence_observed_at: Some("2026-01-01T00:00:00Z"),
            evidence_metadata_fingerprint: Some(hash32(2)),
            evidence_jwks_fingerprint: Some(hash32(3)),
        }
    }
}

/// Inserts an otherwise-valid enabled row at a fresh version-51 table
/// with `row`'s own fields, returning whether the `INSERT` was accepted.
fn try_insert_enabled(row: &EnabledRow) -> bool {
    let conn = conn_at(51);
    conn.execute(
        "INSERT INTO federation_provider \
         (id, slug, display_name, issuer, client_id, scopes, provision_mode, enabled, \
          created_at, updated_at, allowed_origins, config_version, activation_generation, \
          validation_status, evidence_config_version, evidence_activation_generation, \
          evidence_policy_digest, evidence_observed_at, evidence_metadata_fingerprint, \
          evidence_jwks_fingerprint, id_token_algs) \
         VALUES ('p1', 's', 'd', 'https://i', 'c', 'openid', 'link_only', 1, \
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '', 1, 1, \
                  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            row.validation_status,
            row.evidence_config_version,
            row.evidence_activation_generation,
            row.evidence_policy_digest,
            row.evidence_observed_at,
            row.evidence_metadata_fingerprint,
            row.evidence_jwks_fingerprint,
            row.id_token_algs,
        ],
    )
    .is_ok()
}

#[test]
fn an_enabled_row_with_everything_present_is_accepted() {
    assert!(try_insert_enabled(&EnabledRow::default()));
}

#[test]
fn not_validated_is_refused() {
    assert!(!try_insert_enabled(&EnabledRow {
        validation_status: "requires_review",
        ..Default::default()
    }));
}

#[test]
fn missing_id_token_algs_is_refused() {
    assert!(!try_insert_enabled(&EnabledRow {
        id_token_algs: None,
        ..Default::default()
    }));
}

#[test]
fn evidence_config_version_mismatch_is_refused() {
    assert!(!try_insert_enabled(&EnabledRow {
        evidence_config_version: Some(2),
        ..Default::default()
    }));
}

#[test]
fn evidence_activation_generation_mismatch_is_refused() {
    assert!(!try_insert_enabled(&EnabledRow {
        evidence_activation_generation: Some(2),
        ..Default::default()
    }));
}

#[test]
fn missing_evidence_policy_digest_is_refused() {
    assert!(!try_insert_enabled(&EnabledRow {
        evidence_policy_digest: None,
        ..Default::default()
    }));
}

#[test]
fn missing_evidence_observed_at_is_refused() {
    assert!(!try_insert_enabled(&EnabledRow {
        evidence_observed_at: None,
        ..Default::default()
    }));
}

#[test]
fn missing_evidence_metadata_fingerprint_is_refused() {
    assert!(!try_insert_enabled(&EnabledRow {
        evidence_metadata_fingerprint: None,
        ..Default::default()
    }));
}

#[test]
fn missing_evidence_jwks_fingerprint_is_refused() {
    assert!(!try_insert_enabled(&EnabledRow {
        evidence_jwks_fingerprint: None,
        ..Default::default()
    }));
}

#[test]
fn a_disabled_row_with_nothing_present_is_accepted() {
    let conn = conn_at(51);
    conn.execute(
        "INSERT INTO federation_provider \
         (id, slug, display_name, issuer, client_id, enabled, created_at, updated_at) \
         VALUES ('p1', 's', 'd', 'https://i', 'c', 0, \
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    )
    .expect("a disabled row needs none of the enabled-row fields");
}

// ── The C16 shape migrates ──────────────────────────────────────────────

/// Fixture: an enabled, `requires_review` row with no evidence -- the
/// row C16 creates today (before S4 exists to ever write `validated`).
/// 0051 must succeed and the row must be disabled after it, not refuse
/// the migration outright.
#[test]
fn the_c16_shape_migrates_and_ends_disabled() {
    let mut conn = conn_at_50();
    seed_provider_v50(&conn, "p1", 1, None);

    migrations::run_up_to(&mut conn, 51).expect("apply 0051 must not refuse the C16 shape");

    let enabled: i64 = conn
        .query_row(
            "SELECT enabled FROM federation_provider WHERE id = 'p1'",
            [],
            |r| r.get(0),
        )
        .expect("row survives");
    assert_eq!(enabled, 0, "must end disabled");
}
