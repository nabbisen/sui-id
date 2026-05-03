//! `server_settings` table — singleton row, see migration 0016.
//!
//! Holds process-wide settings configurable by an admin without a
//! restart. Today this is just `default_lang`; future settings
//! (UI theme defaults, password-policy knobs, etc) extend the row
//! without needing a new migration.
//!
//! The row is keyed on the literal string `'singleton'` and is
//! INSERTed as part of migration 0016 with conservative defaults,
//! so [`get`] is `Result<ServerSettingsRow>` rather than
//! `Result<Option<…>>` — the row always exists once migrations
//! have run.

use crate::{
    models::{HibpMode, ServerSettingsRow},
    Database, StoreError, StoreResult,
};
use chrono::{DateTime, Utc};
use rusqlite::params;

const SINGLETON_ID: &str = "singleton";

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ServerSettingsRow> {
    let mode_str: String = row.get(2)?;
    let hibp_mode = HibpMode::parse(&mode_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(StoreError::Integrity(format!(
                "unknown server_settings.hibp_mode value: {mode_str}"
            ))),
        )
    })?;
    Ok(ServerSettingsRow {
        default_lang: row.get(1)?,
        hibp_mode,
        idle_session_timeout_secs: row.get(3)?,
        max_concurrent_sessions: row.get(4)?,
        created_at: row.get::<_, DateTime<Utc>>(5)?,
        updated_at: row.get::<_, DateTime<Utc>>(6)?,
    })
}

const SELECT_COLUMNS: &str = "id, default_lang, hibp_mode, \
                              idle_session_timeout_secs, max_concurrent_sessions, \
                              created_at, updated_at";

/// Fetch the singleton server-settings row. Migration 0016 inserts
/// the default row, so post-migration this never returns NotFound.
pub fn get(db: &Database) -> StoreResult<ServerSettingsRow> {
    db.with_conn(|conn| {
        conn.query_row(
            &format!("SELECT {SELECT_COLUMNS} FROM server_settings WHERE id = ?1"),
            [SINGLETON_ID],
            map_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
}

/// Update the server default UI language. `lang` is a BCP-47 tag
/// — application-layer validation should ensure it is one of
/// `Locale::ALL` before calling.
pub fn update_default_lang(
    db: &Database,
    lang: &str,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE server_settings SET default_lang = ?1, updated_at = ?2 WHERE id = ?3",
            params![lang, now, SINGLETON_ID],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
}

/// Update the server-wide Pwned Passwords (HIBP) check mode.
pub fn update_hibp_mode(
    db: &Database,
    mode: HibpMode,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE server_settings SET hibp_mode = ?1, updated_at = ?2 WHERE id = ?3",
            params![mode.as_str(), now, SINGLETON_ID],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
}

/// Update the idle-session-timeout value, in seconds. `0` means
/// the feature is disabled. Application-layer validation should
/// pin the value to an operationally sensible range before
/// calling.
pub fn update_idle_session_timeout(
    db: &Database,
    secs: i64,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE server_settings SET idle_session_timeout_secs = ?1, \
             updated_at = ?2 WHERE id = ?3",
            params![secs, now, SINGLETON_ID],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
}

/// Update the concurrent-session cap. `0` means no cap. The
/// application enforces FIFO eviction at login time when the
/// resulting count would exceed this.
pub fn update_max_concurrent_sessions(
    db: &Database,
    cap: i64,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(|conn| {
        let n = conn.execute(
            "UPDATE server_settings SET max_concurrent_sessions = ?1, \
             updated_at = ?2 WHERE id = ?3",
            params![cap, now, SINGLETON_ID],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
}
