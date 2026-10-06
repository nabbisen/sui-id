//! Repository functions for the `forgot_password_requests` table (RFC 124 D1).
//!
//! The `/forgot-password` request path performs no classification: it
//! records that a recovery was requested for the submitted address,
//! unconditionally and without looking anything up, then responds. A
//! background worker (`sui_id_core::forgot_password::ForgotPasswordWorker`
//! — it lives in `sui-id-core`, not here, since it calls `request_reset`)
//! claims pending rows and performs everything that used to run inline.
//!
//! A row recorded here and not yet processed when the process restarts is
//! not lost: it is still `pending` (or reset to `pending` from
//! `processing` by the worker's startup sweep) when the worker starts
//! again. That is the property a detached `tokio::spawn`ed task would not
//! have had.

use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use sui_id_shared::ids::ForgotPasswordRequestId;

use crate::{Database, StoreResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgotPasswordRequestRow {
    pub id: ForgotPasswordRequestId,
    pub email: String,
    pub requester_ip: Option<String>,
    pub created_at: DateTime<Utc>,
}

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<ForgotPasswordRequestRow> {
    Ok(ForgotPasswordRequestRow {
        id: row.get::<_, String>(0)?.parse().map_err(|e: uuid::Error| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        email: row.get(1)?,
        requester_ip: row.get(2)?,
        created_at: row.get(3)?,
    })
}

/// Durably record that a recovery was requested for `email`. Unconditional:
/// no lookup, no branch on whether the address belongs to an account — the
/// whole point of RFC 124 D1 is that this insert costs the same regardless
/// of what `email` is.
pub async fn record(
    db: &Database,
    id: ForgotPasswordRequestId,
    email: String,
    requester_ip: Option<String>,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO forgot_password_requests \
             (id, state, email, requester_ip, created_at, updated_at) \
             VALUES (?1, 'pending', ?2, ?3, ?4, ?4)",
            rusqlite::params![id.to_string(), email, requester_ip, now],
        )?;
        Ok(())
    })
    .await
}

/// Claim the oldest pending row and mark it `processing`. `None` if the
/// queue is empty.
pub async fn claim_one(
    db: &Database,
    now: DateTime<Utc>,
) -> StoreResult<Option<ForgotPasswordRequestRow>> {
    db.with_conn(move |conn| {
        let tx = conn.unchecked_transaction()?;
        let maybe_row: Option<ForgotPasswordRequestRow> = tx
            .query_row(
                "SELECT id, email, requester_ip, created_at FROM forgot_password_requests \
                 WHERE state = 'pending' ORDER BY created_at ASC LIMIT 1",
                [],
                map,
            )
            .optional()?;
        if let Some(ref r) = maybe_row {
            tx.execute(
                "UPDATE forgot_password_requests SET state = 'processing', updated_at = ?1 \
                 WHERE id = ?2",
                rusqlite::params![now, r.id.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(maybe_row)
    })
    .await
}

/// Delete a row once it has been fully processed.
pub async fn delete(db: &Database, id: ForgotPasswordRequestId) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "DELETE FROM forgot_password_requests WHERE id = ?1",
            [id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Reset rows stuck `processing` back to `pending` at worker startup. A row
/// left `processing` means the previous process died mid-work, not that
/// the work happened — at-least-once, matching the outbox worker's own
/// `requeue_stuck_sending`. Returns the number of rows reset.
pub async fn requeue_stuck_processing(db: &Database, now: DateTime<Utc>) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        Ok(conn.execute(
            "UPDATE forgot_password_requests SET state = 'pending', updated_at = ?1 \
             WHERE state = 'processing'",
            rusqlite::params![now],
        )?)
    })
    .await
}

/// Count rows not yet fully processed (`pending` or `processing`). Used by
/// tests, including the restart proof: a count that stays nonzero across a
/// simulated restart, until the worker actually drains it, is the whole
/// property RFC 124 D1's decision was chosen for.
pub async fn count_outstanding(db: &Database) -> StoreResult<i64> {
    db.with_conn(|conn| {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM forgot_password_requests WHERE state IN ('pending', 'processing')",
            [],
            |r| r.get(0),
        )?)
    })
    .await
}

#[cfg(test)]
mod tests;
