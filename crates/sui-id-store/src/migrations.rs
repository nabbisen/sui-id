//! Schema migrations.
//!
//! Migrations are embedded SQL strings, run in order at startup. The current
//! applied version is recorded in `sui_meta` under the key `schema_version`.
//! This is intentionally simpler than a full migration framework: minimal
//! configuration, easy to reason about during recovery.

use crate::errors::{StoreError, StoreResult};
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::path::Path;

struct Migration {
    version: i32,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("./migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("./migrations/0002_client_scope_and_logout_uris.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("./migrations/0003_totp_mfa.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("./migrations/0004_webauthn.sql"),
    },
    Migration {
        version: 5,
        sql: include_str!("./migrations/0005_revoked_access_tokens.sql"),
    },
    Migration {
        version: 6,
        sql: include_str!("./migrations/0006_session_auth_methods.sql"),
    },
    Migration {
        version: 7,
        sql: include_str!("./migrations/0007_user_lockout.sql"),
    },
    Migration {
        version: 8,
        sql: include_str!("./migrations/0008_refresh_token_family.sql"),
    },
    Migration {
        version: 9,
        sql: include_str!("./migrations/0009_audit_hash_chain.sql"),
    },
    Migration {
        version: 10,
        sql: include_str!("./migrations/0010_session_step_up.sql"),
    },
    Migration {
        version: 11,
        sql: include_str!("./migrations/0011_audit_log_at_action_index.sql"),
    },
    Migration {
        version: 12,
        sql: include_str!("./migrations/0012_users_email.sql"),
    },
    Migration {
        version: 13,
        sql: include_str!("./migrations/0013_webauthn_step_up.sql"),
    },
    Migration {
        version: 14,
        sql: include_str!("./migrations/0014_smtp_config.sql"),
    },
    Migration {
        version: 15,
        sql: include_str!("./migrations/0015_password_reset_tokens.sql"),
    },
    Migration {
        version: 16,
        sql: include_str!("./migrations/0016_i18n.sql"),
    },
    Migration {
        version: 17,
        sql: include_str!("./migrations/0017_hibp_mode.sql"),
    },
    Migration {
        version: 18,
        sql: include_str!("./migrations/0018_session_limits.sql"),
    },
    Migration {
        version: 19,
        sql: include_str!("./migrations/0019_auth_flow_integrity.sql"),
    },
    Migration {
        version: 20,
        sql: include_str!("./migrations/0020_user_identity_invariants.sql"),
    },
    Migration {
        version: 21,
        sql: include_str!("./migrations/0021_schema_invariants.sql"),
    },
    Migration {
        version: 22,
        sql: include_str!("./migrations/0022_boolean_checks.sql"),
    },
    Migration {
        version: 23,
        sql: include_str!("./migrations/0023_email_outbox.sql"),
    },
    Migration {
        version: 24,
        sql: include_str!("./migrations/0024_email_outbox_locale.sql"),
    },
    Migration {
        version: 25,
        sql: include_str!("./migrations/0025_consent.sql"),
    },
    Migration {
        version: 26,
        sql: include_str!("./migrations/0026_me_security_index.sql"),
    },
    Migration {
        version: 27,
        sql: include_str!("./migrations/0027_users_role.sql"),
    },
    Migration {
        version: 28,
        sql: include_str!("./migrations/0028_audit_actor_role.sql"),
    },
    Migration {
        version: 29,
        sql: include_str!("./migrations/0029_user_consent_last_used.sql"),
    },
    Migration {
        version: 30,
        sql: include_str!("./migrations/0030_users_last_login.sql"),
    },
    Migration {
        version: 31,
        sql: include_str!("./migrations/0031_auth_code_index.sql"),
    },
    Migration {
        version: 32,
        sql: include_str!("./migrations/0032_pending_settings_change.sql"),
    },
    Migration {
        version: 33,
        sql: include_str!("./migrations/0033_server_settings_metrics_token.sql"),
    },
    Migration {
        version: 34,
        sql: include_str!("./migrations/0034_users_source.sql"),
    },
    Migration {
        version: 35,
        sql: include_str!("./migrations/0035_clients_app_identity.sql"),
    },
    Migration {
        version: 36,
        sql: include_str!("./migrations/0036_scope_definition_and_reg_token.sql"),
    },
    Migration {
        version: 37,
        sql: include_str!("./migrations/0037_federation_provider.sql"),
    },
    Migration {
        version: 38,
        sql: include_str!("./migrations/0038_federation_link.sql"),
    },
    Migration {
        version: 39,
        sql: include_str!("./migrations/0039_step_up_failure_accounting.sql"),
    },
    Migration {
        version: 40,
        sql: include_str!("./migrations/0040_mfa_failure_count.sql"),
    },
    Migration {
        version: 41,
        sql: include_str!("./migrations/0041_recovery_link_tokens.sql"),
    },
    Migration {
        version: 42,
        sql: include_str!("./migrations/0042_recovery_link_provisioning.sql"),
    },
    Migration {
        version: 43,
        sql: include_str!("./migrations/0043_drop_credentials_must_change.sql"),
    },
];

/// The highest schema version this build of sui-id-store knows how to
/// produce by running its bundled migrations. The backup-restore path
/// uses this to refuse a backup that was taken on a newer sui-id (the
/// migration to read it forward doesn't exist yet) — reversibly,
/// rebuild with a newer binary.
pub const MAX_SCHEMA_VERSION: i32 = {
    // Computed at compile-time from the MIGRATIONS slice. If you add a
    // new migration above, this picks up the new top automatically.
    let mut i = 0;
    let mut max = 0i32;
    while i < MIGRATIONS.len() {
        if MIGRATIONS[i].version > max {
            max = MIGRATIONS[i].version;
        }
        i += 1;
    }
    max
};

const META_KEY_SCHEMA_VERSION: &str = "schema_version";

/// RFC 112 D7: the release that last migrated the database, written in the same
/// transaction as the version. It lets a refusal name the binary to run. No
/// schema change: `sui_meta` is a key/value table.
const META_KEY_LAST_MIGRATED_BY: &str = "last_migrated_by";

// ── RFC 112: one reader, one rule ───────────────────────────────────────

/// What the database says its schema version is (RFC 112 D2, D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredVersion {
    /// No application tables at all: a database that has never been migrated.
    /// The only state in which "no version" means version 0.
    Fresh,
    /// A canonical non-negative integer was recorded. Beyond `i64::MAX` it
    /// saturates at `i64::MAX`, which is still unmistakably too new.
    At(i64),
}

impl StoredVersion {
    /// The version as a number; `Fresh` is 0.
    pub fn number(self) -> i64 {
        match self {
            Self::Fresh => 0,
            Self::At(v) => v,
        }
    }
}

/// Why the stored version cannot be used (RFC 112 D6). Every caller of
/// [`read_stored_version`] and [`check_supported`] gets the same answers.
#[derive(Debug)]
pub enum SchemaError {
    /// Newer than [`MAX_SCHEMA_VERSION`].
    TooNew { found: i64, supported: i32 },
    /// Unreadable, or absent from a database that has application tables.
    Invalid { tables: usize, detail: String },
    /// The read itself failed. It is returned, never collapsed to a version.
    Read(rusqlite::Error),
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooNew { found, supported } => write!(
                f,
                "schema version {found} is newer than this build supports (up to {supported})"
            ),
            Self::Invalid { tables, detail } => {
                write!(
                    f,
                    "schema version unreadable: {detail} ({tables} application table(s))"
                )
            }
            Self::Read(e) => write!(f, "reading the schema version failed: {e}"),
        }
    }
}

impl std::error::Error for SchemaError {}

impl From<SchemaError> for StoreError {
    fn from(e: SchemaError) -> Self {
        match e {
            SchemaError::TooNew { found, supported } => {
                StoreError::SchemaTooNew { found, supported }
            }
            SchemaError::Invalid { tables, detail } => {
                StoreError::SchemaVersionInvalid { tables, detail }
            }
            SchemaError::Read(e) => StoreError::Db(e),
        }
    }
}

impl From<rusqlite::Error> for SchemaError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Read(e)
    }
}

/// Read the stored schema version **without writing anything and without
/// creating anything**: it is safe on a read-only connection, before any pragma.
///
/// - Version `0` ([`StoredVersion::Fresh`]) only when the database has **no
///   tables other than `sui_meta`** (RFC 112 D2). A populated database with no
///   version row, and a foreign SQLite file, are `Invalid`.
/// - A value that is not a canonical non-negative integer (`+43`, ` 43`, `043`,
///   `-1`, empty, or a BLOB) is `Invalid`.
/// - Any read error is returned as [`SchemaError::Read`], not turned into a
///   version.
pub fn read_stored_version(conn: &Connection) -> Result<StoredVersion, SchemaError> {
    let tables: usize = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'table' AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' AND name <> 'sui_meta'",
        [],
        |r| r.get::<_, i64>(0),
    )? as usize;
    let has_meta: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'sui_meta'",
        [],
        |r| r.get::<_, i64>(0),
    )? > 0;

    let raw: Option<rusqlite::types::Value> = if has_meta {
        match conn.query_row(
            "SELECT value FROM sui_meta WHERE key = ?1",
            [META_KEY_SCHEMA_VERSION],
            |r| r.get::<_, rusqlite::types::Value>(0),
        ) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(SchemaError::Read(e)),
        }
    } else {
        None
    };

    let invalid = |detail: String| SchemaError::Invalid { tables, detail };
    match raw {
        None if tables == 0 => Ok(StoredVersion::Fresh),
        None => Err(invalid("no schema version is recorded".into())),
        Some(rusqlite::types::Value::Text(s)) => {
            let canonical = !s.is_empty()
                && s.bytes().all(|b| b.is_ascii_digit())
                && (s == "0" || !s.starts_with('0'));
            if !canonical {
                return Err(invalid(format!(
                    "the recorded value {:?} is not a non-negative integer",
                    s.chars().take(40).collect::<String>()
                )));
            }
            // All digits: a parse failure can only be overflow, i.e. too new.
            let v = s.parse::<i64>().unwrap_or(i64::MAX);
            if v == 0 && tables > 0 {
                return Err(invalid(
                    "the recorded version is 0 but the database is populated".into(),
                ));
            }
            Ok(StoredVersion::At(v))
        }
        Some(other) => Err(invalid(format!(
            "the recorded version is stored as {}, not text",
            match other {
                rusqlite::types::Value::Null => "NULL",
                rusqlite::types::Value::Integer(_) => "an integer",
                rusqlite::types::Value::Real(_) => "a real",
                rusqlite::types::Value::Blob(_) => "a BLOB",
                rusqlite::types::Value::Text(_) => "text",
            }
        ))),
    }
}

/// The one rule (RFC 112 D1): a stored version above [`MAX_SCHEMA_VERSION`] is
/// refused. There is no override.
pub fn check_supported(stored: StoredVersion) -> Result<(), SchemaError> {
    match stored {
        StoredVersion::At(found) if found > i64::from(MAX_SCHEMA_VERSION) => {
            Err(SchemaError::TooNew {
                found,
                supported: MAX_SCHEMA_VERSION,
            })
        }
        _ => Ok(()),
    }
}

/// The release that last migrated the database (RFC 112 D7), if it recorded
/// one. Best effort: absent on a database no build with D7 has migrated, and
/// never an error, because it only improves a message.
pub fn last_migrated_by(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT value FROM sui_meta WHERE key = ?1",
        [META_KEY_LAST_MIGRATED_BY],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

/// [`last_migrated_by`] for a database **file**, read-only, best effort. Used
/// to name the release in a refusal (RFC 112 D5, D7): the refusing binary has no
/// open connection, and must not open one for writing. `None` if the file cannot
/// be read or records no release.
pub fn last_migrated_by_file(path: &Path) -> Option<String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    last_migrated_by(&conn)
}

/// RFC 112 D3: decide about the database **file** before anything touches it.
///
/// `Database::open` used to set `journal_mode = WAL` before `run`, and `run`
/// created `sui_meta` before reading the version, so a refusal inside `run`
/// came after the file had been written to (a rollback-journal database was
/// converted to WAL before it could be refused). This opens the file
/// `SQLITE_OPEN_READ_ONLY`, reads, decides, and closes it. A path that does not
/// exist yet is a fresh database and passes.
pub fn check_database_file(path: &Path) -> StoreResult<()> {
    if !path.exists() {
        return Ok(());
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let stored = read_stored_version(&conn)?;
    check_supported(stored)?;
    Ok(())
}

/// Migrations whose SQL begins with this marker line require foreign key
/// enforcement to be disabled on the connection **before** the transaction
/// begins. This is necessary for migrations that rebuild parent tables
/// (DROP + RENAME) without wanting ON DELETE CASCADE to fire.
///
/// Background: `PRAGMA foreign_keys = OFF` is a no-op inside a SQLite
/// transaction (<https://www.sqlite.org/pragma.html#pragma_foreign_keys>).
/// Setting it before the transaction starts does carry into the transaction.
/// After the transaction commits, the runner re-enables FK enforcement and
/// runs `PRAGMA foreign_key_check` to abort with an error if the migration
/// left any FK violations.
const FK_DISABLE_MARKER: &str = "-- MIGRATION:FK_DISABLE_REQUIRED";

/// Apply a single migration to `conn`, handling FK_DISABLE_REQUIRED safely.
///
/// ### FK restoration guarantee
///
/// If the migration is marked `FK_DISABLE_REQUIRED`, this function:
/// 1. Sets `PRAGMA foreign_keys = OFF` **before** the transaction (outside
///    the transaction, so it actually takes effect).
/// 2. Runs the migration inside its own transaction.
/// 3. Always restores `PRAGMA foreign_keys = ON` afterwards — even if the
///    migration fails. This prevents the connection from being left in a
///    state where FK enforcement is silently disabled.
/// 4. After a successful FK_DISABLE migration, runs `PRAGMA foreign_key_check`
///    to catch any FK violations introduced by the migration SQL itself.
///
/// The caller must not use `conn` for anything if this function returns `Err`.
/// In practice `Database::open()` propagates the error immediately to the
/// caller, which discards the connection.
fn apply_migration(conn: &mut Connection, m: &Migration) -> StoreResult<()> {
    let needs_fk_disable = m.sql.trim_start().starts_with(FK_DISABLE_MARKER);
    tracing::info!(version = m.version, needs_fk_disable, "applying migration");

    // For table-rebuild migrations: disable FK enforcement BEFORE the
    // transaction so DROP TABLE does not fire ON DELETE CASCADE.
    if needs_fk_disable {
        conn.execute_batch("PRAGMA foreign_keys = OFF;")
            .map_err(StoreError::from)?;
    }

    // Run the migration in a closure so we can restore FK state regardless
    // of whether the transaction succeeds or fails.
    let migration_result: StoreResult<bool> = (|| {
        let failed = |source| StoreError::MigrationFailed {
            version: m.version,
            source,
        };
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)?;
        // Re-read under the write lock: another process may have applied this
        // migration since `run` looked, or a newer binary may have gone past
        // this build's ceiling.
        let stored = read_stored_version(&tx)?;
        check_supported(stored)?;
        if stored.number() >= i64::from(m.version) {
            return Ok(false);
        }
        tx.execute_batch(m.sql).map_err(failed)?;
        tx.execute(
            "INSERT OR REPLACE INTO sui_meta(key, value) VALUES(?1, ?2)",
            (META_KEY_SCHEMA_VERSION, m.version.to_string()),
        )
        .map_err(failed)?;
        // RFC 112 D7: in the same transaction as the version.
        tx.execute(
            "INSERT OR REPLACE INTO sui_meta(key, value) VALUES(?1, ?2)",
            (META_KEY_LAST_MIGRATED_BY, env!("CARGO_PKG_VERSION")),
        )
        .map_err(failed)?;
        tx.commit().map_err(failed)?;
        Ok(true)
    })();

    // Always restore FK enforcement after a FK_DISABLE migration, regardless
    // of success or failure. We ignore errors here deliberately: if the
    // connection is in a broken state the real error is in `migration_result`.
    if needs_fk_disable {
        let _ = conn.execute_batch("PRAGMA foreign_keys = ON;");
    }

    // Propagate any migration error now that FK state is restored. `false`
    // means another process had already applied this migration.
    if !migration_result? {
        return Ok(());
    }

    // After a successful FK_DISABLE migration, verify FK integrity. Any
    // violation here means the migration SQL had a bug and we should refuse
    // to start rather than silently corrupt the DB.
    if needs_fk_disable {
        let mut stmt = conn
            .prepare("PRAGMA foreign_key_check")
            .map_err(StoreError::from)?;
        let first_violation = stmt.query_row([], |r| r.get::<_, String>(0)).ok();
        if let Some(table) = first_violation {
            return Err(StoreError::Integrity(format!(
                "migration {}: FK violation after rebuild in table {table:?}; \
                 run `PRAGMA foreign_key_check` for details",
                m.version
            )));
        }
    }

    Ok(())
}

/// Apply all pending migrations to `conn`.
///
/// The version is read, and judged, inside one `BEGIN IMMEDIATE` (RFC 112), so
/// two new binaries starting together do not both decide to apply the same
/// migration; and a database this build does not understand is refused before
/// `sui_meta` is created or anything is written. `apply_migration` re-reads
/// under its own immediate transaction for the same reason.
pub fn run(conn: &mut Connection) -> StoreResult<()> {
    let current = {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = read_stored_version(&tx)?;
        check_supported(stored)?;
        // Only now is it safe to create anything. The initial migration creates
        // the table too (idempotent CREATE IF NOT EXISTS).
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS sui_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        tx.commit()?;
        stored.number()
    };

    for m in MIGRATIONS {
        if i64::from(m.version) <= current {
            continue;
        }
        apply_migration(conn, m)?;
    }
    Ok(())
}

/// Apply migrations up to and including `max_version`. Used in tests to
/// create a database at a known historical schema version so that a
/// subsequent migration can be applied manually and its data-preservation
/// behaviour verified.
///
/// Uses the same `apply_migration()` as `run()`, so FK_DISABLE_REQUIRED
/// migrations are handled identically.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) fn run_up_to(conn: &mut Connection, max_version: i32) -> StoreResult<()> {
    let current = read_stored_version(conn)?.number();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sui_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    for m in MIGRATIONS {
        if i64::from(m.version) <= current || m.version > max_version {
            continue;
        }
        apply_migration(conn, m)?;
    }
    Ok(())
}

/// Return the SQL for the migration at the given version. Panics if the
/// version does not exist — this is intentionally strict so that test
/// helper code fails loudly when migrations are renumbered.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
pub(crate) fn sql_for_version(version: i32) -> &'static str {
    MIGRATIONS
        .iter()
        .find(|m| m.version == version)
        .unwrap_or_else(|| panic!("no migration with version {version}"))
        .sql
}
