//! `user_consent` repository — stored OIDC consent grants (RFC 038).

use chrono::{DateTime, Utc};
use sui_id_shared::ids::{ClientId, UserId};

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::UserConsentRow;

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserConsentRow> {
    Ok(UserConsentRow {
        user_id: row.get::<_, String>(0)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        client_id: row.get::<_, String>(1)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        granted_scopes: row.get(2)?,
        granted_at: row.get(3)?,
    })
}

/// RFC 072: a consent grant row enriched with the client's display name,
/// used for the `/me/apps` self-service page.
#[derive(Debug, Clone)]
pub struct ConsentGrantView {
    pub client_id: ClientId,
    /// The client's human-readable `name` field from the `clients` table.
    pub client_name: String,
    /// Space-separated scope tokens that were granted.
    pub granted_scopes: String,
    pub granted_at: DateTime<Utc>,
    /// NULL until the first token exchange after migration 0029.
    pub last_used_at: Option<DateTime<Utc>>,
}

/// RFC 072: list all active consent grants for a user, joined with the
/// client display name. Excludes soft-deleted clients.
pub async fn list_for_user(db: &Database, user_id: UserId) -> StoreResult<Vec<ConsentGrantView>> {
    let uid = user_id.to_string();
    db.with_read(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT uc.client_id, c.name, uc.granted_scopes, uc.granted_at, uc.last_used_at \
             FROM user_consent uc \
             JOIN clients c ON c.id = uc.client_id \
             WHERE uc.user_id = ?1 AND c.is_deleted = 0 \
             ORDER BY uc.granted_at DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![uid], |row| {
            Ok(ConsentGrantView {
                client_id: row.get::<_, String>(0)?.parse().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                client_name: row.get(1)?,
                granted_scopes: row.get(2)?,
                granted_at: row.get(3)?,
                last_used_at: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(crate::errors::StoreError::from)
    })
    .await
}

/// RFC 072, amended RFC 094 U20: revoke a consent grant and all refresh
/// tokens for the same `(user_id, client_id)`, inside the caller's own
/// transaction — `commands::revoke_consent` (U20) is the only sealed
/// caller, which brings its own transaction rather than this function's
/// former `BEGIN`/`COMMIT`/`ROLLBACK` (a sealed command cannot nest one
/// transaction inside another).
///
/// Refresh tokens first, consent second — the security-critical
/// invariant this always enforced: revoking the grant without
/// invalidating the refresh tokens would leave the app able to mint
/// access tokens indefinitely. **Zero `refresh_tokens` rows is normal**
/// (the user may hold none); **zero `user_consent` rows means there was
/// no grant to revoke**, which is `NotFound`, not a silent no-op — U17's
/// lesson, applied only to the row that actually means "nothing to do."
pub fn revoke_with_tokens_within_tx(
    conn: &rusqlite::Connection,
    user_id: UserId,
    client_id: ClientId,
) -> StoreResult<()> {
    let uid = user_id.to_string();
    let cid = client_id.to_string();
    conn.execute(
        "DELETE FROM refresh_tokens WHERE user_id = ?1 AND client_id = ?2",
        rusqlite::params![uid.clone(), cid.clone()],
    )?;
    let n = conn.execute(
        "DELETE FROM user_consent WHERE user_id = ?1 AND client_id = ?2",
        rusqlite::params![uid, cid],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

/// RFC 072: set `last_used_at = now()` for a grant after a successful
/// token exchange. Best-effort — failures are logged but not propagated.
pub async fn touch_last_used(
    db: &Database,
    user_id: UserId,
    client_id: ClientId,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    let uid = user_id.to_string();
    let cid = client_id.to_string();
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE user_consent SET last_used_at = ?3 \
             WHERE user_id = ?1 AND client_id = ?2",
            rusqlite::params![uid, cid, now],
        )?;
        Ok(())
    })
    .await
}

/// Look up a stored consent for `(user_id, client_id)`.
/// Returns `None` if no consent has been recorded yet.
pub async fn get(
    db: &Database,
    user_id: UserId,
    client_id: ClientId,
) -> StoreResult<Option<UserConsentRow>> {
    db.with_read(move |conn| {
        let r = conn
            .prepare(
                "SELECT user_id, client_id, granted_scopes, granted_at \
                 FROM user_consent WHERE user_id = ?1 AND client_id = ?2",
            )?
            .query_row(
                rusqlite::params![user_id.to_string(), client_id.to_string()],
                map,
            );
        match r {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    })
    .await
}

/// Whether a [`upsert_within_tx`] call found no existing grant (a first
/// approval) or replaced one (the user already had a grant for this
/// client, and this call changed its scopes or merely re-dated it). RFC
/// 094 U21: a first grant and a scope change to an existing grant are
/// different facts an operator reading a consent audit cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentGrantOutcome {
    Granted,
    Updated,
}

impl ConsentGrantOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Updated => "updated",
        }
    }
}

/// Upsert a consent grant, inside the caller's own transaction
/// (`commands::grant_consent`, U21, is the only sealed caller).
///
/// If a row already exists for `(user_id, client_id)`, replace
/// `granted_scopes` with the new value. Reports which happened: `INSERT
/// ... ON CONFLICT DO UPDATE` reports the same `changes()` either way, so
/// the only way to tell insert from update is the cheap existence read
/// this does first, in the same transaction.
pub fn upsert_within_tx(
    conn: &rusqlite::Connection,
    user_id: UserId,
    client_id: ClientId,
    granted_scopes: &str,
    now: DateTime<Utc>,
) -> StoreResult<ConsentGrantOutcome> {
    use rusqlite::OptionalExtension;
    let uid = user_id.to_string();
    let cid = client_id.to_string();
    let existed = conn
        .query_row(
            "SELECT 1 FROM user_consent WHERE user_id = ?1 AND client_id = ?2",
            rusqlite::params![uid.clone(), cid.clone()],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    conn.execute(
        "INSERT INTO user_consent (user_id, client_id, granted_scopes, granted_at) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(user_id, client_id) DO UPDATE SET \
           granted_scopes = excluded.granted_scopes, \
           granted_at     = excluded.granted_at",
        rusqlite::params![uid, cid, granted_scopes, now],
    )?;
    Ok(if existed {
        ConsentGrantOutcome::Updated
    } else {
        ConsentGrantOutcome::Granted
    })
}

/// Remove a stored consent grant. Called when a client is deleted.
/// No-ops silently if no row exists.
pub async fn revoke(db: &Database, user_id: UserId, client_id: ClientId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "DELETE FROM user_consent WHERE user_id = ?1 AND client_id = ?2",
            rusqlite::params![user_id.to_string(), client_id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Check whether a stored consent covers `requested_scopes`.
///
/// Returns `true` if every token in `requested_scopes` appears in
/// `granted_scopes`. Empty `requested_scopes` is always covered.
pub fn covers(granted_scopes: &str, requested_scopes: &str) -> bool {
    let granted: std::collections::HashSet<&str> = granted_scopes.split_whitespace().collect();
    requested_scopes
        .split_whitespace()
        .all(|s| granted.contains(s))
}

#[cfg(test)]
mod tests;
