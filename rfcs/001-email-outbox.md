# RFC 001 — Persistent email outbox + retry worker

**Status.** Proposed
**Tracks.** ROADMAP / Medium term — "Persistent email outbox + retry worker".
**Touches.** `sui-id-store` (new `email_outbox` table + repo), `sui-id-core`
(`mail` module gains an outbox-backed sender), `sui-id` (background worker
spawn alongside `gc::spawn`).

## Summary

Today (v0.22.0+) sui-id sends mail inline with the request that triggered
it: forgot-password, password-changed-notification, and any future templates
all `await` the SMTP `send` directly inside the handler. Failures land in
the audit log but the message itself is lost — there is no second attempt
and no operator-visible queue of pending sends.

This RFC adds a small persistent outbox: an `email_outbox` table, a
single in-process worker that drains it with exponential backoff, and a
new `OutboxMailSender` that wraps the existing `SmtpMailSender`. Handlers
stop awaiting `send` directly; they enqueue and return immediately. The
outbox sits in the same SQLite database as everything else — no new
runtime dependency, no separate process.

## Requirements

After this RFC ships:

1. A handler that triggers an email returns its HTTP response without
   waiting for SMTP. The user-visible latency of `/forgot-password`
   and password-change is unchanged whether SMTP is up, down, or slow.
2. A failed SMTP delivery is retried up to a bounded number of times
   with exponential backoff before being marked permanently failed.
3. Operators can see, in the admin audit log and (future) admin UI,
   which mails are queued, sending, sent, or failed.
4. Nothing in the outbox contains plaintext recipient addresses or
   message bodies in violation of the existing column-encryption
   posture: any field that today is encrypted on `forgot_password_tokens`
   stays encrypted in the outbox.
5. Restart of the sui-id process resumes work from the persistent
   outbox; in-flight rows return to `queued` state.
6. The opt-in stance from v0.22.0 is preserved: with no SMTP config,
   handlers continue to short-circuit and the outbox stays empty.

## Design

### Schema

Migration `0019_email_outbox.sql`:

```sql
CREATE TABLE email_outbox (
    id              TEXT PRIMARY KEY,            -- UUID v4
    state           TEXT NOT NULL                -- 'queued' | 'sending' | 'sent' | 'failed'
                    CHECK (state IN ('queued','sending','sent','failed')),
    template        TEXT NOT NULL,               -- stable identifier, e.g. 'forgot_password'
    recipient_enc   BLOB NOT NULL,               -- AAD-bound XChaCha20-Poly1305
    payload_enc     BLOB NOT NULL,               -- serialised template params, encrypted
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMP NOT NULL,          -- UTC; queued rows are eligible when now() >= this
    last_error      TEXT,                        -- last SMTP error, plain (no creds)
    created_at      TIMESTAMP NOT NULL,
    updated_at      TIMESTAMP NOT NULL
);

CREATE INDEX idx_email_outbox_eligible
    ON email_outbox (state, next_attempt_at)
    WHERE state = 'queued';
```

Two AADs: `email_outbox.recipient` and `email_outbox.payload`. Both
encrypted under the master key, same as every other sealed column.

The `template` column is a stable, lowercase, dotted identifier
(`forgot_password`, `password_changed_notification`, …). It maps
1-to-1 to a function in `core::mail::templates::*`. Adding a
template = adding a `match` arm in the worker.

### Repo

`crates/sui-id-store/src/repos/email_outbox.rs`:

```rust
pub fn enqueue(db: &Database, row: &EmailOutboxRow) -> StoreResult<EmailOutboxId>;
pub fn claim_one_eligible(db: &Database, now: DateTime<Utc>) -> StoreResult<Option<EmailOutboxRow>>;
pub fn mark_sending(db: &Database, id: EmailOutboxId, now: DateTime<Utc>) -> StoreResult<()>;
pub fn mark_sent(db: &Database, id: EmailOutboxId, now: DateTime<Utc>) -> StoreResult<()>;
pub fn record_failure(
    db: &Database,
    id: EmailOutboxId,
    error: &str,
    next_attempt: DateTime<Utc>,
    now: DateTime<Utc>,
) -> StoreResult<()>;
pub fn mark_permanently_failed(db: &Database, id: EmailOutboxId, now: DateTime<Utc>) -> StoreResult<()>;
pub fn requeue_stuck_sending(db: &Database, threshold: DateTime<Utc>) -> StoreResult<usize>;
```

`claim_one_eligible` does a `SELECT … WHERE state='queued' AND
next_attempt_at <= ?1 ORDER BY next_attempt_at LIMIT 1` followed by
an `UPDATE … SET state='sending'` in the same transaction. Single
worker per process, so no row-level lock dance is needed; the
state transition is the lock.

`requeue_stuck_sending` runs at startup and on a slow tick — anything
in `sending` for more than the worker's tick interval is presumed
crashed mid-send and gets reset to `queued`. SMTP is idempotent
enough at the recipient level that re-sending is acceptable; we
favour at-least-once.

### Worker

`crates/sui-id-core/src/mail/outbox_worker.rs`:

```rust
pub struct OutboxWorker {
    db:          Database,
    smtp:        Arc<SmtpMailSender>,
    clock:       SharedClock,
    tick:        Duration,            // default 5s when idle, immediate retry when work seen
    max_attempts: u32,                 // default 5
    backoff:     ExponentialBackoff,  // 30s, 2m, 10m, 1h, 6h
}

impl OutboxWorker {
    pub fn spawn(self) -> tokio::task::JoinHandle<()>;
}
```

The worker runs a single loop: claim one row, run the matching
template render against decrypted `payload_enc`, hand to
`SmtpMailSender::send`, mark sent or record-failure with a fresh
`next_attempt_at` derived from `attempt_count` and the backoff
schedule. After `max_attempts` it goes to `failed` permanently —
no automatic resurrection; an operator action would be required
to retry, and that's deliberately a v2 concern.

Spawned alongside `gc::spawn` from `sui-id/src/main.rs`'s
`serve` and `serve_dev`. Single instance — sui-id is a
single-binary deployment, so no leadership election. If a future
HA story arrives, that's when this becomes a hard problem.

### Handler integration

`core::mail::MailSender` stays the trait. A new
`OutboxMailSender` implements it: `send` enqueues a row and
returns `Ok(())` immediately. `AppState::new` wires this in
place of `SmtpMailSender` once `OutboxMailSender` exists; the
old direct-send path becomes the dev-mode default (no
persistence, no retry — keeps `--dev` self-contained).

```rust
impl MailSender for OutboxMailSender {
    fn send(&self, msg: &Mail) -> CoreResult<()> {
        let row = EmailOutboxRow::pending(msg, self.clock.now());
        sui_id_store::repos::email_outbox::enqueue(&self.db, &row)
            .map(|_| ())
            .map_err(CoreError::from)
    }
}
```

The trait method intentionally does not change. Existing call
sites (`handlers::forgot_password`, `core::me_security`, etc) do
not need to know they're now talking to a queue.

### Audit events

Existing `mail.send.success` / `mail.send.failure` move from
the request thread to the worker thread. Two new events:

- `mail.outbox.enqueued` — at the request thread, when a row
  is added.
- `mail.outbox.permanent_failure` — at the worker, after
  `max_attempts`.

### Configuration

Three new fields under `[security]` (the closest match in the
existing `Config` shape):

```toml
[security]
email_outbox_max_attempts        = 5     # 0 = no retry, fail immediately
email_outbox_initial_backoff_secs = 30
email_outbox_idle_tick_secs       = 5
```

All have defaults that match the schema above. `dev mode`
overrides them to `max_attempts=0` and uses the direct sender
to keep dev startup fast and offline.

## Multiple implementation steps

This work is small enough to land as a single release once the
design is firm. If a phased rollout is preferred:

1. **Schema + repo only.** Migration `0019` lands. No worker.
   Existing `SmtpMailSender` continues to send inline. Repo
   tests prove the table works. No user-visible change.
2. **Outbox sender + worker.** `OutboxMailSender`,
   `OutboxWorker`. `AppState::new` wires the outbox path for
   non-dev builds. Handlers' user-visible latency drops; mail
   delivery remains best-effort. Default config, no retry yet.
3. **Retry policy.** `attempt_count`, `next_attempt_at`,
   exponential backoff, permanent-failure event. This is the
   payoff release.

Each step is independently shippable.

## Tests

- **Repo unit tests.** `enqueue` round-trip; `claim_one_eligible`
  honours `next_attempt_at`; `record_failure` updates
  `attempt_count` and `next_attempt_at`; `requeue_stuck_sending`
  resets a sub-tick `sending` row.
- **Worker integration test** with a `MockClock` and an
  `InMemoryMailSender` substitute that fails the first N times,
  succeeds on the N+1th. Asserts the row hits `sent`, not `failed`,
  and that `attempt_count` matches.
- **End-to-end e2e.** Drive `/forgot-password`, observe the
  outbox row, advance the mock clock, assert the
  `InMemoryMailSender` saw the message exactly once.
- **Permanent failure path.** N+1 failures put the row in
  `failed` and emit `mail.outbox.permanent_failure`. Audit
  chain remains continuous.
- **Restart resumption.** `sui-id` shuts down with `sending`
  rows in flight; `requeue_stuck_sending` on startup resets
  them; the worker picks them back up.
- **No SMTP config = no outbox traffic.** Existing 404
  behaviour on `/forgot-password` still holds; nothing is
  enqueued.

## Security considerations

- **Recipient privacy.** `recipient_enc` is AAD-bound to the
  outbox table, never logged in plaintext. `last_error` is
  stored but goes through the existing SMTP error redactor
  (which strips credentials).
- **Replay.** A row in `sending` that the process crashed mid-flight
  may produce a duplicate at-most-twice, but every existing
  template is idempotent at the recipient level (reset link is
  single-use; password-change notice is informational). Design
  is at-least-once; templates compensate.
- **Capacity DoS.** A flood of forgot-password requests against
  a non-existent address could fill the outbox. The existing
  per-IP rate limit on `/forgot-password` covers the request
  side. Worker side: no upper bound on the queue is enforced
  here; if this becomes a real concern, a `LIMIT N` cap can be
  applied at `enqueue` time. Out of scope for this RFC.
- **Master-key rotation.** `email_outbox.recipient_enc` and
  `payload_enc` join the list of columns reseal'd by `admin
  rotate-key`. Add to the rotation harness in `core::key_rotation`.
- **Permanent-failure rows as evidence.** Stays in the table
  indefinitely, encrypted. An operator-driven cleanup CLI
  (`sui-id admin email-outbox prune --before TIMESTAMP`) can
  follow in a separate change; not in this RFC.

## Open questions

- Should `failed` rows be preserved indefinitely, or auto-pruned
  after a default retention window (e.g. 30 days)?
  Recommendation: preserve, let operators prune explicitly via
  a future CLI. No retention pressure given expected volume.
- Worker liveness probe? A `/_health/outbox` returning queue
  depth could be useful, but smells like the start of the
  Prometheus metrics work (RFC 006). Defer.
