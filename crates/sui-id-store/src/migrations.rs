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

/// Apply all pending migrations to `conn`.
pub fn run(conn: &Connection) -> StoreResult<()> {
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
        tracing::info!(version = m.version, "applying migration");
        conn.execute_batch(m.sql).map_err(StoreError::from)?;
        conn.execute(
            "INSERT OR REPLACE INTO sui_meta(key, value) VALUES(?1, ?2)",
            (META_KEY_SCHEMA_VERSION, m.version.to_string()),
        )?;
    }
    Ok(())
}
