#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_arguments,
    clippy::panic,
    clippy::type_complexity
)]
//! Tests for RFC 096-B2 stage 1 — migration 0050, `federation_provider`'s
//! validation columns and the upgrade rule RFC 096 `:1018-1036`.
//!
//! Seeded through raw `rusqlite`, at a real pre-0050 schema version, the
//! same way `tests_rfc096_b1_stage11b.rs` exercises migration 0048's own
//! upgrade-time data transformation.

use crate::crypto::{self, MasterKey};
use crate::migrations;
use crate::repos::federation_login_attempt;
use crate::{Database, StoreError};
use rusqlite::{Connection, OptionalExtension};

fn conn_at(version: i32) -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, version).expect("run migrations");
    conn
}

fn conn_at_49() -> Connection {
    conn_at(49)
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

#[allow(clippy::too_many_arguments)]
fn seed_provider(
    conn: &Connection,
    id: &str,
    slug: &str,
    issuer: &str,
    client_id: &str,
    client_secret_enc: Option<&[u8]>,
    provision_mode: &str,
    enabled: i64,
) {
    conn.execute(
        "INSERT INTO federation_provider( \
             id, slug, display_name, issuer, client_id, client_secret_enc, scopes, \
             provision_mode, enabled, allowed_origins, created_at, updated_at) \
         VALUES (?1, ?2, 'Provider', ?3, ?4, ?5, 'openid email', ?6, ?7, '', \
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        rusqlite::params![
            id,
            slug,
            issuer,
            client_id,
            client_secret_enc,
            provision_mode,
            enabled
        ],
    )
    .expect("provider");
}

fn hash32(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

#[allow(clippy::too_many_arguments)]
fn seed_attempt(
    conn: &Connection,
    id: &str,
    provider_id: &str,
    tag: u8,
    status: &str,
    claimed_at: Option<&str>,
) {
    conn.execute(
        "INSERT INTO federation_login_attempt( \
             id, provider_id, provider_config_version, provider_activation_generation, \
             state_sha256, nonce_sha256, browser_binding_sha256, pkce_verifier_sealed, \
             exact_redirect_uri, next_path, created_at, expires_at, status, claimed_at) \
         VALUES (?1, ?2, 0, 0, ?3, ?4, ?5, ?6, 'https://rp.example/cb', NULL, \
                  '2026-01-01T00:00:00Z', '2026-01-01T00:10:00Z', ?7, ?8)",
        rusqlite::params![
            id,
            provider_id,
            hash32(tag),
            hash32(tag.wrapping_add(100)),
            hash32(tag.wrapping_add(200)),
            b"sealed-pkce".as_slice(),
            status,
            claimed_at,
        ],
    )
    .expect("attempt");
}

const PENDING_EXPIRES_AT: &str = "2026-01-01T00:05:00Z";

fn seed_mfa_pending(
    conn: &Connection,
    id: &str,
    provider_id: &str,
    user_id: &str,
    tag: u8,
    status: &str,
    continuation_sealed: Option<&[u8]>,
) {
    conn.execute(
        "INSERT INTO federation_mfa_pending( \
             id, provider_id, provider_config_version, provider_activation_generation, user_id, \
             continuation_sealed, next_path, browser_binding_sha256, csrf_binding_sha256, \
             primary_method, failure_count, status, created_at, expires_at) \
         VALUES (?1, ?2, 0, 0, ?3, ?4, NULL, ?5, ?6, 'fed', 0, ?7, '2026-01-01T00:00:00Z', ?8)",
        rusqlite::params![
            id,
            provider_id,
            user_id,
            continuation_sealed,
            hash32(tag),
            hash32(tag.wrapping_add(100)),
            status,
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

fn row_federation_provider(conn: &Connection, id: &str) -> (i64, i64, i64, String, Option<i64>) {
    conn.query_row(
        "SELECT enabled, config_version, activation_generation, validation_status, \
         evidence_config_version FROM federation_provider WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )
    .expect("provider row")
}

fn attempt_status(conn: &Connection, id: &str) -> (String, Option<String>) {
    conn.query_row(
        "SELECT status, claimed_at FROM federation_login_attempt WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .expect("attempt row")
}

fn mfa_pending_status(conn: &Connection, id: &str) -> (String, Option<Vec<u8>>) {
    conn.query_row(
        "SELECT status, continuation_sealed FROM federation_mfa_pending WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .expect("mfa_pending row")
}

fn row_exists(conn: &Connection, table: &str, id: &str) -> bool {
    conn.query_row(
        &format!("SELECT 1 FROM {table} WHERE id = ?1"),
        [id],
        |_| Ok(()),
    )
    .optional()
    .expect("query")
    .is_some()
}

// ── Fixture: zero providers ─────────────────────────────────────────────

#[test]
fn zero_providers_migrates_cleanly() {
    let mut conn = conn_at_49();
    migrations::run_up_to(&mut conn, 50).expect("apply 0050 against an empty database");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM federation_provider", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 0);
}

// ── Every provider ends disabled/requires_review/counters at 0 ─────────

/// Fixture: an **enabled** legacy provider and a **disabled** one, one of
/// each `provision_mode`.
#[test]
fn every_existing_provider_ends_disabled_review_required_with_counters_at_zero() {
    let mut conn = conn_at_49();
    seed_provider(
        &conn,
        "p1",
        "enabled-link-only",
        "https://idp1.example",
        "client-1",
        None,
        "link_only",
        1,
    );
    seed_provider(
        &conn,
        "p2",
        "disabled-provision",
        "https://idp2.example",
        "client-2",
        None,
        "provision_on_first_login",
        0,
    );

    migrations::run_up_to(&mut conn, 50).expect("apply 0050");

    for id in ["p1", "p2"] {
        let (enabled, config_version, activation_generation, validation_status, evidence) =
            row_federation_provider(&conn, id);
        assert_eq!(enabled, 0, "{id}: must end disabled");
        assert_eq!(config_version, 0, "{id}: config_version must start at 0");
        assert_eq!(
            activation_generation, 0,
            "{id}: activation_generation must start at 0"
        );
        assert_eq!(
            validation_status, "requires_review",
            "{id}: must require review"
        );
        assert_eq!(evidence, None, "{id}: no evidence may be guessed");
    }
}

/// **Nothing is guessed**: every evidence column, on every provider,
/// stays exactly `NULL` — not merely `evidence_config_version` (checked
/// above), every one of the six.
#[test]
fn nothing_is_guessed_every_evidence_column_stays_null() {
    let mut conn = conn_at_49();
    seed_provider(
        &conn,
        "p1",
        "s1",
        "https://idp.example",
        "client",
        None,
        "link_only",
        1,
    );
    migrations::run_up_to(&mut conn, 50).expect("apply 0050");

    let row: (
        Option<i64>,
        Option<i64>,
        Option<Vec<u8>>,
        Option<String>,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
    ) = conn
        .query_row(
            "SELECT evidence_config_version, evidence_activation_generation, \
             evidence_policy_digest, evidence_observed_at, evidence_metadata_fingerprint, \
             evidence_jwks_fingerprint FROM federation_provider WHERE id = 'p1'",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .expect("evidence columns");
    assert_eq!(row, (None, None, None, None, None, None));
}

// ── Links, IDs and the sealed secret survive ────────────────────────────

/// Fixture: existing links and users, and a **sealed secret** — a real
/// `crypto::seal` envelope, round-tripped through `crypto::open` after
/// the migration to prove it is byte-for-byte intact, not merely present.
#[test]
fn links_ids_and_the_sealed_secret_survive() {
    let key = MasterKey::generate();
    let plaintext = b"super-secret-client-credential";
    let sealed =
        crypto::seal(&key, plaintext, crate::repos::federation_provider::AAD).expect("seal");

    let mut conn = conn_at_49();
    seed_user(&conn, "u1");
    seed_provider(
        &conn,
        "p1",
        "google",
        "https://idp.example",
        "client",
        Some(&sealed),
        "link_only",
        1,
    );
    seed_link(&conn, "u1", "p1");

    migrations::run_up_to(&mut conn, 50).expect("apply 0050");

    // The provider id is unchanged, and the link survives naming it.
    let still_linked: bool = conn
        .query_row(
            "SELECT 1 FROM federation_link WHERE user_id = 'u1' AND provider_id = 'p1'",
            [],
            |_| Ok(true),
        )
        .expect("link survives");
    assert!(still_linked);

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
    assert_eq!(
        opened, plaintext,
        "must decrypt to the exact original plaintext"
    );
}

// ── A malformed row still migrates ──────────────────────────────────────

/// Fixture: a **malformed** row — empty `issuer`/`client_id` and a
/// `client_secret_enc` that is not a valid sealed envelope at all
/// (simulating pre-existing corruption). Migration 0050 never reads or
/// validates either column, so this must migrate exactly like a
/// well-formed row: disabled, `requires_review`, counters at 0, and the
/// garbage bytes preserved untouched (not "fixed", not rejected).
#[test]
fn a_malformed_row_still_migrates_and_is_disabled_for_review() {
    let mut conn = conn_at_49();
    seed_provider(
        &conn,
        "p1",
        "malformed",
        "", // empty issuer
        "", // empty client_id
        Some(b"not-a-real-sealed-envelope"),
        "link_only",
        1,
    );

    migrations::run_up_to(&mut conn, 50).expect("apply 0050 must not refuse a malformed row");

    let (enabled, config_version, activation_generation, validation_status, _) =
        row_federation_provider(&conn, "p1");
    assert_eq!(enabled, 0);
    assert_eq!(config_version, 0);
    assert_eq!(activation_generation, 0);
    assert_eq!(validation_status, "requires_review");
    let garbage: Vec<u8> = conn
        .query_row(
            "SELECT client_secret_enc FROM federation_provider WHERE id = 'p1'",
            [],
            |r| r.get(0),
        )
        .expect("garbage bytes read back");
    assert_eq!(garbage, b"not-a-real-sealed-envelope");
}

// ── An attempt, an MFA continuation and a ceremony in flight ────────────

/// Fixture: an attempt, an MFA continuation and a ceremony **in flight**
/// (plus one terminal row per table, proven untouched). Each table's own
/// existing invalidation shape, generalised to every row — not a new
/// one.
#[test]
fn in_flight_attempt_mfa_pending_and_ceremony_are_all_invalidated_terminal_rows_untouched() {
    let mut conn = conn_at_49();
    seed_provider(
        &conn,
        "p1",
        "s1",
        "https://idp.example",
        "client",
        None,
        "link_only",
        1,
    );
    seed_user(&conn, "u1");
    seed_link(&conn, "u1", "p1");

    // Attempts: pending, exchanging (already claimed), and a terminal
    // completed one that must be left exactly as it is.
    seed_attempt(&conn, "a-pending", "p1", 1, "pending", None);
    seed_attempt(
        &conn,
        "a-exchanging",
        "p1",
        2,
        "exchanging",
        Some("2026-01-01T00:05:00Z"),
    );
    seed_attempt(
        &conn,
        "a-completed",
        "p1",
        3,
        "completed",
        Some("2026-01-01T00:05:00Z"),
    );

    // MFA pending: one in-flight (pending), with a ceremony bound to it,
    // and one terminal (completed) that must be left exactly as it is.
    seed_mfa_pending(
        &conn,
        "m-pending",
        "p1",
        "u1",
        10,
        "pending",
        Some(b"continuation"),
    );
    seed_ceremony(&conn, "c1", "m-pending", "p1", "u1");
    seed_mfa_pending(&conn, "m-completed", "p1", "u1", 11, "completed", None);

    migrations::run_up_to(&mut conn, 50).expect("apply 0050");

    // Attempts.
    let (status, claimed_at) = attempt_status(&conn, "a-pending");
    assert_eq!(status, "failed");
    assert!(
        claimed_at.is_some(),
        "claimed_at must be filled in on invalidation"
    );
    let (status, claimed_at) = attempt_status(&conn, "a-exchanging");
    assert_eq!(status, "failed");
    assert_eq!(
        claimed_at.as_deref(),
        Some("2026-01-01T00:05:00Z"),
        "an already-claimed row's real claim time must survive, not be overwritten"
    );
    let (status, claimed_at) = attempt_status(&conn, "a-completed");
    assert_eq!(
        status, "completed",
        "a terminal attempt must be left exactly as it is"
    );
    assert_eq!(claimed_at.as_deref(), Some("2026-01-01T00:05:00Z"));

    // MFA pending.
    let (status, continuation) = mfa_pending_status(&conn, "m-pending");
    assert_eq!(status, "invalidated");
    assert_eq!(
        continuation, None,
        "a terminal row must carry no usable continuation"
    );
    let (status, continuation) = mfa_pending_status(&conn, "m-completed");
    assert_eq!(
        status, "completed",
        "a terminal MFA-pending row must be left exactly as it is"
    );
    assert_eq!(continuation, None);

    // Ceremony: the in-flight one is gone.
    assert!(
        !row_exists(&conn, "federation_webauthn_ceremony", "c1"),
        "an in-flight ceremony must not survive the migration"
    );
}

/// **An in-flight attempt cannot complete after the migration** — a
/// behavioural proof, not only a row-state one: the real
/// [`federation_login_attempt::claim`] is called against the migrated
/// row and must refuse it, through the application's own guard, the
/// same way a genuinely superseded/replayed attempt already does.
#[tokio::test]
async fn an_in_flight_attempt_cannot_complete_after_the_migration() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("t.sqlite");
    // Real UUIDs, round-tripped through the typed ids below -- plain
    // strings like the other, raw-SQL-only tests in this file use would
    // not parse as `FederationLoginAttemptId`/`FederationProviderId`.
    let provider_id = uuid::Uuid::new_v4().to_string();
    let attempt_id = uuid::Uuid::new_v4().to_string();
    {
        let mut conn = Connection::open(&path).expect("open file-backed db");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("enable FK");
        migrations::run_up_to(&mut conn, 49).expect("run up to 49");
        seed_provider(
            &conn,
            &provider_id,
            "s1",
            "https://idp.example",
            "client",
            None,
            "link_only",
            1,
        );
        seed_attempt(&conn, &attempt_id, &provider_id, 1, "pending", None);
        migrations::run_up_to(&mut conn, 50).expect("apply 0050");
    }

    let db = Database::open(&path, MasterKey::generate()).expect("open as Database");
    let attempt_id: sui_id_shared::ids::FederationLoginAttemptId =
        attempt_id.parse().expect("parse attempt id");
    let provider_id: sui_id_shared::ids::FederationProviderId =
        provider_id.parse().expect("parse provider id");
    let result = federation_login_attempt::claim(
        &db,
        attempt_id,
        chrono::Utc::now(),
        provider_id,
        0,
        0,
        [0u8; 32],
    )
    .await;
    assert!(
        matches!(result, Err(StoreError::Conflict)),
        "claiming a pre-upgrade attempt must be refused, not completed: {result:?}"
    );
}

// ── Interrupted and repeated migration ──────────────────────────────────

/// An **interrupted** migration: 0050 runs inside the same `BEGIN
/// IMMEDIATE` transaction every other migration does (`apply_migration`),
/// so a real interruption (crash before `COMMIT`) must leave the
/// database at 49, not partially at 50 — proven here by hand-rolling the
/// same transaction, forcing a failure before commit (a duplicate
/// primary key — a real, guaranteed rusqlite error, unlike e.g. `1/0`,
/// which SQLite evaluates to `NULL`, not an error), and confirming
/// nothing committed.
///
/// Then **repeated**: `migrations::run_up_to` is the only realistic way
/// this project ever re-attempts a migration (version-gated: a stored
/// `schema_version` of 50 would make it a no-op, so "repeated" only
/// means what a real crash-and-restart gives — one more attempt against
/// the still-unmigrated, rolled-back state proven above). That real
/// retry must succeed and reach exactly the same correct end state a
/// clean first attempt would.
#[test]
fn an_interrupted_migration_can_be_safely_retried() {
    let mut conn = conn_at_49();
    seed_provider(
        &conn,
        "p1",
        "s1",
        "https://idp.example",
        "client",
        None,
        "link_only",
        1,
    );
    seed_attempt(&conn, "a-pending", "p1", 1, "pending", None);

    // The interruption: the real migration body, hand-rolled under an
    // explicit transaction so a forced failure can be injected before
    // `COMMIT` — standing in for a crash at that exact point.
    {
        let sql = migrations::sql_for_version(50);
        let tx = conn.unchecked_transaction().expect("tx");
        tx.execute_batch(sql).expect("the real migration body");
        let forced_failure = tx.execute("INSERT INTO federation_provider (id) VALUES ('p1')", []);
        assert!(forced_failure.is_err(), "sanity: this must fail");
        drop(tx); // rusqlite rolls back an uncommitted transaction on drop
    }

    // Nothing from the migration body committed: the new columns do not
    // exist, the same as a connection still at 49.
    let still_old_shape = conn.query_row(
        "SELECT config_version FROM federation_provider WHERE id = 'p1'",
        [],
        |r| r.get::<_, i64>(0),
    );
    assert!(
        still_old_shape.is_err(),
        "a rolled-back migration must leave the pre-migration schema, not a partial one"
    );

    // The repeat: the real, version-gated path, which can genuinely
    // apply now (the rollback above left `schema_version` at 49).
    migrations::run_up_to(&mut conn, 50).expect("the retried migration must succeed");

    let (enabled, config_version, activation_generation, validation_status, evidence) =
        row_federation_provider(&conn, "p1");
    assert_eq!(enabled, 0);
    assert_eq!(config_version, 0);
    assert_eq!(activation_generation, 0);
    assert_eq!(validation_status, "requires_review");
    assert_eq!(evidence, None);
    let (status, claimed_at) = attempt_status(&conn, "a-pending");
    assert_eq!(status, "failed");
    assert!(claimed_at.is_some());
}

/// The same seed, migrated on two independent, freshly-built
/// connections: identical semantic outcomes on both, including the
/// `COALESCE`d `claimed_at` for an already-claimed attempt — no run
/// depends on iteration order, prior state, or anything but the fixture
/// itself.
#[test]
fn the_migration_is_deterministic_across_independent_runs() {
    fn migrate_and_read(
        claimed_at_seed: Option<&str>,
    ) -> ((i64, i64, i64, String, Option<i64>), String, Option<String>) {
        let mut conn = conn_at_49();
        seed_provider(
            &conn,
            "p1",
            "s1",
            "https://idp.example",
            "client",
            None,
            "link_only",
            1,
        );
        seed_attempt(
            &conn,
            "a-exchanging",
            "p1",
            1,
            "exchanging",
            claimed_at_seed,
        );
        migrations::run_up_to(&mut conn, 50).expect("apply 0050");
        let provider = row_federation_provider(&conn, "p1");
        let (status, claimed_at) = attempt_status(&conn, "a-exchanging");
        (provider, status, claimed_at)
    }

    let first = migrate_and_read(Some("2026-01-01T00:05:00Z"));
    let second = migrate_and_read(Some("2026-01-01T00:05:00Z"));
    assert_eq!(
        first, second,
        "identical fixtures must migrate to identical results"
    );
    assert_eq!(
        first.2.as_deref(),
        Some("2026-01-01T00:05:00Z"),
        "COALESCE must preserve the real claim time, not stamp a fresh one"
    );
}

// ── The fresh-install schema equals the upgraded schema ─────────────────

#[test]
fn the_applied_schema_is_identical_fresh_vs_forward_migrated() {
    fn dump(conn: &Connection) -> String {
        conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'federation_provider'",
            [],
            |r| r.get(0),
        )
        .expect("table present")
    }

    let fresh = conn_at(50);
    let mut forward = conn_at(49);
    migrations::run_up_to(&mut forward, 50).expect("apply 0050 forward");

    assert_eq!(
        dump(&fresh),
        dump(&forward),
        "a fresh database at 50 and a database forward-migrated from 49 to 50 \
         must produce byte-identical federation_provider schema text"
    );
}

// ── The gap this cycle exists for ───────────────────────────────────────

/// **The one test that would have caught the gap this cycle exists
/// for**: a provider that was `enabled = 1` before the upgrade is not
/// offered on the login screen after it.
#[tokio::test]
async fn a_previously_enabled_provider_is_not_offered_on_the_login_screen_after_upgrade() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("t.sqlite");
    // A real UUID: `federation_provider::list_enabled`'s row mapper
    // parses `id` as a `FederationProviderId`, which a plain string like
    // the other, raw-SQL-only tests in this file use would fail.
    let provider_id = uuid::Uuid::new_v4().to_string();
    {
        let mut conn = Connection::open(&path).expect("open file-backed db");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("enable FK");
        migrations::run_up_to(&mut conn, 49).expect("run up to 49");
        seed_provider(
            &conn,
            &provider_id,
            "was-enabled",
            "https://idp.example",
            "client",
            None,
            "link_only",
            1, // enabled, pre-upgrade
        );
        migrations::run_up_to(&mut conn, 50).expect("apply 0050");
    }

    let db = Database::open(&path, MasterKey::generate()).expect("open as Database");
    let enabled = crate::repos::federation_provider::list_enabled(&db)
        .await
        .expect("list_enabled");
    assert!(
        enabled.is_empty(),
        "a provider enabled before the upgrade must not be offered on the login \
         screen after it: {enabled:?}"
    );
}
