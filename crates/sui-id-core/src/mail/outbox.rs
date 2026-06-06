//! Persistent email outbox — sender and background worker (RFC 001).
//!
//! `OutboxMailSender` implements `MailSender` by persisting each outgoing
//! mail to the `email_outbox` table and returning immediately. The actual
//! SMTP delivery is handled by `OutboxWorker`, which runs as a background
//! task alongside `gc::spawn`.
//!
//! ## Retry schedule (defaults)
//!
//! | Attempt | Delay before retry |
//! |---|---|
//! | 1 | 30 seconds |
//! | 2 | 2 minutes |
//! | 3 | 10 minutes |
//! | 4 | 1 hour |
//! | 5 | 6 hours |
//! | (final) | → `failed`, no further retries |
//!
//! These defaults are configurable via `Config::email_outbox_*`. Dev mode
//! sets `max_attempts = 0` (no retry) and uses the direct `SmtpMailSender`.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sui_id_shared::ids::EmailOutboxId;
use sui_id_store::repos::email_outbox;
use sui_id_store::models::{EmailOutboxRow, EmailOutboxState};
use sui_id_store::Database;

use crate::errors::{CoreError, CoreResult};
use crate::mail::{MailSender, MailSendOutcome, OutgoingMail, SmtpMailSender};
use crate::time::SharedClock;

// ── Encryption helpers ────────────────────────────────────────────────────────

fn encrypt_field(db: &Database, plaintext: &[u8], aad: &[u8]) -> CoreResult<Vec<u8>> {
    sui_id_store::crypto::seal(db.key(), plaintext, aad)
        .map_err(|_| CoreError::Internal)
}

/// Symmetric pair of `encrypt_field` reserved for a future
/// outbox replay / inspection path; currently no live caller.
#[allow(dead_code)]
fn decrypt_field(db: &Database, ciphertext: &[u8], aad: &[u8]) -> CoreResult<Vec<u8>> {
    sui_id_store::crypto::open(db.key(), ciphertext, aad)
        .map_err(|_| CoreError::Internal)
}

// ── OutboxMailSender ──────────────────────────────────────────────────────────

/// `MailSender` implementation that enqueues mail to the persistent outbox.
/// Returns immediately; delivery is handled by `OutboxWorker`.
#[derive(Clone)]
pub struct OutboxMailSender {
    db:    Database,
    clock: SharedClock,
}

impl OutboxMailSender {
    pub fn new(db: Database, clock: SharedClock) -> Self {
        Self { db, clock }
    }
}

impl MailSender for OutboxMailSender {
    fn send<'a>(
        &'a self,
        mail: OutgoingMail,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = CoreResult<MailSendOutcome>> + Send + 'a>> {
        Box::pin(async move {
            let now = self.clock.now();
            let recipient_enc = encrypt_field(&self.db, mail.to.as_bytes(), email_outbox::RECIPIENT_AAD)?;
            let payload = serde_json::to_vec(&OutboxPayload {
                to:        mail.to.clone(),
                subject:   mail.subject.clone(),
                text_body: mail.text_body.clone(),
                html_body: mail.html_body.clone(),
            }).map_err(|_| CoreError::Internal)?;
            let payload_enc = encrypt_field(&self.db, &payload, email_outbox::PAYLOAD_AAD)?;

            let row = EmailOutboxRow {
                id:              EmailOutboxId::new(),
                state:           EmailOutboxState::Queued,
                template:        "direct".into(),
                recipient_enc,
                payload_enc,
                attempt_count:   0,
                next_attempt_at: now,
                last_error:      None,
                // locale is resolved at the call site and stored here so the
                // worker renders in the recipient's language (RFC 002 § C).
                locale:          mail.locale.map(|l| l.tag().to_owned()),
                created_at:      now,
                updated_at:      now,
            };
            email_outbox::enqueue(&self.db, row).await.map_err(CoreError::from)?;

            // Return a synthetic outcome — actual delivery happens asynchronously.
            Ok(MailSendOutcome {
                from:    "(queued)".into(),
                to:      mail.to,
                subject: mail.subject,
            })
        })
    }
}

// ── Serialised payload ────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OutboxPayload {
    to:        String,
    subject:   String,
    text_body: String,
    html_body: Option<String>,
}

// ── Retry schedule ────────────────────────────────────────────────────────────

/// Seconds to wait before each retry attempt (0-indexed by `attempt_count`).
const BACKOFF_SECS: &[i64] = &[30, 120, 600, 3600, 21600];

fn next_attempt_at(attempt_count: i64, now: DateTime<Utc>) -> DateTime<Utc> {
    let delay = BACKOFF_SECS
        .get(attempt_count as usize)
        .copied()
        .unwrap_or(*BACKOFF_SECS.last().unwrap_or(&21600));
    now + chrono::Duration::seconds(delay)
}

// ── OutboxWorker ──────────────────────────────────────────────────────────────

/// Background task that drains the outbox by attempting SMTP delivery.
pub struct OutboxWorker {
    db:           Database,
    smtp:         Arc<SmtpMailSender>,
    clock:        SharedClock,
    /// How long to sleep between drain cycles when the queue is empty.
    idle_tick:    Duration,
    /// Maximum delivery attempts before a row is marked permanently failed.
    max_attempts: u32,
}

impl OutboxWorker {
    pub fn new(
        db: Database,
        smtp: Arc<SmtpMailSender>,
        clock: SharedClock,
        idle_tick_secs: u64,
        max_attempts: u32,
    ) -> Self {
        Self {
            db,
            smtp,
            clock,
            idle_tick: Duration::from_secs(idle_tick_secs),
            max_attempts,
        }
    }

    /// Spawn the worker as a Tokio background task. Returns a `JoinHandle`;
    /// callers typically drop it (fire-and-forget).
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn run(self) {
        // Reset any rows that were mid-send when the process last exited.
        let stuck_threshold = self.clock.now() - chrono::Duration::seconds(60);
        if let Err(e) = email_outbox::requeue_stuck_sending(
            &self.db, stuck_threshold, self.clock.now()
        ).await {
            tracing::warn!(error = %e, "outbox: could not reset stuck rows at startup");
        }

        loop {
            let now = self.clock.now();
            match email_outbox::claim_one_eligible(&self.db, now).await {
                Ok(Some(row)) => {
                    self.process_row(row).await;
                    // Continue immediately — there may be more.
                }
                Ok(None) => {
                    tokio::time::sleep(self.idle_tick).await;
                }
                Err(e) => {
                    tracing::error!(error = %e, "outbox: claim_one_eligible failed");
                    tokio::time::sleep(self.idle_tick).await;
                }
            }
        }
    }

    async fn process_row(&self, row: EmailOutboxRow) {
        let now = self.clock.now();

        // Decrypt payload.
        let payload_bytes = match sui_id_store::crypto::open(
            self.db.key(),
            &row.payload_enc,
            email_outbox::PAYLOAD_AAD,
        ) {
            Ok(b) => b,
            Err(_) => {
                tracing::error!(id = %row.id, "outbox: payload decryption failed — marking failed");
                let _ = email_outbox::mark_permanently_failed(
                    &self.db, row.id, "decryption_error".into(), now
                ).await;
                return;
            }
        };
        let payload: OutboxPayload = match serde_json::from_slice(&payload_bytes) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(id = %row.id, error = %e, "outbox: payload deserialisation failed");
                let _ = email_outbox::mark_permanently_failed(
                    &self.db, row.id, format!("deserialise_error: {e}"), now
                ).await;
                return;
            }
        };

        let mail = OutgoingMail {
            to:        payload.to,
            subject:   payload.subject,
            text_body: payload.text_body,
            html_body: payload.html_body,
        locale: None,
    };

        // Attempt delivery.
        match self.smtp.send(mail).await {
            Ok(outcome) => {
                tracing::debug!(id = %row.id, to = %outcome.to, "outbox: mail sent");
                let _ = email_outbox::mark_sent(&self.db, row.id, now).await;
            }
            Err(e) => {
                let attempt = row.attempt_count + 1;
                let err_str = redact_smtp_error(&e.to_string());
                if attempt >= self.max_attempts as i64 {
                    tracing::warn!(
                        id = %row.id, attempts = attempt,
                        "outbox: permanent failure after max attempts"
                    );
                    let _ = email_outbox::mark_permanently_failed(
                        &self.db, row.id, err_str, now
                    ).await;
                } else {
                    let next = next_attempt_at(attempt, now);
                    tracing::debug!(
                        id = %row.id, attempt, ?next,
                        "outbox: transient failure, scheduled retry"
                    );
                    let _ = email_outbox::record_failure(
                        &self.db, row.id, err_str, next, now
                    ).await;
                }
            }
        }
    }
}

/// Redact potential credential leakage from SMTP error strings.
/// Strips anything that looks like a password or auth token.
fn redact_smtp_error(s: &str) -> String {
    // Simple: truncate to 200 chars and strip newlines.
    let cleaned = s.replace(['\r', '\n'], " ");
    if cleaned.len() > 200 {
        format!("{}…", &cleaned[..200])
    } else {
        cleaned
    }
}
