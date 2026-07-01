//! Repository functions for the `email_outbox` table (RFC 001).
//!
//! The outbox is a persistent queue for outgoing mail. Rows are written
//! by `OutboxMailSender::send` on the request thread and drained by the
//! `OutboxWorker` background task. The worker claims one row at a time,
//! attempts delivery, and either marks the row `sent` or schedules a
//! retry with exponential backoff.

use chrono::{DateTime, Utc};
use sui_id_shared::ids::EmailOutboxId;

use rusqlite::OptionalExtension;

use crate::models::{EmailOutboxRow, EmailOutboxState};
use crate::{Database, StoreResult};

/// AAD tag for the encrypted recipient address column.
pub const RECIPIENT_AAD: &[u8] = b"email_outbox.recipient";
/// AAD tag for the encrypted payload column.
pub const PAYLOAD_AAD: &[u8] = b"email_outbox.payload";

// ── private mapper ────────────────────────────────────────────────────────────

fn map(row: &rusqlite::Row<'_>) -> rusqlite::Result<EmailOutboxRow> {
    let state_str: String = row.get(1)?;
    let state = state_str.parse::<EmailOutboxState>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(EmailOutboxRow {
        id: row.get::<_, String>(0)?.parse().map_err(|e: uuid::Error| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        state,
        template: row.get(2)?,
        recipient_enc: row.get(3)?,
        payload_enc: row.get(4)?,
        attempt_count: row.get(5)?,
        next_attempt_at: row.get(6)?,
        last_error: row.get(7)?,
        locale: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

// ── public API ────────────────────────────────────────────────────────────────

/// Persist a new outbox row in `queued` state. Returns the row's id.
pub async fn enqueue(db: &Database, row: EmailOutboxRow) -> StoreResult<EmailOutboxId> {
    // RFC 006: track outbox enqueue volume.
    if let Some(m) = crate::global_metrics() {
        m.email_outbox_enqueued();
    }
    let id = row.id;
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO email_outbox \
             (id, state, template, recipient_enc, payload_enc, attempt_count, \
              next_attempt_at, last_error, locale, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                row.id.to_string(),
                EmailOutboxState::Queued.as_str(),
                row.template,
                row.recipient_enc,
                row.payload_enc,
                0_i64,
                row.next_attempt_at,
                Option::<String>::None,
                row.locale,
                row.created_at,
                row.updated_at,
            ],
        )?;
        Ok(())
    })
    .await?;
    Ok(id)
}

/// Claim one eligible queued row (next_attempt_at ≤ now) and mark it
/// `sending`. Returns `None` if the queue is empty or not yet due.
pub async fn claim_one_eligible(
    db: &Database,
    now: DateTime<Utc>,
) -> StoreResult<Option<EmailOutboxRow>> {
    db.with_conn(move |conn| {
        let tx = conn.unchecked_transaction()?;
        let maybe_row: Option<EmailOutboxRow> = tx
            .query_row(
                "SELECT id, state, template, recipient_enc, payload_enc, \
                        attempt_count, next_attempt_at, last_error, locale, created_at, updated_at \
                 FROM email_outbox \
                 WHERE state = 'queued' AND next_attempt_at <= ?1 \
                 ORDER BY next_attempt_at ASC LIMIT 1",
                [now],
                map,
            )
            .optional()?;
        let claimed = if let Some(mut r) = maybe_row {
            tx.execute(
                "UPDATE email_outbox SET state = 'sending', updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, r.id.to_string()],
            )?;
            // Reflect the new state in the returned row.
            r.state = EmailOutboxState::Sending;
            r.updated_at = now;
            Some(r)
        } else {
            None
        };
        tx.commit()?;
        Ok(claimed)
    })
    .await
}

/// Mark a `sending` row as successfully sent.
pub async fn mark_sent(db: &Database, id: EmailOutboxId, now: DateTime<Utc>) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE email_outbox SET state = 'sent', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Record a transient delivery failure; schedule a retry at `next_attempt_at`.
pub async fn record_failure(
    db: &Database,
    id: EmailOutboxId,
    error: String,
    next_attempt_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE email_outbox \
             SET state = 'queued', attempt_count = attempt_count + 1, \
                 last_error = ?1, next_attempt_at = ?2, updated_at = ?3 \
             WHERE id = ?4",
            rusqlite::params![error, next_attempt_at, now, id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Mark a row permanently failed after exhausting all retry attempts.
pub async fn mark_permanently_failed(
    db: &Database,
    id: EmailOutboxId,
    error: String,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE email_outbox \
             SET state = 'failed', attempt_count = attempt_count + 1, \
                 last_error = ?1, updated_at = ?2 \
             WHERE id = ?3",
            rusqlite::params![error, now, id.to_string()],
        )?;
        Ok(())
    })
    .await
}

/// Reset stuck `sending` rows to `queued` at startup or on a slow tick.
/// A row is considered stuck if it has been in `sending` since before
/// `threshold`. Returns the number of rows reset.
pub async fn requeue_stuck_sending(
    db: &Database,
    threshold: DateTime<Utc>,
    now: DateTime<Utc>,
) -> StoreResult<usize> {
    db.with_conn(move |conn| {
        let n = conn.execute(
            "UPDATE email_outbox \
             SET state = 'queued', next_attempt_at = ?1, updated_at = ?2 \
             WHERE state = 'sending' AND updated_at < ?3",
            rusqlite::params![now, now, threshold],
        )?;
        Ok(n)
    })
    .await
}

/// Re-seal `recipient_enc` and `payload_enc` with a new master key.
/// Called by the key-rotation harness.
pub fn reseal_all(
    tx: &rusqlite::Transaction<'_>,
    old_key: &crate::crypto::MasterKey,
    new_key: &crate::crypto::MasterKey,
) -> StoreResult<usize> {
    use crate::crypto::{open, seal};
    let rows = {
        let mut stmt = tx.prepare("SELECT id, recipient_enc, payload_enc FROM email_outbox")?;
        let rows: Vec<(String, Vec<u8>, Vec<u8>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };
    let n = rows.len();
    for (id, rec_enc, pay_enc) in rows {
        let rec_plain = open(old_key, &rec_enc, RECIPIENT_AAD)
            .map_err(|_| crate::errors::StoreError::Crypto)?;
        let pay_plain =
            open(old_key, &pay_enc, PAYLOAD_AAD).map_err(|_| crate::errors::StoreError::Crypto)?;
        let new_rec = seal(new_key, &rec_plain, RECIPIENT_AAD)
            .map_err(|_| crate::errors::StoreError::Crypto)?;
        let new_pay = seal(new_key, &pay_plain, PAYLOAD_AAD)
            .map_err(|_| crate::errors::StoreError::Crypto)?;
        tx.execute(
            "UPDATE email_outbox SET recipient_enc = ?1, payload_enc = ?2 WHERE id = ?3",
            rusqlite::params![new_rec, new_pay, id],
        )?;
    }
    Ok(n)
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// RFC 073: Count rows that have been pending in `queued` state for
/// longer than `stuck_threshold`. Used by the dashboard to surface
/// "outbox stuck" warnings — typically when SMTP credentials are wrong
/// or the SMTP host is unreachable.
pub async fn count_stuck_pending(
    db: &Database,
    stuck_threshold: chrono::Duration,
    now: chrono::DateTime<chrono::Utc>,
) -> StoreResult<usize> {
    let cutoff = (now - stuck_threshold).to_rfc3339();
    db.with_conn(move |conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM email_outbox \
             WHERE state = 'queued' AND created_at < ?1",
            rusqlite::params![cutoff],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    })
    .await
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_copy)]
    use super::*;
    use crate::Database;
    use chrono::Utc;

    fn fresh_db() -> Database {
        let key = crate::crypto::MasterKey::generate();
        Database::open_in_memory(key).expect("in-memory db")
    }

    fn sample_row() -> EmailOutboxRow {
        let now = Utc::now();
        EmailOutboxRow {
            id: EmailOutboxId::new(),
            state: EmailOutboxState::Queued,
            template: "forgot_password".into(),
            recipient_enc: vec![0u8; 32],
            payload_enc: vec![1u8; 64],
            attempt_count: 0,
            next_attempt_at: now,
            last_error: None,
            locale: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn enqueue_and_claim_round_trip() {
        let db = fresh_db();
        let row = sample_row();
        let id = enqueue(&db, row.clone()).await.expect("enqueue");
        assert_eq!(id, row.id);

        let claimed = claim_one_eligible(&db, Utc::now())
            .await
            .expect("claim")
            .expect("some");
        assert_eq!(claimed.id, id);
        assert_eq!(claimed.state, EmailOutboxState::Sending);
    }

    #[tokio::test]
    async fn claim_respects_next_attempt_at() {
        let db = fresh_db();
        let mut row = sample_row();
        // Schedule 1 hour in the future
        row.next_attempt_at = Utc::now() + chrono::Duration::hours(1);
        enqueue(&db, row).await.expect("enqueue");

        let claimed = claim_one_eligible(&db, Utc::now()).await.expect("claim");
        assert!(claimed.is_none(), "should not be eligible yet");
    }

    #[tokio::test]
    async fn mark_sent_after_claim() {
        let db = fresh_db();
        let row = sample_row();
        let id = enqueue(&db, row).await.expect("enqueue");
        claim_one_eligible(&db, Utc::now())
            .await
            .expect("claim")
            .expect("some");
        mark_sent(&db, id.clone(), Utc::now())
            .await
            .expect("mark sent");

        // Should not appear as eligible anymore
        let claimed2 = claim_one_eligible(&db, Utc::now()).await.expect("claim2");
        assert!(claimed2.is_none(), "sent rows must not be re-claimed");
    }

    #[tokio::test]
    async fn record_failure_increments_attempt_count() {
        let db = fresh_db();
        let row = sample_row();
        let id = enqueue(&db, row).await.expect("enqueue");
        claim_one_eligible(&db, Utc::now())
            .await
            .expect("claim")
            .expect("some");

        let next_try = Utc::now() + chrono::Duration::seconds(30);
        record_failure(
            &db,
            id.clone(),
            "connection refused".into(),
            next_try,
            Utc::now(),
        )
        .await
        .expect("record failure");

        // Advance to after next_try
        let claimed2 = claim_one_eligible(&db, next_try + chrono::Duration::seconds(1))
            .await
            .expect("claim2")
            .expect("some");
        assert_eq!(claimed2.attempt_count, 1);
    }

    #[tokio::test]
    async fn requeue_stuck_sending_resets_old_rows() {
        let db = fresh_db();
        let row = sample_row();
        enqueue(&db, row).await.expect("enqueue");
        claim_one_eligible(&db, Utc::now())
            .await
            .expect("claim")
            .expect("some");
        // Threshold is in the future: any sending row older than it gets reset.
        let threshold = Utc::now() + chrono::Duration::seconds(1);
        let n = requeue_stuck_sending(&db, threshold, Utc::now())
            .await
            .expect("requeue");
        assert_eq!(n, 1);
        // Now it should be claimable again
        let claimed = claim_one_eligible(&db, Utc::now()).await.expect("claim2");
        assert!(claimed.is_some());
    }
}
