//! `federation_link` repository — per-user upstream identity links (RFC 004).
//!
//! The mapping key is always `(provider_id, upstream_sub)`.  Email is
//! stored as metadata only and is never used for lookup (P1).

use crate::{
    Database,
    errors::{StoreError, StoreResult},
    models::FederationLinkRow,
};
use rusqlite::params;
use sui_id_shared::ids::{FederationProviderId, UserId};

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<FederationLinkRow> {
    let user_id = row.get::<_, String>(0)?.parse::<UserId>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let provider_id = row
        .get::<_, String>(1)?
        .parse::<FederationProviderId>()
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;
    Ok(FederationLinkRow {
        user_id,
        provider_id,
        upstream_sub: row.get(2)?,
        upstream_email: row.get(3)?,
        linked_at: row.get(4)?,
        last_seen_at: row.get(5)?,
    })
}

const SELECT: &str = "SELECT user_id, provider_id, upstream_sub, upstream_email, linked_at, last_seen_at \
     FROM federation_link";

/// Find a link by `(provider_id, upstream_sub)` — the authoritative mapping
/// key (P1).  Returns `None` if no link exists (not an error).
pub async fn find_by_sub(
    db: &Database,
    provider_id: FederationProviderId,
    upstream_sub: &str,
) -> StoreResult<Option<FederationLinkRow>> {
    let pid = provider_id.to_string();
    let sub = upstream_sub.to_owned();
    db.with_read(move |conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE provider_id = ?1 AND upstream_sub = ?2"
        ))?;
        let mut rows = stmt.query_map(params![pid, sub], map)?;
        match rows.next() {
            Some(Ok(r)) => Ok(Some(r)),
            Some(Err(e)) => Err(StoreError::from(e)),
            None => Ok(None),
        }
    })
    .await
}

/// Check whether any user is linked to a given `(provider_id, email)`.
/// Used for the account-takeover guard (P2): if email matches and the
/// provider_id differs, it may be an attempted takeover.
pub async fn find_any_by_email(
    db: &Database,
    provider_id: FederationProviderId,
    email: &str,
) -> StoreResult<Option<FederationLinkRow>> {
    let pid = provider_id.to_string();
    let em = email.to_lowercase();
    db.with_read(move |conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE provider_id = ?1 AND lower(upstream_email) = ?2 LIMIT 1"
        ))?;
        let mut rows = stmt.query_map(params![pid, em], map)?;
        match rows.next() {
            Some(Ok(r)) => Ok(Some(r)),
            Some(Err(e)) => Err(StoreError::from(e)),
            None => Ok(None),
        }
    })
    .await
}

/// List all links for a local user (for `/me/security` future use).
pub async fn list_for_user(db: &Database, user_id: UserId) -> StoreResult<Vec<FederationLinkRow>> {
    let uid = user_id.to_string();
    db.with_read(move |conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE user_id = ?1"))?;
        let rows = stmt.query_map([uid], map)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::from)
    })
    .await
}

/// Insert or update a federation link for `(user_id, provider_id)`.
///
/// On conflict (same primary key — user re-signing-in), update
/// `upstream_email` and `last_seen_at`.
pub async fn upsert(db: &Database, row: FederationLinkRow) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO federation_link \
             (user_id, provider_id, upstream_sub, upstream_email, linked_at, last_seen_at) \
             VALUES (?1,?2,?3,?4,?5,?6) \
             ON CONFLICT(user_id, provider_id) DO UPDATE SET \
             upstream_email = excluded.upstream_email, \
             last_seen_at   = excluded.last_seen_at",
            params![
                row.user_id.to_string(),
                row.provider_id.to_string(),
                row.upstream_sub,
                row.upstream_email,
                row.linked_at,
                row.last_seen_at,
            ],
        )?;
        Ok(())
    })
    .await
}

/// Remove a single link (user revokes one provider connection).
pub async fn delete(
    db: &Database,
    user_id: UserId,
    provider_id: FederationProviderId,
) -> StoreResult<()> {
    let uid = user_id.to_string();
    let pid = provider_id.to_string();
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM federation_link WHERE user_id = ?1 AND provider_id = ?2",
            params![uid, pid],
        )?;
        if n == 0 {
            Err(StoreError::NotFound)
        } else {
            Ok(())
        }
    })
    .await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
