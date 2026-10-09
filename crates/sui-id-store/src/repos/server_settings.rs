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
    models::{AuditLogRow, HibpMode, ServerSettingsRow},
    repos::audit,
};
use chrono::{DateTime, Utc};
use rusqlite::params;
use sui_id_shared::ids::UserId;

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

/// Update the server default UI language. `lang` is a BCP-47 tag
/// — application-layer validation should ensure it is one of
/// `Locale::ALL` before calling.
pub async fn update_default_lang(db: &Database, lang: &str, now: DateTime<Utc>) -> StoreResult<()> {
    let lang = lang.to_owned();
    db.with_conn(move |conn| {
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
    .await
}

/// Update the server-wide Pwned Passwords (HIBP) check mode.
pub async fn update_hibp_mode(
    db: &Database,
    mode: HibpMode,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(move |conn| {
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
    .await
}

/// Change the server-wide Pwned Passwords (HIBP) check mode on behalf of
/// `actor`, and record the change, **in one transaction** (RFC 120 D5).
///
/// Returns `(old, new)`. The audit row is `settings.hibp_mode.changed` with the
/// note `old=<mode> new=<mode>`; it is written whether or not the value differs,
/// because what is audited is that an administrator made this decision.
pub async fn change_hibp_mode(
    db: &Database,
    actor: UserId,
    mode: HibpMode,
    now: DateTime<Utc>,
) -> StoreResult<(HibpMode, HibpMode)> {
    db.with_tx(move |tx| {
        let old = get_within_tx(tx)?.hibp_mode;
        let n = tx.execute(
            "UPDATE server_settings SET hibp_mode = ?1, updated_at = ?2 WHERE id = ?3",
            params![mode.as_str(), now, SINGLETON_ID],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        audit::append_within_tx(
            tx,
            &AuditLogRow {
                at: now,
                actor: Some(actor),
                action: "settings.hibp_mode.changed".into(),
                target: None,
                result: "ok".into(),
                note: Some(format!("old={} new={}", old.as_str(), mode.as_str())),
            },
        )?;
        Ok((old, mode))
    })
    .await
}

/// Change the server-wide default language on behalf of `actor`, and record the
/// change, in one transaction. Returns `(old, new)`. The audit row is
/// `settings.default_language.changed` with the note `old=<tag> new=<tag>`.
pub async fn change_default_lang(
    db: &Database,
    actor: UserId,
    lang: &str,
    now: DateTime<Utc>,
) -> StoreResult<(String, String)> {
    let lang = lang.to_owned();
    db.with_tx(move |tx| {
        let old = get_within_tx(tx)?.default_lang;
        let n = tx.execute(
            "UPDATE server_settings SET default_lang = ?1, updated_at = ?2 WHERE id = ?3",
            params![lang, now, SINGLETON_ID],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        audit::append_within_tx(
            tx,
            &AuditLogRow {
                at: now,
                actor: Some(actor),
                action: "settings.default_language.changed".into(),
                target: None,
                result: "ok".into(),
                note: Some(format!("old={old} new={lang}")),
            },
        )?;
        Ok((old, lang))
    })
    .await
}

/// Update the idle-session-timeout value, in seconds. `0` means
/// the feature is disabled. Application-layer validation should
/// pin the value to an operationally sensible range before
/// calling.
pub async fn update_idle_session_timeout(
    db: &Database,
    secs: i64,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(move |conn| {
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
    .await
}

/// Update the concurrent-session cap. `0` means no cap. The
/// application enforces FIFO eviction at login time when the
/// resulting count would exceed this.
pub async fn update_max_concurrent_sessions(
    db: &Database,
    cap: i64,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(move |conn| {
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
    .await
}

/// Store the hashed bearer token for the `/metrics` endpoint (RFC 006).
/// Pass `None` to clear the token (disables bearer-token auth).
pub async fn update_metrics_token_hash(
    db: &Database,
    hash: Option<&str>,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let hash = hash.map(|h| h.to_owned());
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE server_settings SET metrics_token_hash = ?1, updated_at = ?2 WHERE id = ?3",
            params![hash, now, SINGLETON_ID],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}
