# RFC 013 — Reduce blocking impact of synchronous SQLite on async handlers

**Status.** Exploratory
**Priority.** Medium. Performance / scalability ceiling fix. No
correctness gap today; the cost is concurrency under load.
**Tracks.** v0.29.3 codebase review — medium-priority finding #5.
**Touches.** `sui-id-store::Database` (the connection-handling
shape), every repo function (signature change), `sui-id-core`
(call sites become async), `sui-id` (handlers — mostly already
async, but the await sites multiply).

## Summary

`sui-id-store` today wraps a single `rusqlite::Connection` in a
`std::sync::Mutex`. Every repo call acquires the mutex
synchronously — including every call made from inside an Axum
async handler. Holding `std::sync::Mutex` across an `await`
point is sound (we don't await while holding it), but every
holder is a Tokio worker thread that, while inside the lock, is
effectively running synchronous SQLite I/O on a worker that
should be free to do async work.

For sui-id's stated audience — small self-hosted deployments
running on a single instance — this is fine. The review's point
is that the *ceiling* under load is set by this design choice, and
that the choice should be either (a) made explicit as a deliberate
trade-off, or (b) lifted by moving DB work off the async runtime.

This RFC proposes (b), in a shape that keeps the SQLite-default
deployment story intact and doesn't pull `sqlx` (RFC 009) forward
prematurely.

## Why this is medium-priority, not high

- sui-id has no correctness bug here. Sessions resolve correctly,
  audit chains stay continuous, transactions commit atomically.
- sui-id has no measured performance problem here either; the
  review notes that no benchmarks exist and SLOs are unspecified.
- The risk is operational: a deployment that grows past its
  expected load discovers the ceiling at the wrong time.

The change should land *before* RFC 009 (multi-backend SQL
support) because RFC 009 would also touch the connection-handling
shape, and doing it twice is worse than doing it once. Sequence:
RFC 013 first, RFC 009 builds on top.

## Requirements

After this RFC ships:

1. No HTTP handler, on the happy path, holds a synchronous
   `Mutex` while doing DB I/O. Tokio workers are free to make
   progress on other requests during a slow query.
2. The repo trait surface in `sui-id-store::repos::*` is async.
   Call sites become `.await`-ed.
3. The single-binary deployment shape is preserved. No new
   process, no new sidecar, no required runtime dependency
   beyond the already-present `tokio`.
4. SQLite remains the default storage. The change is about
   *how we call into rusqlite*, not about replacing it.
5. The `--dev` mode still works without a network or extra
   setup.
6. Existing tests pass without behavioural changes. The
   integration-test binary continues to use in-memory SQLite
   and run on the default `cargo test` invocation.

## Design

### Three reasonable shapes

#### Option A — `tokio::task::spawn_blocking` at every call site

Every repo function becomes `async fn`, and its body wraps the
existing synchronous code in `spawn_blocking`. The connection
mutex still exists; it's just acquired on a blocking-thread-pool
worker instead of a Tokio runtime worker.

```rust
pub async fn get_user(db: &Database, user_id: UserId) -> StoreResult<User> {
    let db = db.clone(); // Database is Arc-shaped
    tokio::task::spawn_blocking(move || {
        let conn = db.lock();
        // existing synchronous body
    })
    .await
    .map_err(StoreError::JoinError)?
}
```

Pros: minimal mechanical change, every existing test keeps its
current shape, easy to land incrementally (function-by-function).
Cons: every DB call pays a `spawn_blocking` overhead (~few
microseconds), and the blocking pool is shared with other Tokio
blocking work.

#### Option B — Dedicated DB executor thread

A single OS thread owns the rusqlite Connection. Repo calls send
a request enum down a `tokio::sync::mpsc::Sender`, the executor
runs the query synchronously (no mutex contention — there's only
one consumer), and returns the result over a oneshot channel.

```rust
pub struct Database {
    requests: mpsc::Sender<DbRequest>,
}

enum DbRequest {
    GetUser { user_id: UserId, reply: oneshot::Sender<StoreResult<User>> },
    // … one variant per repo function
}
```

Pros: zero mutex contention by construction. Single-threaded
SQLite (the recommended posture for SQLite anyway). Easy to
reason about: serialised access, no lock-ordering surprises.
Cons: enum-with-N-variants is annoying to maintain. Every new
repo function needs a variant + a match arm in the executor.
Boilerplate scales with the API.

#### Option C — Connection pool via `r2d2_sqlite`, accessed via `spawn_blocking`

`r2d2_sqlite` gives a small pool of rusqlite Connections.
`spawn_blocking` wraps each call. Concurrency goes up because
multiple readers can be in flight; SQLite's WAL mode makes this
safe.

```rust
pub struct Database {
    pool: r2d2::Pool<SqliteConnectionManager>,
}

pub async fn get_user(db: &Database, user_id: UserId) -> StoreResult<User> {
    let pool = db.pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        // existing synchronous body
    })
    .await?
}
```

Pros: real concurrency for read-heavy workloads. Standard pattern
for sync-DB-from-async-Rust. New dependency is small.
Cons: SQLite write contention still serialises (single-writer at
the SQL level). The pool needs sizing. WAL mode must be enabled
explicitly.

### Recommendation

**Option A first**, **then revisit** based on benchmark data.
Reasoning:

- Option A is the smallest mechanical change that lifts the
  immediate ceiling. Tokio workers stop blocking on
  long-running queries; the blocking pool absorbs them.
- The cost (`spawn_blocking` overhead) is small relative to
  any real query. For a single-row indexed lookup,
  `spawn_blocking` adds maybe 1–5µs; the SQLite query itself
  takes more.
- Once benchmarks (introduced in RFC 014) exist, we have data
  to decide whether Option B or Option C is worth the
  additional work. Until then, "let Tokio breathe" is the
  high-leverage change.

The RFC is written as Option A; Option B/C remain as
follow-ups gated on benchmark evidence.

### The mechanical migration

Every function in `crates/sui-id-store/src/repos/*.rs`:

1. Add `async` to the signature.
2. Wrap the body in `tokio::task::spawn_blocking(move || { … })`.
3. `.await` the result; map the `JoinError` into a new
   `StoreError::JoinError` variant.

Every call site in `sui-id-core::*` and the handlers:

1. Add `.await` to the repo call.
2. If the calling function is not `async fn`, propagate
   asyncness up — most are already async because they're
   called from handler code.

The transaction helper `db.transaction(|tx| { … })` becomes
`db.transaction(|tx| async move { … })` or stays sync inside
a single `spawn_blocking` (recommended — keeps the closure's
body fully synchronous, which matches rusqlite's API).

### Database shape change

`Database` is currently:

```rust
pub struct Database {
    inner: Arc<Mutex<Connection>>,
}
```

Becomes:

```rust
pub struct Database {
    inner: Arc<Mutex<Connection>>,  // unchanged shape
}
```

No structural change. The async-ness is at the *call site*, not
at the struct. The mutex is now held only inside a
`spawn_blocking` closure, so it's a `parking_lot::Mutex` or even
a `std::sync::Mutex` without any cross-await holding concern.

### Backwards compatibility for tests

Test fixtures that call repo functions directly today (e.g.
`tests/e2e/common.rs`'s `enable_smtp`) become `async`. The
existing tests are mostly already `#[tokio::test]`, so the
migration is `.await` insertion. A few synchronous test helpers
in `crates/sui-id-store/src/repos/*/tests.rs` need a `tokio`
runtime; the typical pattern is `tokio::runtime::Runtime::new()`
in the test body or move the test to `#[tokio::test]`.

## Multiple implementation steps

The work is bigger than it looks because every repo function and
every call site changes. Breakdown:

1. **Foundation.** `StoreError::JoinError` variant.
   `Database::transaction` adapted (still sync inside, but the
   public `async` wrapper). One example repo function migrated
   end-to-end (recommend `users::get_user`) as the pattern
   reference. Tests still pass.
2. **Migrate `repos::users` and `repos::credentials`.** The
   highest-traffic tables. This step ripples into
   `sui-id-core::session`, which is the largest call-site
   blast radius. Land once that compiles and tests pass.
3. **Migrate the rest of `repos::*`.** Mechanical, one repo
   module per commit if helpful for review. Audit chain,
   sessions, refresh tokens, signing keys, MFA, SMTP config,
   server settings, password reset, WebAuthn pending, audit.
4. **Migrate handlers.** Mostly `.await` insertion. The
   integration tests in `tests/e2e/*` get the same treatment.
5. **Verify.** Full test suite green; observe blocking-pool
   utilisation under a synthetic load (RFC 014 benchmarks
   come in handy here).

Each step lands as its own commit / patch. The intermediate
states compile and pass tests — the migration is incremental.

## Tests

- All existing tests must continue to pass with no
  behavioural change. This is the primary acceptance signal.
- A new integration test in `tests/e2e/concurrency.rs` (new
  module) drives N concurrent `/admin/login` attempts and
  asserts that none of them produce errors and that the
  Tokio runtime did not stall (heuristic: total wall-clock
  time is under a generous threshold). This won't detect a
  regression precisely but will catch a catastrophic one.
- A `loom`-style or property-test pass over the transaction
  boundary is *not* required. The transaction code path
  doesn't change semantically; the change is at the
  threading boundary, not the consistency boundary.

## Security considerations

- **No new attack surface.** The change is internal threading,
  not an API change. All authorisation, all input validation,
  all rate limiting are upstream of the DB layer and unaffected.
- **No new race conditions.** A `spawn_blocking` task runs to
  completion before its result is observed. The transaction
  boundary continues to be SQLite's atomic commit.
- **DoS posture unchanged.** Blocking-pool exhaustion would
  manifest as request queueing, the same as
  Tokio-worker-blocking does today. Existing rate limits cap
  the inflow either way.
- **Audit chain integrity.** Audit appends still happen inside
  a transaction. Concurrent appends from two requests serialise
  on the same write lock (because SQLite serialises writes).
  No new chain-integrity concern.

## Open questions

- **Should the blocking-pool size be tuned?** Tokio's default is
  reasonable for a single-instance deployment. If we ever see
  blocking-pool starvation in the benchmark work (RFC 014),
  revisit then.
- **Should we adopt `parking_lot::Mutex` for the inner mutex?**
  Probably yes — it's smaller and faster than std's. Trivial
  swap. Folded into the migration.
- **Should `Database::transaction`'s closure be `async`?**
  Recommend no. Keeping the closure body fully synchronous
  inside a single `spawn_blocking` matches rusqlite's API and
  avoids the awkward "is it OK to await inside a transaction"
  question.
- **When do we revisit Option B or C?** When (a) benchmarks
  exist (RFC 014 lands), and (b) measured contention shows
  Tokio worker availability degrading under load that the
  deployment posture is meant to support. Until both, this RFC
  is the change.
