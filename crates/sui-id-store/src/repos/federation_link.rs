//! `federation_link` repository — per-user upstream identity links (RFC 004).
//!
//! The mapping key is always `(provider_id, upstream_sub)`.  Email is
//! stored as metadata only and is never used for lookup (P1).

use crate::{
    Database,
    errors::{StoreError, StoreResult},
    models::FederationLinkRow,
};
use rusqlite::{OptionalExtension, params};
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

/// Create exactly one federation link, on the caller's own transaction
/// (RFC 096-B1 stage 7, F04). **Insert, never upsert** — unlike
/// [`upsert`], a conflict is refused rather than silently absorbed,
/// because the manifest's own recheck exists to catch exactly the race an
/// upsert would hide: a concurrent callback for the same upstream identity
/// already creating the first link. Either `UNIQUE` constraint on this
/// table can fire here — `(user_id, provider_id)` (this `user_id` already
/// linked to this provider, which should not happen for a freshly
/// generated id, but is reported the same way regardless) or
/// `(provider_id, upstream_sub)` (another user already linked to this
/// exact upstream identity, the race F04's recheck is for) — both map to
/// [`StoreError::Conflict`], since the caller's response to either is the
/// same: the write it had planned is no longer safe to make.
pub fn insert_within_tx(conn: &rusqlite::Connection, row: &FederationLinkRow) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO federation_link \
         (user_id, provider_id, upstream_sub, upstream_email, linked_at, last_seen_at) \
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            row.user_id.to_string(),
            row.provider_id.to_string(),
            row.upstream_sub,
            row.upstream_email,
            row.linked_at,
            row.last_seen_at,
        ],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::Conflict
        }
        other => StoreError::from(other),
    })?;
    Ok(())
}

/// Durable existence re-read, on the caller's own transaction (RFC
/// 096-B1 stage 10b fix round, R2: F03's own authority recheck —
/// "exact provider/**link**/user/factor/method", `contracts/write-
/// commands.toml`'s own F03 row). Under normal operation this can
/// never observe a missing link against a *live* `federation_mfa_pending`
/// row: that row's own composite `FOREIGN KEY ... REFERENCES
/// federation_link ON DELETE CASCADE` (migration 0047) removes the row
/// itself the instant the link is removed, so F03's own earlier read of
/// the row would already have failed with `NotFound` — measured
/// directly (`tests.rs`'s own `f03_...`; see this round's own package
/// for the probe). This check exists for the gap the cascade is itself
/// the primary defense against and does not cover — `PRAGMA
/// foreign_keys` disabled for a connection, a future write path that
/// bypasses it, or any other way the two tables' rows could drift out
/// of sync with each other — not for the scenario a correctly
/// FK-enforced delete already prevents entirely.
pub fn exists_within_tx(
    conn: &rusqlite::Connection,
    user_id: UserId,
    provider_id: FederationProviderId,
) -> StoreResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM federation_link WHERE user_id = ?1 AND provider_id = ?2",
            params![user_id.to_string(), provider_id.to_string()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// RFC 096-B2 S6, C18 §A: the exact count the delete confirmation states
/// in words ("N users will no longer be able to sign in") and the event
/// records (`federation.provider.deleted`'s `deleted_links` — A3: "the
/// event records that count"). Read before the delete that cascades
/// these rows away, in the same transaction.
pub fn count_for_provider_within_tx(
    conn: &rusqlite::Connection,
    provider_id: FederationProviderId,
) -> StoreResult<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM federation_link WHERE provider_id = ?1",
        [provider_id.to_string()],
        |r| r.get(0),
    )
    .map_err(StoreError::from)
}

/// The same count, for a caller outside any transaction — the delete
/// confirm page, which needs it to state "N users" before an
/// administrator has committed to anything (`handlers/admin/federation_
/// providers.rs`). A handler reaching for `Database::with_conn` directly
/// is exactly what `write-authority.toml`'s allow-list exists to catch;
/// this is the ordinary async repo function that boundary expects it to
/// call instead.
pub async fn count_for_provider(
    db: &Database,
    provider_id: FederationProviderId,
) -> StoreResult<i64> {
    db.with_conn(move |conn| count_for_provider_within_tx(conn, provider_id))
        .await
}

/// RFC 096-B2 S6, C20: delete one link inside the caller's own
/// transaction, so the pair's `federation_mfa_pending` rows cascade away
/// with it (migration 0047 `:152-154`) in the same commit as the event.
/// Unlike the standalone async [`delete`] above (its own `with_conn`,
/// its own implicit commit), this is composable into `class_a`'s
/// transaction. Returns the number of rows affected; the caller asserts
/// exactly 1 or treats 0 as `NotFound`.
pub fn delete_within_tx(
    conn: &rusqlite::Connection,
    user_id: UserId,
    provider_id: FederationProviderId,
) -> StoreResult<usize> {
    Ok(conn.execute(
        "DELETE FROM federation_link WHERE user_id = ?1 AND provider_id = ?2",
        params![user_id.to_string(), provider_id.to_string()],
    )?)
}

/// RFC 096-B2 S2a Part F: whether *any* user is linked to this provider
/// at all — `architecture.md:306-307`'s upgrade report needs a yes/no,
/// not which users. Used by the startup `requires_review` warning, not
/// on any request path.
pub async fn any_link_exists_for_provider(
    db: &Database,
    provider_id: FederationProviderId,
) -> StoreResult<bool> {
    let provider_id_str = provider_id.to_string();
    db.with_read(move |conn| {
        let mut stmt = conn.prepare("SELECT 1 FROM federation_link WHERE provider_id = ?1")?;
        match stmt.query_row([&provider_id_str], |_| Ok(())) {
            Ok(()) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(StoreError::from(e)),
        }
    })
    .await
}

/// Touch `last_seen_at`, on the caller's own transaction (RFC 096-B1
/// stage 10b, F03's own link observation/bookkeeping on `Promoted`).
/// `NotFound` if the link no longer exists — the same FK that makes
/// `federation_mfa_pending` rows cascade away with their own unlink
/// means this signals a genuine authority change, not a bookkeeping
/// miss. Does not touch `upstream_email`: F03 runs on a later request
/// than the one that carried fresh upstream claims (F02's), so there is
/// no new email to observe here, only the fact of this successful
/// sign-in.
pub fn touch_last_seen_within_tx(
    conn: &rusqlite::Connection,
    user_id: UserId,
    provider_id: FederationProviderId,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<()> {
    let n = conn.execute(
        "UPDATE federation_link SET last_seen_at = ?1 WHERE user_id = ?2 AND provider_id = ?3",
        params![now, user_id.to_string(), provider_id.to_string()],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
