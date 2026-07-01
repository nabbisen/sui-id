//! Schema migrations.
//!
//! Migrations are embedded SQL strings, run in order at startup. The current
//! applied version is recorded in `sui_meta` under the key `schema_version`.
//! This is intentionally simpler than a full migration framework: minimal
//! configuration, easy to reason about during recovery.

use crate::errors::{StoreError, StoreResult};
use rusqlite::Connection;

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
    let migration_result: StoreResult<()> = (|| {
        let tx = conn.transaction().map_err(StoreError::from)?;
        tx.execute_batch(m.sql).map_err(StoreError::from)?;
        tx.execute(
            "INSERT OR REPLACE INTO sui_meta(key, value) VALUES(?1, ?2)",
            (META_KEY_SCHEMA_VERSION, m.version.to_string()),
        )
        .map_err(StoreError::from)?;
        tx.commit().map_err(StoreError::from)?;
        Ok(())
    })();

    // Always restore FK enforcement after a FK_DISABLE migration, regardless
    // of success or failure. We ignore errors here deliberately: if the
    // connection is in a broken state the real error is in `migration_result`.
    if needs_fk_disable {
        let _ = conn.execute_batch("PRAGMA foreign_keys = ON;");
    }

    // Propagate any migration error now that FK state is restored.
    migration_result?;

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
pub fn run(conn: &mut Connection) -> StoreResult<()> {
    // Ensure the meta table exists before we ask it for its version. The
    // initial migration creates the table too (idempotent CREATE IF NOT
    // EXISTS), but we need to read from it before the migration runs.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sui_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;

    let current: i32 = conn
        .query_row(
            "SELECT value FROM sui_meta WHERE key = ?1",
            [META_KEY_SCHEMA_VERSION],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    for m in MIGRATIONS {
        if m.version <= current {
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
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sui_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    let current: i32 = conn
        .query_row(
            "SELECT value FROM sui_meta WHERE key = ?1",
            [META_KEY_SCHEMA_VERSION],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    for m in MIGRATIONS {
        if m.version <= current || m.version > max_version {
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
