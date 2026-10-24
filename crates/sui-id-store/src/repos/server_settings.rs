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
    Database, StoreError, StoreResult,
    models::{HibpMode, ServerSettingsRow},
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
        metrics_token_hash: row.get(5)?,
        created_at: row.get::<_, DateTime<Utc>>(6)?,
        updated_at: row.get::<_, DateTime<Utc>>(7)?,
    })
}

const SELECT_COLUMNS: &str = "id, default_lang, hibp_mode, \
                              idle_session_timeout_secs, max_concurrent_sessions, \
                              metrics_token_hash, \
                              created_at, updated_at";

/// Fetch the singleton server-settings row. Migration 0016 inserts
/// the default row, so post-migration this never returns NotFound.
pub async fn get(db: &Database) -> StoreResult<ServerSettingsRow> {
    // Not `db.with_read(get_within_tx)`: `get_within_tx` is shared with
    // `change_hibp_mode`/`change_default_lang`'s `with_tx` (write) path and
    // must keep taking `&rusqlite::Connection`, so its one read is inlined
    // here instead of changing that shared helper's signature.
    db.with_read(|conn| {
        conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM server_settings WHERE id = ?1"
        ))?
        .query_row([SINGLETON_ID], map_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
            other => StoreError::from(other),
        })
    })
    .await
}

/// Same as [`get`], on a caller-held connection or transaction.
pub fn get_within_tx(conn: &rusqlite::Connection) -> StoreResult<ServerSettingsRow> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM server_settings WHERE id = ?1"),
        [SINGLETON_ID],
        map_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
        other => StoreError::from(other),
    })
}

// RFC 094 M2b stage 3: every mutator below is now `_within_tx`, called
// only from `commands.rs`'s sealed S01-S05. Before this stage, this file
// held two shapes for the same two settings (HIBP mode, default
// language): a bare `update_*` with no audit at all (the admin settings
// page's own path) and an atomic `change_*` (the setup wizard's path,
// already correct). The stage plan originally called this wave "sealing
// what is already atomic" -- wrong, because it traced the atomic pair
// and missed that production calls the bare one. One sealed command per
// setting now serves both callers; see the dispatch's §0 and §2.
//
// `update_idle_session_timeout`, `update_max_concurrent_sessions` and
// `update_metrics_token_hash` had no atomic pair at all -- their audit
// event names were declared in the inventory and never emitted by
// anything, which is this stage's sharper finding (§0).
//
// Measured before deleting anything: no `repos/server_settings/tests.rs`
// exists, but `update_hibp_mode`, `update_idle_session_timeout` and
// `update_max_concurrent_sessions` each have real test-seeding callers
// elsewhere in the workspace (`sui-id`'s e2e suite and
// `sui-id-core`'s `session_limit_tests.rs`) that have nothing to do with
// the audit path this stage closes -- the same shape stage 1 left
// `insert`/`cancel` in place for. Each keeps a thin async wrapper below,
// for that reason only; no production code calls any of the three.
// `update_default_lang`, `change_default_lang`, `change_hibp_mode` and
// `update_metrics_token_hash` have no such caller and are fully removed.

/// Update the server default UI language, inside the caller's
/// transaction. Returns the old tag. `lang` is a BCP-47 tag —
/// application-layer validation should ensure it is one of
/// `Locale::ALL` before calling.
pub fn update_default_lang_within_tx(
    conn: &rusqlite::Connection,
    lang: &str,
    now: DateTime<Utc>,
) -> StoreResult<String> {
    let old = get_within_tx(conn)?.default_lang;
    let n = conn.execute(
        "UPDATE server_settings SET default_lang = ?1, updated_at = ?2 WHERE id = ?3",
        params![lang, now, SINGLETON_ID],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(old)
}

/// Update the server-wide Pwned Passwords (HIBP) check mode, inside the
/// caller's transaction. Returns the old mode.
pub fn update_hibp_mode_within_tx(
    conn: &rusqlite::Connection,
    mode: HibpMode,
    now: DateTime<Utc>,
) -> StoreResult<HibpMode> {
    let old = get_within_tx(conn)?.hibp_mode;
    let n = conn.execute(
        "UPDATE server_settings SET hibp_mode = ?1, updated_at = ?2 WHERE id = ?3",
        params![mode.as_str(), now, SINGLETON_ID],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(old)
}

/// Test-seeding only — no production caller (measured). Real callers
/// use the sealed S02 command (`commands::change_hibp_mode`); this
/// exists because `sui-id`'s e2e suite seeds the mode directly, which
/// has nothing to do with S02's own audit path.
pub async fn update_hibp_mode(
    db: &Database,
    mode: HibpMode,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_tx(move |tx| update_hibp_mode_within_tx(tx, mode, now).map(|_| ()))
        .await
}

/// Update the idle-session-timeout value, in seconds, inside the
/// caller's transaction. Returns the old value. `0` means the feature
/// is disabled. Application-layer validation should pin the value to
/// an operationally sensible range before calling.
pub fn update_idle_session_timeout_within_tx(
    conn: &rusqlite::Connection,
    secs: i64,
    now: DateTime<Utc>,
) -> StoreResult<i64> {
    let old = get_within_tx(conn)?.idle_session_timeout_secs;
    let n = conn.execute(
        "UPDATE server_settings SET idle_session_timeout_secs = ?1, \
         updated_at = ?2 WHERE id = ?3",
        params![secs, now, SINGLETON_ID],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(old)
}

/// Test-seeding only — no production caller (measured). Real callers
/// use the sealed S03 command (`commands::change_idle_session_timeout`);
/// this exists because `sui-id-core`'s `session_limit_tests.rs` and
/// `sui-id`'s e2e suite seed the value directly, unrelated to S03's own
/// audit path.
pub async fn update_idle_session_timeout(
    db: &Database,
    secs: i64,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_tx(move |tx| update_idle_session_timeout_within_tx(tx, secs, now).map(|_| ()))
        .await
}

/// Update the concurrent-session cap, inside the caller's transaction.
/// Returns the old value. `0` means no cap. The application enforces
/// FIFO eviction at login time when the resulting count would exceed
/// this.
pub fn update_max_concurrent_sessions_within_tx(
    conn: &rusqlite::Connection,
    cap: i64,
    now: DateTime<Utc>,
) -> StoreResult<i64> {
    let old = get_within_tx(conn)?.max_concurrent_sessions;
    let n = conn.execute(
        "UPDATE server_settings SET max_concurrent_sessions = ?1, \
         updated_at = ?2 WHERE id = ?3",
        params![cap, now, SINGLETON_ID],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(old)
}

/// Test-seeding only — no production caller (measured). Real callers
/// use the sealed S04 command (`commands::change_max_concurrent_sessions`);
/// this exists for the same reason `update_idle_session_timeout`'s
/// wrapper does.
pub async fn update_max_concurrent_sessions(
    db: &Database,
    cap: i64,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_tx(move |tx| update_max_concurrent_sessions_within_tx(tx, cap, now).map(|_| ()))
        .await
}

/// Store the hashed bearer token for the `/metrics` endpoint (RFC 006),
/// inside the caller's transaction. Pass `None` to clear the token
/// (disables bearer-token auth). Returns nothing — the dispatch's own
/// instruction is that neither the new nor the old hash becomes an
/// audit attribute, so there is no caller that needs the old value.
pub fn update_metrics_token_hash_within_tx(
    conn: &rusqlite::Connection,
    hash: Option<&str>,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let n = conn.execute(
        "UPDATE server_settings SET metrics_token_hash = ?1, updated_at = ?2 WHERE id = ?3",
        params![hash, now, SINGLETON_ID],
    )?;
    if n == 0 {
        Err(StoreError::NotFound)
    } else {
        Ok(())
    }
}
