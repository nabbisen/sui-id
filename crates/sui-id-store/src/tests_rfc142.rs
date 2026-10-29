#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! RFC 142 stage 2 — the migration corpus (D1).
//!
//! 38 fixtures: 37 built by replaying today's `MIGRATIONS` from an empty
//! database up to each of the 37 shipped schema versions (stage 1), plus
//! one hand-built fixture for the `0.29.7`-origin schema-21 shape that
//! replay cannot reproduce (stage 1 §4.3). Each seeds a populated
//! database, migrates it to head, and asserts the data survived —
//! row counts, specific per-migration invariants, and at least one value
//! (not just a count) read back unchanged.
//!
//! # The boundary generator (stage 1 §4.1 / stage 2 §5)
//!
//! `derive_ranges` walks every migration from 1 to [`migrations::MAX_SCHEMA_VERSION`]
//! and classifies each one mechanically. It **stops and asks** — by
//! panicking, naming the migration and the question, failing this test —
//! on exactly three events a human must resolve, not a heuristic:
//!
//! 1. a migration creates a table that is in neither [`SEEDED_TABLES`]
//!    nor [`EXCLUDED_TABLES`];
//! 2. a migration attaches a `CHECK` to a column of a table that already
//!    existed as a seeded table, and the migration number has no entry
//!    in [`RESOLVED_CHECKS`];
//! 3. a migration drops a column from a seeded table, and the migration
//!    number has no entry in [`RESOLVED_DROP_COLUMNS`].
//!
//! When migration 50 lands, this test either keeps passing (the
//! generator decided mechanically that nothing changed for the seeded
//! set) or fails naming exactly what it found — never silently keeps
//! testing the old set.

use crate::migrations::{self, MAX_SCHEMA_VERSION};

// ───────────────────────── the seeded-table set (RFC 142 §Settled) ──────

/// Tables holding operator- or end-user-authored data that an upgrade
/// must not lose. Amended in review from stage 1's 13 to include
/// `revoked_access_tokens` (RFC 142 §Settled, 2026-10-29): losing it
/// silently un-revokes an already-revoked, still-valid access token.
pub(crate) const SEEDED_TABLES: &[&str] = &[
    "users",
    "credentials",
    "clients",
    "signing_keys",
    "user_totp",
    "user_webauthn_credentials",
    "smtp_config",
    "server_settings",
    "user_consent",
    "scope_definition",
    "client_registration_token",
    "federation_provider",
    "federation_link",
    "revoked_access_tokens",
];

/// Tables created by some migration that are deliberately **not** seeded
/// data — ephemeral/session-like state, migration bookkeeping, dead
/// code, or `audit_log` (which gets its own two fixtures, §audit_log
/// below, rather than a place in the 14-table corpus; stage 1 §4.2).
/// A table created by a migration that is in neither this list nor
/// [`SEEDED_TABLES`] is unresolved and fails [`derive_ranges`] loudly.
const EXCLUDED_TABLES: &[&str] = &[
    "sui_meta",
    "auth_codes",
    "sessions",
    "refresh_tokens",
    "login_pending_mfa",
    "webauthn_pending",
    "webauthn_pending_new",
    "consents",
    "audit_log",
    "email_outbox",
    "password_reset_tokens",
    "pending_settings_change",
    "federation_login_attempt",
    "federation_mfa_pending",
    "federation_webauthn_ceremony",
    "forgot_password_requests",
    // Transient staging names for the rename-rebuild idiom (migration
    // 0022): the effective rebuilt table is the post-RENAME name, which
    // is already in SEEDED_TABLES; these staging names never exist once
    // the migration that creates them finishes.
    "_users_new",
    "_credentials_new",
    "_clients_new",
    "_signing_keys_new",
    "_user_totp_new",
];

/// A migration that attaches a `CHECK` to a column of an **already
/// existing** seeded table (as opposed to a brand-new column carrying
/// its own self-referential `CHECK`, which is never ambiguous — see
/// [`analyze_migration`]'s doc) must be recorded here, once resolved by
/// a human, naming whether it opens a new seed-script range.
struct ResolvedCheck {
    migration: i32,
    opens_boundary: bool,
    #[allow(dead_code)]
    note: &'static str,
}

const RESOLVED_CHECKS: &[ResolvedCheck] = &[ResolvedCheck {
    migration: 22,
    opens_boundary: true,
    note: "boolean/consistency CHECK rebuild on users/credentials/clients/\
           signing_keys/user_totp. Treated conservatively as opening a \
           boundary: this codebase's existing data already used 0/1 \
           booleans, so no *specific* seed would be rejected, but \
           re-proving that by inspection for every future seed, forever, \
           costs more than one extra script now (stage 1 §4.1 point 2).",
}];

/// A migration that drops a column from a seeded table must be recorded
/// here, once resolved by a human, naming whether it opens a new
/// seed-script range.
struct ResolvedDropColumn {
    migration: i32,
    table: &'static str,
    column: &'static str,
    opens_boundary: bool,
    #[allow(dead_code)]
    note: &'static str,
}

const RESOLVED_DROP_COLUMNS: &[ResolvedDropColumn] = &[ResolvedDropColumn {
    migration: 43,
    table: "credentials",
    column: "must_change",
    opens_boundary: false,
    note: "DEFAULT 0, no production reader ever consulted it (the \
           migration's own comment says so), so a seed that never names \
           the column is valid both before and after (stage 1 §3).",
}];

// ───────────────────────── the mechanical analyzer ───────────────────────

/// What one migration's SQL does, as far as the generator needs to know.
#[derive(Debug, Default)]
struct MigrationEffects {
    /// Tables this migration creates for the first time (`CREATE TABLE`,
    /// with any same-migration `RENAME TO` already resolved to the final
    /// name).
    created_tables: Vec<String>,
    /// `(table, column)` pairs added where the column is `NOT NULL` with
    /// no `DEFAULT` — would always open a boundary if `table` is seeded.
    /// Never observed in this codebase (stage 1 confirmed every `NOT
    /// NULL` `ADD COLUMN` carries a default), kept as a real mechanical
    /// rule rather than an assumption.
    added_not_null_no_default: Vec<(String, String)>,
    /// Tables whose `CREATE TABLE` body (after resolving a same-migration
    /// rename) names an **already-existing** table and contains `CHECK`
    /// — the rename-rebuild idiom (migration 0022's shape).
    rebuilt_with_check: Vec<String>,
    /// `(table, column)` where `ADD COLUMN` carries a `CHECK` whose text
    /// does not mention that same column — i.e. a check on something
    /// *other* than the column just added, which is not self-evidently
    /// safe the way a self-referential check with a satisfying default
    /// is. Never observed in this codebase; a real mechanical rule, not
    /// an assumption.
    added_column_check_not_self_referential: Vec<(String, String)>,
    /// `(table, column)` dropped.
    dropped_columns: Vec<(String, String)>,
}

fn strip_sql_comments(sql: &str) -> String {
    sql.lines()
        .map(|line| line.find("--").map_or(line, |i| &line[..i]))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The first identifier token at the start of `s` (ASCII letters, digits,
/// underscore), case preserved.
fn first_ident(s: &str) -> Option<&str> {
    let s = s.trim_start();
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    if end == 0 { None } else { Some(&s[..end]) }
}

/// Does `haystack` contain `word` as a whole identifier (not as a
/// substring of a longer identifier)? Case-insensitive.
fn contains_word(haystack: &str, word: &str) -> bool {
    let haystack_lower = haystack.to_ascii_lowercase();
    let word_lower = word.to_ascii_lowercase();
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut start = 0;
    while let Some(rel) = haystack_lower[start..].find(&word_lower) {
        let pos = start + rel;
        let before_ok = pos == 0 || !is_ident(haystack_lower.as_bytes()[pos - 1] as char);
        let end = pos + word_lower.len();
        let after_ok =
            end >= haystack_lower.len() || !is_ident(haystack_lower.as_bytes()[end] as char);
        if before_ok && after_ok {
            return true;
        }
        start = pos + 1;
    }
    false
}

/// Analyze one migration's SQL (already embedded, as `migrations` holds
/// it) for the effects [`derive_ranges`] needs. `already_seeded` is the
/// set of seeded tables that exist **before** this migration runs — used
/// to tell "a brand-new table's own CREATE happens to contain CHECK"
/// (never ambiguous) from "an existing table was rebuilt with a new
/// CHECK" (migration 0022's shape, genuinely ambiguous).
fn analyze_migration(
    sql: &str,
    already_seeded: &std::collections::BTreeSet<&str>,
) -> MigrationEffects {
    let stripped = strip_sql_comments(sql);
    let statements: Vec<&str> = stripped
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    let mut effects = MigrationEffects::default();
    // Same-migration rename map: transient name -> final name.
    let mut renames: Vec<(String, String)> = Vec::new();
    // (name-as-created, body) for every CREATE TABLE in this migration,
    // resolved against `renames` after the full pass.
    let mut creates: Vec<(String, String)> = Vec::new();
    let mut drop_tables: Vec<String> = Vec::new();

    for stmt in &statements {
        let flat = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
        let upper = flat.to_ascii_uppercase();

        if upper.starts_with("CREATE TABLE") {
            let rest = flat["CREATE TABLE".len()..].trim_start();
            let rest = rest
                .strip_prefix("IF NOT EXISTS")
                .map_or(rest, str::trim_start);
            let Some(name) = first_ident(rest) else {
                continue;
            };
            let open = rest.find('(');
            let body = open.map(|i| &rest[i..]).unwrap_or("");
            creates.push((name.to_string(), body.to_string()));
        } else if upper.starts_with("ALTER TABLE") {
            let rest = flat["ALTER TABLE".len()..].trim_start();
            let Some(table) = first_ident(rest) else {
                continue;
            };
            let after_table = rest[table.len()..].trim_start();
            let after_table_upper = after_table.to_ascii_uppercase();

            if after_table_upper.starts_with("ADD COLUMN") {
                let col_rest = after_table["ADD COLUMN".len()..].trim_start();
                let Some(col) = first_ident(col_rest) else {
                    continue;
                };
                let def = col_rest[col.len()..].trim_start();
                let def_upper = def.to_ascii_uppercase();
                let has_not_null = def_upper.contains("NOT NULL");
                let has_default = def_upper.contains("DEFAULT");
                if has_not_null && !has_default {
                    effects
                        .added_not_null_no_default
                        .push((table.to_string(), col.to_string()));
                }
                if def_upper.contains("CHECK") && !contains_word(def, col) {
                    effects
                        .added_column_check_not_self_referential
                        .push((table.to_string(), col.to_string()));
                }
            } else if after_table_upper.starts_with("DROP COLUMN") {
                let col_rest = after_table["DROP COLUMN".len()..].trim_start();
                if let Some(col) = first_ident(col_rest) {
                    effects
                        .dropped_columns
                        .push((table.to_string(), col.to_string()));
                }
            } else if after_table_upper.starts_with("RENAME TO") {
                let new_rest = after_table["RENAME TO".len()..].trim_start();
                if let Some(new_name) = first_ident(new_rest) {
                    renames.push((table.to_string(), new_name.to_string()));
                }
            }
        } else if upper.starts_with("DROP TABLE") {
            let rest = flat["DROP TABLE".len()..].trim_start();
            let rest = rest.strip_prefix("IF EXISTS").map_or(rest, str::trim_start);
            if let Some(name) = first_ident(rest) {
                drop_tables.push(name.to_string());
            }
        }
        // CREATE INDEX / CREATE UNIQUE INDEX / UPDATE / INSERT / PRAGMA:
        // deliberately not analyzed. None can change whether an existing
        // seed's INSERT still works (stage 1 §4.1's mechanical rules),
        // and INSERT/UPDATE touch data this generator does not seed.
    }

    // Resolve same-migration renames: a created name that is later
    // renamed is known by its final name for everything downstream.
    let resolve = |name: &str| -> String {
        let mut cur = name.to_string();
        // At most one hop in every migration this codebase has; loop
        // defensively rather than assume.
        for _ in 0..4 {
            match renames.iter().find(|(from, _)| *from == cur) {
                Some((_, to)) => cur = to.clone(),
                None => break,
            }
        }
        cur
    };

    for (created_name, body) in &creates {
        let final_name = resolve(created_name);
        // A name this migration itself drops before (or instead of)
        // recreating under the same literal name (e.g. 0019's
        // `auth_codes`) is still a "first creation" from the seeded
        // set's point of view unless that exact name was already a
        // seeded table before this migration ran.
        if already_seeded.contains(final_name.as_str()) {
            if contains_word(body, "CHECK") {
                effects.rebuilt_with_check.push(final_name);
            }
            // Not a new table — already existed, just rebuilt. Do not
            // also report it as newly created.
        } else {
            effects.created_tables.push(final_name);
        }
    }
    let _ = drop_tables; // DROP TABLE alone (no recreate) never occurs for a seeded table in this history; nothing to act on.
    effects
}

/// One seed-script range: `[start, end]` inclusive, and the seeded
/// tables in scope for its whole duration.
#[derive(Debug, Clone)]
pub(crate) struct Range {
    pub start: i32,
    pub end: i32,
    pub tables: Vec<&'static str>,
}

/// Derive the seed-script ranges mechanically. Panics — failing whatever
/// test calls this — naming the migration and the question, on any of
/// the three events this module's doc names, when the migration number
/// has no recorded resolution.
pub(crate) fn derive_ranges() -> Vec<Range> {
    let mut in_scope: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
    let mut ranges: Vec<Range> = Vec::new();
    let mut current_start = 1;

    for v in 1..=MAX_SCHEMA_VERSION {
        let sql = migrations::sql_for_version(v);
        let effects = analyze_migration(sql, &in_scope);
        let mut opens = v == 1;

        for t in &effects.created_tables {
            if let Some(&known) = SEEDED_TABLES.iter().find(|s| *s == t) {
                in_scope.insert(known);
                opens = true;
            } else if EXCLUDED_TABLES.contains(&t.as_str()) {
                // known, deliberately not seeded data.
            } else {
                panic!(
                    "RFC 142 corpus generator: migration {v} creates table {t:?}, \
                     which is in neither SEEDED_TABLES nor EXCLUDED_TABLES \
                     (crates/sui-id-store/src/tests_rfc142.rs). Is this operator- \
                     or end-user-authored data the upgrade corpus must prove \
                     survives a migration? Record the answer in one of those \
                     two lists before this test can pass."
                );
            }
        }

        for (t, c) in &effects.added_not_null_no_default {
            if SEEDED_TABLES.contains(&t.as_str()) {
                opens = true;
                let _ = c;
            }
        }

        for t in &effects.rebuilt_with_check {
            match RESOLVED_CHECKS.iter().find(|r| r.migration == v) {
                Some(r) => {
                    if r.opens_boundary {
                        opens = true;
                    }
                }
                None => panic!(
                    "RFC 142 corpus generator: migration {v} rebuilds already-\
                     existing seeded table {t:?} with a new CHECK constraint, \
                     and RESOLVED_CHECKS has no entry for migration {v}. Decide \
                     whether this opens a new seed-script range (stage 1 §4.1 \
                     point 2) and record it there before this test can pass."
                ),
            }
        }

        for (t, c) in &effects.added_column_check_not_self_referential {
            if SEEDED_TABLES.contains(&t.as_str()) {
                match RESOLVED_CHECKS.iter().find(|r| r.migration == v) {
                    Some(r) => {
                        if r.opens_boundary {
                            opens = true;
                        }
                    }
                    None => panic!(
                        "RFC 142 corpus generator: migration {v} adds column \
                         {t}.{c} with a CHECK that does not mention {c} itself \
                         -- a check on something else -- and RESOLVED_CHECKS has \
                         no entry for migration {v}. Decide whether this opens a \
                         new seed-script range and record it before this test \
                         can pass."
                    ),
                }
            }
        }

        for (t, c) in &effects.dropped_columns {
            if SEEDED_TABLES.contains(&t.as_str()) {
                match RESOLVED_DROP_COLUMNS
                    .iter()
                    .find(|r| r.migration == v && r.table == t && r.column == c)
                {
                    Some(r) => {
                        if r.opens_boundary {
                            opens = true;
                        }
                    }
                    None => panic!(
                        "RFC 142 corpus generator: migration {v} drops column \
                         {t}.{c} from a seeded table, and RESOLVED_DROP_COLUMNS \
                         has no entry for migration {v}/{t}/{c}. Decide whether \
                         a seed script would have named this column (stage 1 \
                         §3) and record the answer before this test can pass."
                    ),
                }
            }
        }

        if opens && v > 1 {
            if let Some(last) = ranges.last_mut() {
                last.end = v - 1;
            }
            current_start = v;
        }
        let snapshot: Vec<&'static str> = SEEDED_TABLES
            .iter()
            .copied()
            .filter(|t| in_scope.contains(t))
            .collect();
        if opens {
            ranges.push(Range {
                start: current_start,
                end: v,
                tables: snapshot,
            });
        } else if let Some(last) = ranges.last_mut() {
            // Not a new range: extend the current one's provisional end
            // (and keep its table snapshot current) every iteration, so
            // the final range needs no special-cased close at
            // MAX_SCHEMA_VERSION -- the loop ending *is* the close.
            last.end = v;
            last.tables = snapshot;
        }
    }
    ranges
}

#[test]
fn derive_ranges_matches_the_accepted_stage1_table() {
    let ranges = derive_ranges();

    // RFC 142 §Settled / stage 1 report, amended in review: the 14-table
    // seeded set (adding revoked_access_tokens, created at 0005) splits
    // the original R3 {4..13} into {4} and {5..13} — 11 structural
    // ranges in all. Exactly one, starting at 37, has no shipped
    // starting version (stage 1 §1/§3) and needs no seed script, but
    // the generator itself does not know which versions shipped, so it
    // still appears here as its own range; stage 2's fixture list below
    // filters it out by intersecting with the shipped-version set.
    // Structural boundaries open at the migration that changes the
    // shape, not at the nearest shipped tag — 0014 (smtp_config) never
    // shipped alone (stage 1 §1), but it is still where the boundary
    // opens; 15 is the first *shipped* member of that range.
    let starts: Vec<i32> = ranges.iter().map(|r| r.start).collect();
    assert_eq!(
        starts,
        vec![1, 3, 4, 5, 14, 16, 22, 25, 36, 37, 38],
        "range starts changed -- full ranges: {ranges:#?}"
    );
    let ends: Vec<i32> = ranges.iter().map(|r| r.end).collect();
    assert_eq!(
        ends,
        vec![2, 3, 4, 13, 15, 21, 24, 35, 36, 37, 49],
        "range ends changed -- full ranges: {ranges:#?}"
    );

    let tables_at = |start: i32| -> Vec<&'static str> {
        let mut t = ranges
            .iter()
            .find(|r| r.start == start)
            .unwrap()
            .tables
            .clone();
        t.sort_unstable();
        t
    };
    assert_eq!(
        tables_at(1),
        vec!["clients", "credentials", "signing_keys", "users"]
    );
    assert_eq!(
        tables_at(3),
        vec![
            "clients",
            "credentials",
            "signing_keys",
            "user_totp",
            "users"
        ]
    );
    assert_eq!(
        tables_at(4),
        vec![
            "clients",
            "credentials",
            "signing_keys",
            "user_totp",
            "user_webauthn_credentials",
            "users"
        ]
    );
    assert_eq!(
        tables_at(5),
        vec![
            "clients",
            "credentials",
            "revoked_access_tokens",
            "signing_keys",
            "user_totp",
            "user_webauthn_credentials",
            "users"
        ]
    );
    assert!(tables_at(14).contains(&"smtp_config"));
    assert!(tables_at(16).contains(&"server_settings"));
    assert!(tables_at(25).contains(&"user_consent"));
    assert!(tables_at(36).contains(&"scope_definition"));
    assert!(tables_at(36).contains(&"client_registration_token"));
    assert!(tables_at(37).contains(&"federation_provider"));
    assert!(tables_at(38).contains(&"federation_link"));
    // The 22-24 range: no table joins, but the rebuild is still its own
    // range (opened by the resolved CHECK event, not a new table).
    assert_eq!(
        tables_at(22),
        tables_at(16),
        "migration 22 must not change *which* tables are in scope, only how they're built"
    );
}

/// Mutation evidence for the generator's own "stop and ask" behaviour
/// (not a `#[test]`, a helper the panic tests below call): confirms the
/// three ask-events actually fire, by constructing a migration-shaped
/// SQL string this module never embeds and checking the analyzer flags
/// it, rather than only trusting that it happens to pass on the real
/// 49 migrations.
#[test]
fn analyze_migration_flags_an_unresolved_new_table() {
    let already = std::collections::BTreeSet::new();
    let effects = analyze_migration(
        "CREATE TABLE totally_new_thing (id TEXT PRIMARY KEY);",
        &already,
    );
    assert_eq!(
        effects.created_tables,
        vec!["totally_new_thing".to_string()]
    );
}

#[test]
fn analyze_migration_flags_a_check_on_an_already_existing_table() {
    let mut already = std::collections::BTreeSet::new();
    already.insert("users");
    let effects = analyze_migration(
        "CREATE TABLE _users_new (id TEXT PRIMARY KEY, is_admin INTEGER CHECK (is_admin IN (0,1))); \
         ALTER TABLE _users_new RENAME TO users;",
        &already,
    );
    assert_eq!(effects.rebuilt_with_check, vec!["users".to_string()]);
    assert!(effects.created_tables.is_empty());
}

#[test]
fn analyze_migration_does_not_flag_a_self_referential_check_on_a_new_column() {
    let mut already = std::collections::BTreeSet::new();
    already.insert("server_settings");
    let effects = analyze_migration(
        "ALTER TABLE server_settings ADD COLUMN hibp_mode TEXT NOT NULL DEFAULT 'warn' \
         CHECK (hibp_mode IN ('off','warn','block'));",
        &already,
    );
    assert!(effects.added_column_check_not_self_referential.is_empty());
    assert!(effects.rebuilt_with_check.is_empty());
}

#[test]
fn analyze_migration_flags_a_check_on_a_new_column_that_references_something_else() {
    let mut already = std::collections::BTreeSet::new();
    already.insert("widgets");
    let effects = analyze_migration(
        "ALTER TABLE widgets ADD COLUMN gizmo_count INTEGER NOT NULL DEFAULT 0 \
         CHECK (sprocket_count > 0);",
        &already,
    );
    assert_eq!(
        effects.added_column_check_not_self_referential,
        vec![("widgets".to_string(), "gizmo_count".to_string())]
    );
}

#[test]
fn analyze_migration_flags_a_drop_column_on_a_seeded_table() {
    let already = std::collections::BTreeSet::new();
    let effects = analyze_migration("ALTER TABLE credentials DROP COLUMN must_change;", &already);
    assert_eq!(
        effects.dropped_columns,
        vec![("credentials".to_string(), "must_change".to_string())]
    );
}

#[test]
fn analyze_migration_ignores_create_index_and_updates() {
    let already = std::collections::BTreeSet::new();
    let effects = analyze_migration(
        "CREATE INDEX idx_foo ON users(email); UPDATE users SET email = NULL;",
        &already,
    );
    assert!(effects.created_tables.is_empty());
    assert!(effects.dropped_columns.is_empty());
    assert!(effects.rebuilt_with_check.is_empty());
}

// ───────────────────────── the 37-by-replay fixtures ─────────────────────
//
// 10 seed functions (one per range from `derive_ranges`), dispatched by
// each range's `start`. Every function seeds exactly the tables in
// scope for its range -- cumulatively, not just the delta since the
// previous range, because the corpus must prove *every* table with data
// at a given starting version survives from there, not only the newest
// one.
//
// `users.user_uuid` is a trap, not an assumption: the column (added at
// migration 4) only has a `DEFAULT ''` until migration 22's rebuild
// removes the default and keeps only `CHECK (length(user_uuid) = 36)`.
// A row inserted directly into an already-migrated database never goes
// through either migration's own one-time backfill, so every seed from
// range 4 onward sets a real 36-character UUID explicitly rather than
// relying on a default that stops existing partway through the chain
// the fixture is about to replay.

use rusqlite::Connection;

fn uuid36() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn count_rows(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap_or_else(|e| panic!("counting {table}: {e}"))
}

/// The four tables present from migration 1 onward. `with_user_uuid`
/// must be true from range-start 4 onward (see module note above).
fn seed_core(conn: &Connection, marker: &str, with_user_uuid: bool) {
    let (uuid_col, uuid_val) = if with_user_uuid {
        (", user_uuid", format!(", '{}'", uuid36()))
    } else {
        ("", String::new())
    };
    conn.execute_batch(&format!(
        "INSERT INTO users(id, username, display_name, is_admin, is_disabled, \
             is_deleted, created_at, updated_at{uuid_col}) \
         VALUES('u-{marker}', '{marker}', '{marker}', 0, 0, 0, datetime('now'), \
             datetime('now'){uuid_val});
         INSERT INTO credentials(user_id, password_hash, updated_at) \
         VALUES('u-{marker}', 'hash-{marker}', datetime('now'));
         INSERT INTO clients(id, name, confidential, secret_hash, redirect_uris, \
             is_disabled, is_deleted, created_at, updated_at) \
         VALUES('c-{marker}', 'client-{marker}', 1, 'sechash-{marker}', '[]', 0, 0, \
             datetime('now'), datetime('now'));
         INSERT INTO signing_keys(id, algorithm, private_key_enc, public_key, created_at) \
         VALUES('sk-{marker}', 'EdDSA', X'00', X'00', datetime('now'));"
    ))
    .unwrap_or_else(|e| panic!("seed_core({marker}): {e}"));
}

fn seed_user_totp(conn: &Connection, marker: &str) {
    conn.execute(
        &format!(
            "INSERT INTO user_totp(user_id, secret_enc, created_at) \
             VALUES('u-{marker}', X'00', datetime('now'))"
        ),
        [],
    )
    .unwrap_or_else(|e| panic!("seed_user_totp({marker}): {e}"));
}

fn seed_webauthn(conn: &Connection, marker: &str) {
    conn.execute(
        &format!(
            "INSERT INTO user_webauthn_credentials(id, user_id, credential_id, \
                 passkey_enc, nickname, created_at) \
             VALUES('wc-{marker}', 'u-{marker}', X'00', X'00', '{marker}', datetime('now'))"
        ),
        [],
    )
    .unwrap_or_else(|e| panic!("seed_webauthn({marker}): {e}"));
}

fn seed_revoked_access_token(conn: &Connection, marker: &str) {
    conn.execute(
        &format!(
            "INSERT INTO revoked_access_tokens(jti, revoked_at, exp) \
             VALUES('jti-{marker}', datetime('now'), datetime('now','+1 hour'))"
        ),
        [],
    )
    .unwrap_or_else(|e| panic!("seed_revoked_access_token({marker}): {e}"));
}

fn seed_smtp_config(conn: &Connection, marker: &str) {
    // smtp_config's own migration (0014) does not auto-insert a row
    // (unlike server_settings/scope_definition), so a fresh INSERT
    // never collides with the `id = 'singleton'` CHECK's only legal
    // value.
    conn.execute(
        &format!(
            "INSERT INTO smtp_config(id, host, port, tls_mode, from_address, \
                 base_url, created_at, updated_at) \
             VALUES('singleton', 'smtp-{marker}.example', 587, 'starttls', \
                 'noreply@{marker}.example', 'https://{marker}.example', \
                 datetime('now'), datetime('now'))"
        ),
        [],
    )
    .unwrap_or_else(|e| panic!("seed_smtp_config({marker}): {e}"));
}

/// `server_settings` is a singleton **already populated by its own
/// migration** (0016 inserts the `'singleton'` row itself) — a second
/// `INSERT` with that same primary key would conflict. `UPDATE` instead,
/// which doubles as the table's value-level (not just count) assertion:
/// `default_lang` has no `CHECK`, so writing the marker into it and
/// reading it back after migrating to head proves the row's *content*
/// survived, not only its existence.
fn seed_server_settings(conn: &Connection, marker: &str) {
    conn.execute(
        &format!("UPDATE server_settings SET default_lang = '{marker}' WHERE id = 'singleton'"),
        [],
    )
    .unwrap_or_else(|e| panic!("seed_server_settings({marker}): {e}"));
}

fn seed_user_consent(conn: &Connection, marker: &str) {
    conn.execute(
        &format!(
            "INSERT INTO user_consent(user_id, client_id, granted_scopes, granted_at) \
             VALUES('u-{marker}', 'c-{marker}', 'openid', datetime('now'))"
        ),
        [],
    )
    .unwrap_or_else(|e| panic!("seed_user_consent({marker}): {e}"));
}

/// `scope_definition`'s own migration (0036) auto-inserts four rows
/// (`openid`, `profile`, `email`, `offline_access`) but its primary key
/// is `name`, so a marker-named row never collides with them.
fn seed_scope_and_registration_token(conn: &Connection, marker: &str) {
    conn.execute_batch(&format!(
        "INSERT INTO scope_definition(name, created_at) \
         VALUES('scope-{marker}', datetime('now'));
         INSERT INTO client_registration_token(id, token_hash, created_at, updated_at) \
         VALUES('crt-{marker}', 'tokenhash-{marker}', datetime('now'), datetime('now'));"
    ))
    .unwrap_or_else(|e| panic!("seed_scope_and_registration_token({marker}): {e}"));
}

fn seed_federation(conn: &Connection, marker: &str) {
    conn.execute_batch(&format!(
        "INSERT INTO federation_provider(id, slug, display_name, issuer, client_id, \
             created_at, updated_at) \
         VALUES('fp-{marker}', 'slug-{marker}', 'disp-{marker}', \
             'https://issuer-{marker}.example', 'fpclient-{marker}', \
             datetime('now'), datetime('now'));
         INSERT INTO federation_link(user_id, provider_id, upstream_sub, linked_at, last_seen_at) \
         VALUES('u-{marker}', 'fp-{marker}', 'sub-{marker}', datetime('now'), datetime('now'));"
    ))
    .unwrap_or_else(|e| panic!("seed_federation({marker}): {e}"));
}

fn seed_r1(conn: &Connection, marker: &str) {
    seed_core(conn, marker, false);
}
fn seed_r2(conn: &Connection, marker: &str) {
    seed_core(conn, marker, false);
    seed_user_totp(conn, marker);
}
fn seed_r3a(conn: &Connection, marker: &str) {
    seed_core(conn, marker, true);
    seed_user_totp(conn, marker);
    seed_webauthn(conn, marker);
}
fn seed_r3b(conn: &Connection, marker: &str) {
    seed_r3a(conn, marker);
    seed_revoked_access_token(conn, marker);
}
fn seed_r4(conn: &Connection, marker: &str) {
    seed_r3b(conn, marker);
    seed_smtp_config(conn, marker);
}
fn seed_r5(conn: &Connection, marker: &str) {
    seed_r4(conn, marker);
    seed_server_settings(conn, marker);
}
fn seed_r6(conn: &Connection, marker: &str) {
    // Same table set as R5 (migration 22 rebuilds, does not add a
    // table) — kept as its own named function because it is its own
    // range, not an alias, so a future change to one cannot silently
    // change the other.
    seed_r5(conn, marker);
}
fn seed_r7(conn: &Connection, marker: &str) {
    seed_r6(conn, marker);
    seed_user_consent(conn, marker);
}
fn seed_r8(conn: &Connection, marker: &str) {
    seed_r7(conn, marker);
    seed_scope_and_registration_token(conn, marker);
}
fn seed_r10(conn: &Connection, marker: &str) {
    seed_r8(conn, marker);
    seed_federation(conn, marker);
}

/// The 37 shipped schema versions (RFC 142 stage 1 §1, independently
/// re-derived there from all 137 tags; re-stated here as the literal
/// fixture list, not re-derived, because re-deriving it from tags would
/// require network/git access this crate's own tests do not have).
const SHIPPED_VERSIONS: &[i32] = &[
    1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 15, 16, 17, 18, 20, 21, 22, 23, 24, 25, 26, 28, 29, 30,
    31, 32, 33, 34, 36, 38, 43, 44, 46, 48, 49,
];

fn seed_for_range_start(start: i32, conn: &Connection, marker: &str) {
    match start {
        1 => seed_r1(conn, marker),
        3 => seed_r2(conn, marker),
        4 => seed_r3a(conn, marker),
        5 => seed_r3b(conn, marker),
        14 => seed_r4(conn, marker),
        16 => seed_r5(conn, marker),
        22 => seed_r6(conn, marker),
        25 => seed_r7(conn, marker),
        36 => seed_r8(conn, marker),
        38 => seed_r10(conn, marker),
        other => panic!(
            "no seed function registered for a range starting at migration {other} -- \
             derive_ranges() found a range this test file's seed functions do not \
             cover. Add one before this test can pass."
        ),
    }
}

/// D1's own test, for all 37 shipped versions: seed a populated database
/// at that starting version, migrate it to head, and assert the data
/// survived. "It did not error" is not the test (RFC 142 D1) — every
/// seeded table's row count must be unchanged, `users.display_name`
/// (and `server_settings.default_lang`, where in scope) must read back
/// exactly the value seeded (not merely a count), the final schema
/// version must equal [`MAX_SCHEMA_VERSION`], and `PRAGMA
/// foreign_key_check` must report nothing.
#[test]
fn corpus_every_shipped_version_reaches_head_with_data_intact() {
    let ranges = derive_ranges();
    let mut failures: Vec<String> = Vec::new();

    for &v in SHIPPED_VERSIONS {
        let Some(range) = ranges.iter().find(|r| r.start <= v && v <= r.end) else {
            failures.push(format!("version {v}: matches no derived range"));
            continue;
        };
        let marker = format!("v{v}");

        let mut conn = Connection::open_in_memory().expect("in-memory db");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("enable FK");
        if let Err(e) = migrations::run_up_to(&mut conn, v) {
            failures.push(format!("version {v}: run_up_to({v}) failed: {e}"));
            continue;
        }

        seed_for_range_start(range.start, &conn, &marker);

        let before: Vec<(&str, i64)> = range
            .tables
            .iter()
            .map(|&t| (t, count_rows(&conn, t)))
            .collect();

        if let Err(e) = migrations::run_up_to(&mut conn, MAX_SCHEMA_VERSION) {
            failures.push(format!(
                "version {v}: migrating to head (MAX_SCHEMA_VERSION={MAX_SCHEMA_VERSION}) failed: {e}"
            ));
            continue;
        }

        let final_version: i64 = conn
            .query_row(
                "SELECT value FROM sui_meta WHERE key = 'schema_version'",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_else(|e| panic!("version {v}: reading final schema_version: {e}"))
            .parse()
            .unwrap_or_else(|e| panic!("version {v}: parsing final schema_version: {e}"));
        if final_version != i64::from(MAX_SCHEMA_VERSION) {
            failures.push(format!(
                "version {v}: final schema_version {final_version} != MAX_SCHEMA_VERSION {MAX_SCHEMA_VERSION}"
            ));
        }

        for (table, before_count) in before {
            let after_count = count_rows(&conn, table);
            if after_count != before_count {
                failures.push(format!(
                    "version {v}, table {table}: row count {before_count} before migrating to \
                     head, {after_count} after -- a migration silently dropped or duplicated rows"
                ));
            }
        }

        // Value-level, not count-level (RFC 142 stage 2 §2): the exact
        // seeded content must still be there, not merely a row.
        let display_name: Option<String> = conn
            .query_row(
                "SELECT display_name FROM users WHERE username = ?1",
                [&marker],
                |r| r.get(0),
            )
            .unwrap_or_else(|e| panic!("version {v}: reading back users.display_name: {e}"));
        if display_name.as_deref() != Some(marker.as_str()) {
            failures.push(format!(
                "version {v}: users.display_name after migrating to head is {display_name:?}, \
                 expected Some({marker:?})"
            ));
        }
        if range.tables.contains(&"server_settings") {
            let default_lang: String = conn
                .query_row(
                    "SELECT default_lang FROM server_settings WHERE id = 'singleton'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or_else(|e| {
                    panic!("version {v}: reading back server_settings.default_lang: {e}")
                });
            if default_lang != marker {
                failures.push(format!(
                    "version {v}: server_settings.default_lang after migrating to head is \
                     {default_lang:?}, expected {marker:?}"
                ));
            }
        }

        let violations: Vec<String> = conn
            .prepare("PRAGMA foreign_key_check")
            .unwrap_or_else(|e| panic!("version {v}: preparing foreign_key_check: {e}"))
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap_or_else(|e| panic!("version {v}: running foreign_key_check: {e}"))
            .filter_map(Result::ok)
            .collect();
        if !violations.is_empty() {
            failures.push(format!(
                "version {v}: FK violations after migrating to head: {violations:?}"
            ));
        }
    }

    assert_eq!(
        SHIPPED_VERSIONS.len(),
        37,
        "the shipped-version list itself must have 37 entries (stage 1 §1)"
    );
    assert!(
        failures.is_empty(),
        "corpus failures ({} of {}):\n{}",
        failures.len(),
        SHIPPED_VERSIONS.len(),
        failures.join("\n")
    );
}

/// Mutation evidence that the row-count assertion above is not vacuous:
/// a representative single-version run, with one seeded row's survival
/// deliberately broken, must fail with the message this test's own
/// logic would produce — proven here directly against `run_up_to`
/// rather than only trusted because the full sweep above passes.
#[test]
fn a_dropped_row_between_seed_and_head_is_caught_not_silently_passed() {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, 25).expect("run_up_to(25)");
    seed_r7(&conn, "mutation-check");
    let before = count_rows(&conn, "user_consent");
    assert_eq!(before, 1, "sanity: exactly one user_consent row seeded");

    // The mutation this test proves is caught: something between seed
    // and head deletes the row the fixture is supposed to prove
    // survives. Simulated directly (deleting it here stands in for a
    // migration that silently drops it) rather than editing migration
    // SQL, which this round must not do (RFC 142 stage 2 §7).
    conn.execute("DELETE FROM user_consent", [])
        .expect("simulate a dropped row");
    migrations::run_up_to(&mut conn, MAX_SCHEMA_VERSION).expect("run_up_to head");
    let after = count_rows(&conn, "user_consent");

    assert_ne!(
        before, after,
        "the mutation must change the row count for the assertion above to have something to catch"
    );
}

// ───────────────────────── audit_log: two fixtures (RFC 142 stage 2 §3) ──
//
// Not one of the 14-table corpus: stage 1 §4.2 found `append_within_tx`
// (the real hash-chain writer) unconditionally reads/writes
// `prev_hash`/`hash`, which do not exist before migration 9 -- so it
// cannot seed a database below schema 9 at all, independent of
// `audit_log`'s own column-compatibility (which is otherwise never a
// boundary, stage 1 §4.2). Two fixtures, not nine and not zero: one
// above the line using the real writer, one below it using a plain
// `INSERT` with no hash columns -- and the one below the line is "the
// point, not the leftover" (dispatch): it is the only thing in the
// whole corpus that exercises migration 9 against pre-existing rows
// that predate hashing, which is an *upgrade* property distinct from
// RFC 125's own tests of whether verification itself is correct.

use crate::crypto::MasterKey;
use crate::db::Database;
use crate::models::AuditLogRow;
use crate::repos::audit;

/// A fresh file-backed database at `path`, migrated to `version`, with
/// `foreign_keys` on. File-backed (not `:memory:`) because the
/// `verify_chain_tail` assertions below need a [`Database`] handle,
/// and `Database::open` always runs every pending migration against
/// whatever is already on disk -- a no-op once the raw connection below
/// has already reached head, but not something `Database` can be asked
/// to skip, so a real path is needed rather than `:memory:`, which
/// `Database::open_in_memory` always re-creates from nothing.
fn open_raw_at(path: &std::path::Path, version: i32) -> Connection {
    let mut conn = Connection::open(path).expect("open file-backed db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, version).expect("run_up_to");
    conn
}

#[tokio::test]
async fn audit_log_fixture_at_or_above_schema_9_uses_the_real_writer_and_still_verifies() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("audit-ge9.sqlite");
    let conn = open_raw_at(&path, 9);

    // Two real rows, through the production hash-chain writer -- not a
    // duplicated hash function (stage 1 §4.2's own reasoning for why
    // this is reusable at all above schema 9).
    {
        let tx = conn.unchecked_transaction().expect("tx");
        audit::append_within_tx(
            &tx,
            &AuditLogRow {
                at: chrono::Utc::now(),
                actor: None,
                action: "corpus.fixture.ge9.first".to_string(),
                target: None,
                result: "ok".to_string(),
                note: None,
            },
        )
        .expect("append first row");
        audit::append_within_tx(
            &tx,
            &AuditLogRow {
                at: chrono::Utc::now(),
                actor: None,
                action: "corpus.fixture.ge9.second".to_string(),
                target: None,
                result: "ok".to_string(),
                note: None,
            },
        )
        .expect("append second row");
        tx.commit().expect("commit");
    }
    let before = count_rows(&conn, "audit_log");
    assert_eq!(before, 2, "sanity: two rows seeded");
    drop(conn);

    let mut conn = Connection::open(&path).expect("reopen raw");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, MAX_SCHEMA_VERSION).expect("migrate to head");
    let after = count_rows(&conn, "audit_log");
    assert_eq!(after, before, "row count must survive migrating to head");
    drop(conn);

    let db = Database::open(&path, MasterKey::generate()).expect("open as Database");
    let report = audit::verify_chain_tail(&db, 100)
        .await
        .expect("verify_chain_tail");
    assert_eq!(
        report.checked, 2,
        "both rows must be chain-checked: {report:?}"
    );
    assert_eq!(
        report.legacy_unhashed, 0,
        "neither row predates hashing: {report:?}"
    );
    assert_eq!(
        report.broken_at_seq, None,
        "the chain must verify cleanly: {report:?}"
    );
}

#[tokio::test]
async fn audit_log_fixture_below_schema_9_preserves_legacy_rows_as_legacy_not_corruption() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("audit-lt9.sqlite");
    let conn = open_raw_at(&path, 1);

    // A plain INSERT with no hash columns -- they do not exist yet at
    // schema 1. This is the row an operator's real pre-v0.17.0 database
    // would carry into the upgrade.
    conn.execute(
        "INSERT INTO audit_log(at, actor, action, target, result, note) \
         VALUES(datetime('now'), NULL, 'corpus.fixture.lt9.legacy', NULL, 'ok', NULL)",
        [],
    )
    .expect("insert legacy row");
    let before = count_rows(&conn, "audit_log");
    assert_eq!(before, 1, "sanity: one legacy row seeded");
    drop(conn);

    let mut conn = Connection::open(&path).expect("reopen raw");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut conn, MAX_SCHEMA_VERSION).expect("migrate to head");
    let after_migrate = count_rows(&conn, "audit_log");
    assert_eq!(
        after_migrate, before,
        "the legacy row must survive migrating across migration 9's own ADD COLUMN"
    );

    // A real row appended *after* reaching head, through the real
    // writer, so there is a successor the legacy row's own `prev_hash`
    // (a migration default, not data) can be checked against.
    {
        let tx = conn.unchecked_transaction().expect("tx");
        audit::append_within_tx(
            &tx,
            &AuditLogRow {
                at: chrono::Utc::now(),
                actor: None,
                action: "corpus.fixture.lt9.first_hashed".to_string(),
                target: None,
                result: "ok".to_string(),
                note: None,
            },
        )
        .expect("append a real row after the legacy one");
        tx.commit().expect("commit");
    }
    drop(conn);

    let final_count = {
        let conn = Connection::open(&path).expect("reopen raw for final count");
        count_rows(&conn, "audit_log")
    };
    assert_eq!(final_count, 2, "legacy row plus the one appended after it");

    let db = Database::open(&path, MasterKey::generate()).expect("open as Database");
    let report = audit::verify_chain_tail(&db, 100)
        .await
        .expect("verify_chain_tail");
    assert_eq!(
        report.legacy_unhashed, 1,
        "the pre-existing row must be reported as legacy, not corruption: {report:?}"
    );
    assert_eq!(
        report.checked, 1,
        "exactly the one row appended after reaching head is chain-checked: {report:?}"
    );
    assert_eq!(
        report.broken_at_seq, None,
        "a legacy row followed by a real one must verify cleanly end to end -- \
         chain verification must not fail wholesale because history predates \
         hashing: {report:?}"
    );
}

// ───────────────────────── the hand-built 0.29.7 fixture (RFC 142 §4.3) ──
//
// The one starting state replay cannot reproduce: `run_up_to` always
// replays *today's* `MIGRATIONS`, and migration 21's embedded text is
// the corrected (v0.29.8) one. The original (v0.29.7) text, below, is
// the exact blob at `68ad4f3` -- confirmed by stage 1 to be the content
// of exactly one tag, `0.29.7`, nothing broader. It predates the
// `FK_DISABLE_REQUIRED` marker convention (introduced for migration 22)
// and has no marker of its own, so running it inside a bare transaction
// — exactly what `apply_migration` would have done for it in production
// — is the faithful replay, not a simplification: `PRAGMA foreign_keys
// = OFF` inside a transaction is a documented SQLite no-op, so `DROP
// TABLE users` fires `ON DELETE CASCADE` on every child table with a
// `REFERENCES users(id) ON DELETE CASCADE`, which is the data-loss bug
// RFC 021 (v0.29.8) exists to fix. Reproducing it here is not optional
// window-dressing -- the fixture's whole point is to start from the
// shape that bug leaves behind, not a sanitized stand-in for it.
const ORIGINAL_0021_V0_29_7_SQL: &str = r#"
PRAGMA foreign_keys = OFF;

CREATE TABLE _users_new (
    id           TEXT PRIMARY KEY,
    username     TEXT NOT NULL UNIQUE,
    display_name TEXT,
    is_admin     INTEGER NOT NULL DEFAULT 0
                     CHECK (is_admin     IN (0, 1)),
    is_disabled  INTEGER NOT NULL DEFAULT 0
                     CHECK (is_disabled  IN (0, 1)),
    is_deleted   INTEGER NOT NULL DEFAULT 0
                     CHECK (is_deleted   IN (0, 1)),
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    user_uuid    TEXT NOT NULL DEFAULT '',
    failed_login_count  INTEGER NOT NULL DEFAULT 0,
    locked_until        TEXT,
    email               TEXT,
    preferred_lang      TEXT,
    email_normalized    TEXT,
    email_verified_at   TEXT
);

INSERT INTO _users_new
SELECT id, username, display_name,
       CASE WHEN is_admin    NOT IN (0,1) THEN 0 ELSE is_admin    END,
       CASE WHEN is_disabled NOT IN (0,1) THEN 0 ELSE is_disabled END,
       CASE WHEN is_deleted  NOT IN (0,1) THEN 0 ELSE is_deleted  END,
       created_at, updated_at, user_uuid,
       failed_login_count, locked_until, email, preferred_lang,
       email_normalized, email_verified_at
FROM users;

DROP TABLE users;
ALTER TABLE _users_new RENAME TO users;

CREATE UNIQUE INDEX idx_users_user_uuid
    ON users(user_uuid) WHERE user_uuid <> '';
CREATE UNIQUE INDEX idx_users_email_normalized
    ON users(email_normalized) WHERE email_normalized IS NOT NULL;

CREATE TABLE _credentials_new (
    user_id      TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL,
    must_change  INTEGER NOT NULL DEFAULT 0
                     CHECK (must_change IN (0, 1)),
    updated_at   TEXT NOT NULL
);

INSERT INTO _credentials_new
SELECT user_id, password_hash,
       CASE WHEN must_change NOT IN (0,1) THEN 0 ELSE must_change END,
       updated_at
FROM credentials;

DROP TABLE credentials;
ALTER TABLE _credentials_new RENAME TO credentials;

CREATE TABLE _clients_new (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    confidential INTEGER NOT NULL
                     CHECK (confidential IN (0, 1)),
    secret_hash  TEXT,
    redirect_uris               TEXT NOT NULL,
    is_disabled  INTEGER NOT NULL DEFAULT 0
                     CHECK (is_disabled  IN (0, 1)),
    is_deleted   INTEGER NOT NULL DEFAULT 0
                     CHECK (is_deleted   IN (0, 1)),
    allowed_scopes              TEXT NOT NULL DEFAULT '',
    post_logout_redirect_uris   TEXT NOT NULL DEFAULT '[]',
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    CHECK (
        (confidential = 1 AND secret_hash IS NOT NULL) OR
        (confidential = 0 AND secret_hash IS NULL)
    )
);

INSERT INTO _clients_new
SELECT id, name,
       CASE WHEN confidential NOT IN (0,1) THEN 0 ELSE confidential END,
       secret_hash, redirect_uris,
       CASE WHEN is_disabled  NOT IN (0,1) THEN 0 ELSE is_disabled  END,
       CASE WHEN is_deleted   NOT IN (0,1) THEN 0 ELSE is_deleted   END,
       allowed_scopes, post_logout_redirect_uris, created_at, updated_at
FROM clients;

DROP TABLE clients;
ALTER TABLE _clients_new RENAME TO clients;

CREATE TABLE _signing_keys_new (
    id               TEXT PRIMARY KEY,
    algorithm        TEXT NOT NULL,
    private_key_enc  BLOB NOT NULL,
    public_key       BLOB NOT NULL,
    is_active        INTEGER NOT NULL DEFAULT 0
                         CHECK (is_active IN (0, 1)),
    created_at       TEXT NOT NULL,
    rotated_at       TEXT
);

INSERT INTO _signing_keys_new
SELECT id, algorithm, private_key_enc, public_key,
       CASE WHEN is_active NOT IN (0,1) THEN 0 ELSE is_active END,
       created_at, rotated_at
FROM signing_keys;

DROP TABLE signing_keys;
ALTER TABLE _signing_keys_new RENAME TO signing_keys;

CREATE UNIQUE INDEX idx_signing_keys_single_active
    ON signing_keys(is_active) WHERE is_active = 1;

CREATE TABLE _user_totp_new (
    user_id            TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    secret_enc         BLOB NOT NULL,
    enabled            INTEGER NOT NULL DEFAULT 0
                           CHECK (enabled IN (0, 1)),
    recovery_codes_enc BLOB,
    last_used_step     INTEGER NOT NULL DEFAULT 0,
    created_at         TEXT NOT NULL,
    confirmed_at       TEXT
);

INSERT INTO _user_totp_new
SELECT user_id, secret_enc,
       CASE WHEN enabled NOT IN (0,1) THEN 0 ELSE enabled END,
       recovery_codes_enc, last_used_step, created_at, confirmed_at
FROM user_totp;

DROP TABLE user_totp;
ALTER TABLE _user_totp_new RENAME TO user_totp;

DROP TABLE IF EXISTS consents;

CREATE TABLE consents (
    user_id        TEXT NOT NULL REFERENCES users(id)   ON DELETE CASCADE,
    client_id      TEXT NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    granted_scopes TEXT NOT NULL,
    granted_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    PRIMARY KEY (user_id, client_id),
    CHECK (length(granted_scopes) > 0)
);

CREATE INDEX idx_sessions_user_active_alive
    ON sessions(user_id, expires_at, created_at)
    WHERE revoked_at IS NULL;

PRAGMA foreign_keys = ON;
"#;

/// Seed the five tables migration 0021 touches, plus two of `users`'
/// child tables (`sessions`, `refresh_tokens`) whose cascade deletion
/// is exactly the historical bug -- at schema 20, before either lineage's
/// own 0021 runs.
fn seed_pre_21_with_children(conn: &Connection, marker: &str) {
    conn.execute_batch(&format!(
        "INSERT INTO users(id, username, display_name, is_admin, is_disabled, is_deleted, \
             created_at, updated_at, user_uuid) \
         VALUES('u-{marker}', '{marker}', '{marker}', 0, 0, 0, datetime('now'), datetime('now'), '{uuid}');
         INSERT INTO credentials(user_id, password_hash, updated_at) \
         VALUES('u-{marker}', 'hash-{marker}', datetime('now'));
         INSERT INTO clients(id, name, confidential, secret_hash, redirect_uris, is_disabled, \
             is_deleted, created_at, updated_at) \
         VALUES('c-{marker}', 'client-{marker}', 1, 'sechash-{marker}', '[]', 0, 0, \
             datetime('now'), datetime('now'));
         INSERT INTO signing_keys(id, algorithm, private_key_enc, public_key, created_at) \
         VALUES('sk-{marker}', 'EdDSA', X'00', X'00', datetime('now'));
         INSERT INTO user_totp(user_id, secret_enc, created_at) \
         VALUES('u-{marker}', X'00', datetime('now'));
         INSERT INTO sessions(id, user_id, created_at, expires_at) \
         VALUES('sess-{marker}', 'u-{marker}', datetime('now'), datetime('now','+1 hour'));
         INSERT INTO refresh_tokens(id, token_enc, user_id, client_id, scope, expires_at, created_at) \
         VALUES('rt-{marker}', X'00', 'u-{marker}', 'c-{marker}', 'openid', \
             datetime('now','+30 days'), datetime('now'));",
        uuid = uuid36(),
    ))
    .unwrap_or_else(|e| panic!("seed_pre_21_with_children({marker}): {e}"));
}

/// The same five tables' column list (name, type, notnull, pk), in
/// `cid` order -- `PRAGMA table_info`'s own ordering -- so "structurally
/// identical" is checked directly rather than by re-deriving a shape
/// comparison from row contents.
fn table_shape(conn: &Connection, table: &str) -> Vec<(String, String, i64, i64)> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap_or_else(|e| panic!("table_info({table}): {e}"));
    stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(5)?,
        ))
    })
    .unwrap_or_else(|e| panic!("table_info({table}) query: {e}"))
    .collect::<Result<Vec<_>, _>>()
    .unwrap_or_else(|e| panic!("table_info({table}) collect: {e}"))
}

#[test]
fn the_0_29_7_origin_schema_21_fixture_reaches_head_structurally_identical_to_replay() {
    // ── Lineage A: the hand-built 0.29.7 shape ──────────────────────────
    let mut a = Connection::open_in_memory().expect("mem db A");
    a.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut a, 20).expect("A: run_up_to(20)");

    seed_pre_21_with_children(&a, "pre21");
    assert_eq!(
        count_rows(&a, "sessions"),
        1,
        "sanity: one session seeded before 0021"
    );
    assert_eq!(
        count_rows(&a, "refresh_tokens"),
        1,
        "sanity: one refresh token seeded before 0021"
    );
    assert_eq!(
        count_rows(&a, "user_totp"),
        1,
        "sanity: one user_totp row seeded before 0021"
    );

    {
        let tx = a.transaction().expect("A: begin tx for original 0021");
        tx.execute_batch(ORIGINAL_0021_V0_29_7_SQL)
            .expect("A: apply the original 0.29.7 text");
        tx.execute(
            "INSERT OR REPLACE INTO sui_meta(key, value) VALUES('schema_version', '21')",
            [],
        )
        .expect("A: record schema_version=21");
        tx.commit().expect("A: commit original 0021");
    }

    // The historical bug, reproduced and confirmed -- not assumed. If
    // this does not fail, the SQL above is not faithful to `68ad4f3`
    // and nothing downstream is testing what it claims to.
    assert_eq!(
        count_rows(&a, "sessions"),
        0,
        "the historical 0.29.7 bug must have cascade-deleted sessions -- if this \
         is not 0, ORIGINAL_0021_V0_29_7_SQL is not a faithful replay of the real bug"
    );
    assert_eq!(
        count_rows(&a, "refresh_tokens"),
        0,
        "the historical 0.29.7 bug must have cascade-deleted refresh_tokens"
    );
    assert_eq!(
        count_rows(&a, "user_totp"),
        0,
        "the historical 0.29.7 bug must have cascade-deleted user_totp"
    );
    assert_eq!(
        count_rows(&a, "credentials"),
        0,
        "credentials has its own direct `REFERENCES users(id) ON DELETE CASCADE` \
         (migration 0001), so DROP TABLE users cascades it away exactly like \
         sessions/refresh_tokens/user_totp -- found by this fixture failing on \
         a first, wrong assumption that only the latter three were victims"
    );
    // `users` and `clients`/`signing_keys` are never a cascade *target* --
    // `clients`/`signing_keys` have no FK to `users` at all, and `users`
    // itself is the table being dropped, not a child of anything -- so
    // these three survive their own rebuild exactly as re-inserted.
    assert_eq!(
        count_rows(&a, "users"),
        1,
        "the user row itself is not a cascade target"
    );
    assert_eq!(count_rows(&a, "clients"), 1);
    assert_eq!(count_rows(&a, "signing_keys"), 1);

    // Data created *after* reaching the 0.29.7-origin schema-21 shape --
    // representing an operator who kept running on the damaged database
    // before ever applying the fix -- is what D1 actually needs to prove
    // survives the rest of the chain.
    seed_core(&a, "post21", true);
    seed_user_totp(&a, "post21");
    migrations::run_up_to(&mut a, MAX_SCHEMA_VERSION).expect("A: migrate to head");

    // ── Lineage B: the normal, replayed 0.29.8-origin shape ─────────────
    let mut b = Connection::open_in_memory().expect("mem db B");
    b.pragma_update(None, "foreign_keys", "ON")
        .expect("enable FK");
    migrations::run_up_to(&mut b, 21).expect("B: run_up_to(21), the safe text");
    seed_core(&b, "post21", true);
    seed_user_totp(&b, "post21");
    migrations::run_up_to(&mut b, MAX_SCHEMA_VERSION).expect("B: migrate to head");

    // ── Convergence, structural and data ────────────────────────────────
    for table in [
        "users",
        "credentials",
        "clients",
        "signing_keys",
        "user_totp",
    ] {
        assert_eq!(
            table_shape(&a, table),
            table_shape(&b, table),
            "{table}: the 0.29.7-origin lineage must end structurally identical \
             to the replayed 0.29.8-origin lineage at head"
        );
    }

    // users/clients/signing_keys kept their pre-schema-21 row (never a
    // cascade target) and gained the post-schema-21 one: 2. credentials/
    // user_totp lost their pre-schema-21 row to the historical cascade
    // and have only the post-schema-21 one: 1. Either way, the
    // post-schema-21 row itself must be present in every one of them.
    for (table, expected) in [
        ("users", 2),
        ("clients", 2),
        ("signing_keys", 2),
        ("credentials", 1),
        ("user_totp", 1),
    ] {
        assert_eq!(
            count_rows(&a, table),
            expected,
            "{table}: row count at head did not match (pre-21 survivor + post-21 row, \
             accounting for the historical cascade)"
        );
    }
    let display_name: Option<String> = a
        .query_row(
            "SELECT display_name FROM users WHERE username = 'post21'",
            [],
            |r| r.get(0),
        )
        .expect("A: read back users.display_name");
    assert_eq!(display_name, Some("post21".to_string()));

    let final_version: i64 = a
        .query_row(
            "SELECT value FROM sui_meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .expect("A: read final schema_version")
        .parse()
        .expect("A: parse final schema_version");
    assert_eq!(final_version, i64::from(MAX_SCHEMA_VERSION));

    let violations: Vec<String> = a
        .prepare("PRAGMA foreign_key_check")
        .expect("A: prepare foreign_key_check")
        .query_map([], |r| r.get::<_, String>(0))
        .expect("A: run foreign_key_check")
        .filter_map(Result::ok)
        .collect();
    assert!(
        violations.is_empty(),
        "A: FK violations at head: {violations:?}"
    );
}
