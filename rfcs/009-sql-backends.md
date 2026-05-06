# RFC 009 — Pluggable SQL backends (PostgreSQL, MariaDB/MySQL)

**Status.** Exploratory
**Tracks.** ROADMAP / Longer term — "Pluggable SQL backends".
**Touches.** `sui-id-store` (a wide rewrite — repository
implementations and the migration runner), `sui-id` (config schema
gains a `[storage] driver` field; CLI subcommands `backup` /
`restore` / `verify-backup` learn to dispatch), `Cargo.toml`
(new dependency: most likely `sqlx` with selectable feature flags),
docs (deployment guidance, "which backend should I pick" essay).

## Summary

sui-id today persists everything to a single SQLite database
file. That choice is the right default — it ships zero ops surface,
backs up by `cp`, and runs offline. But it forecloses on
deployments that already operate a Postgres or MariaDB cluster
and would rather not introduce a new SQLite file alongside.

This RFC proposes adding *optional* PostgreSQL and MariaDB/MySQL
backends behind a config flag, so an operator can choose the
storage at deploy time. SQLite remains the default, the
single-binary distribution stays single-binary, and nothing in
sui-id's "minimum operating ceremony" stance changes for the
existing user base.

## Background — why this is hard, not just tedious

The SQLite-shaped assumptions in `sui-id-store` aren't all
visible at the call sites:

- **Column-encryption AAD.** XChaCha20-Poly1305 with an
  AAD scheme that's been validated against SQLite's
  byte-exact storage. PostgreSQL and MariaDB store `BLOB` /
  `BYTEA` / `VARBINARY` with their own conventions — the
  AAD computation must remain bit-identical across backends
  or every encrypted column becomes unreadable on migration.
- **Migrations.** sui-id has 18+ forward-only migrations
  written as raw SQLite SQL. The migration runner today
  expects SQLite syntax. Three syntaxes (`AUTOINCREMENT` /
  `SERIAL` / `AUTO_INCREMENT`, datetime types, partial
  indexes, `WITHOUT ROWID`) don't translate cleanly.
- **`sui-id admin rotate-key`.** The master-key rotation CLI
  reseals every encrypted column inside one SQLite
  transaction. The same atomicity guarantee on PostgreSQL
  and MariaDB is straightforward (both have transactions),
  but the mechanics of "reseal then update one row at a
  time across N tables" need to be backend-agnostic.
- **Backup / restore.** `sui-id backup` does a hot SQLite
  snapshot. PostgreSQL and MariaDB have entirely different
  backup primitives (`pg_dump`, `mysqldump`, or just filesystem
  snapshots of the data directory). The CLI either dispatches
  to those tools or refuses to handle backup for non-SQLite
  backends and points the operator at native tools.
- **Refresh-token theft detection, audit hash chain, lockout
  counters.** All read-modify-write under a transaction. Need
  to confirm isolation level expectations match across
  backends; PostgreSQL's default is `READ COMMITTED`, MariaDB's
  is `REPEATABLE READ`, SQLite's is effectively `SERIALIZABLE`
  via its locking model. The current code assumes the strongest
  guarantees implicitly.

The mechanical work — query rewriting, type mapping — is
large but tractable. The semantic work above is what keeps
this in `Exploratory`.

## Requirements (if implemented)

1. Storage backend is selected at deploy time via a single
   config field (`[storage] driver = "sqlite" | "postgres" |
   "mariadb"`). Default remains `sqlite`. Existing single-binary
   single-file deployments need zero changes.
2. The trait surface in `sui-id-store::repos::*` is identical
   across backends. `sui-id-core` and `sui-id` do not learn
   anything about the backend choice; they call the same repo
   functions either way.
3. The encrypted-column posture is identical: same AAD
   computation, same XChaCha20-Poly1305 parameters, same
   ciphertext byte-for-byte for the same plaintext-and-key
   regardless of backend. A column sealed under SQLite must
   be unsealable under Postgres if the row is dump-restored.
4. Migrations work on all three backends. Either a single
   SQL file per migration that all three accept (preferred,
   if feasible) or per-backend SQL files keyed by driver
   (acceptable for irreducible syntax differences).
5. The single-binary distribution stays single-binary. The
   Postgres and MariaDB drivers are compile-time feature
   flags that ship in the public release binary; an operator
   on SQLite carries the runtime weight of the unused
   drivers (small) but never has to install separate
   binaries.
6. `sui-id backup` / `restore` / `verify-backup`:
   - For SQLite, behaviour unchanged.
   - For Postgres/MariaDB, the CLI shells out to `pg_dump` /
     `mysqldump` (or refuses with a clear pointer to the
     native tool, depending on tradeoff — see Design).
7. `sui-id admin rotate-key` works on all three backends with
   the same atomicity guarantee.
8. The default deployment story in the README and quickstart
   doesn't change. SQLite stays the recommendation for
   anyone who doesn't already operate a database server.

## Design

### Library choice

Three reasonable shapes, in increasing order of weight:

#### Option A: `sqlx` with multi-driver features

Use `sqlx` with `sqlite`, `postgres`, `mysql` features. `sqlx`
already has a multi-backend story; query macros validate at
compile time against a *single* backend, but `query()` /
`query_as!()` (non-macro forms) work across drivers if SQL is
written portably.

Pros:
- One dependency, well-maintained, widely adopted.
- Connection pool, async, nullable-handling story all in one.
- Migration framework (`sqlx::migrate!()`) supports per-driver
  migration directories out of the box.

Cons:
- Bigger compile-time cost than `rusqlite` alone.
- Query macros lose compile-time validation if we want a
  single source query that works across all drivers.
- Async-first; sui-id-store today is mostly synchronous via
  `rusqlite`. A move to async ripples upward through repo
  signatures.

#### Option B: Hand-rolled trait abstraction

Define a `Backend` trait in `sui-id-store`, implement it
three times (`SqliteBackend` over `rusqlite`, `PostgresBackend`
over `tokio-postgres` or `postgres`, `MariadbBackend` over
`mysql` or `mysql_async`).

Pros:
- Maximum control, can keep `rusqlite`'s sync API for the
  SQLite path and only the new backends pay for async.
- No `sqlx` macro dependency.

Cons:
- We re-invent connection pooling, prepared statements,
  null handling. Months of work.
- Three driver crates to vendor and audit.
- Heterogeneous error types — every repo function gains
  a `BackendError` translation step.

#### Option C: `sqlx` plus a thin `Backend` shim

Use `sqlx` for the actual database I/O but wrap it in a
sui-id-internal `Backend` trait so that we can swap to
hand-rolled if `sqlx`'s direction ever stops aligning with
ours. The shim is small (connect, execute, query, transaction
boundaries) and keeps `sqlx` from leaking into call sites.

**Recommendation: Option C.** `sqlx` is the right horse, but
we don't want to bind sui-id's identity to it. A 200-line
shim gives us escape velocity if needed.

### Async ripple

`rusqlite` is sync. `sqlx` is async. Going to `sqlx` makes
the repo layer async, which makes `sui-id-core` async at
the touch points, which makes handlers async — but handlers
are already async (Axum). The propagation is real but not
catastrophic.

A pragmatic intermediate: keep `rusqlite` for the SQLite
backend, use `sqlx` only for Postgres / MariaDB. The
`Backend` trait declares async methods; the SQLite
implementation wraps `rusqlite` calls in
`spawn_blocking`. This is uglier than going fully `sqlx`
but preserves the option to keep SQLite's offline-first
posture (no async runtime when running embedded, useful
for tests that want to skip the tokio dance).

If the implementer chooses to bet fully on `sqlx`, that's
defensible too — the code is simpler. The recommendation
is the hybrid only if the test-runtime simplicity matters.

### Schema portability

Three families of difference:

**Trivial (per-backend column types in the migration runner):**

| SQLite | PostgreSQL | MariaDB |
|---|---|---|
| `TEXT` | `TEXT` | `VARCHAR(...)` or `TEXT` |
| `BLOB` | `BYTEA` | `VARBINARY(...)` or `BLOB` |
| `BOOLEAN` (stored as `INTEGER`) | `BOOLEAN` | `TINYINT(1)` |
| `TIMESTAMP` (TEXT in ISO 8601) | `TIMESTAMP WITH TIME ZONE` | `TIMESTAMP` (UTC convention) |

These are handled by either (a) writing migrations against
ANSI SQL and accepting a few `CREATE TABLE` quirks resolved
inline, or (b) having three migration directories
(`migrations/sqlite/`, `migrations/postgres/`,
`migrations/mariadb/`) with the same set of files keyed by
driver.

Recommend (b). The migrations are short, the duplication is
explicit and reviewable, and there is no clever templating
layer that could go wrong.

**Non-trivial (semantic):**

- **Partial indexes.** SQLite supports them. MariaDB does
  not (until recent versions, and even then with limits).
  Used for `users.email` uniqueness `WHERE email IS NOT NULL`
  and a few others. The MariaDB equivalent is a generated
  column + plain unique index, or just a unique index that
  treats `NULL` as distinct (MariaDB's default does, which
  actually matches our intent — verify per-version).
- **`UPSERT` / `ON CONFLICT`.** SQLite and PostgreSQL
  agree. MariaDB uses `ON DUPLICATE KEY UPDATE` with a
  different syntax. The repo layer needs a small
  `Backend::upsert()` helper rather than inline SQL.
- **Concurrency primitives.** `SELECT ... FOR UPDATE`
  exists on Postgres and MariaDB. SQLite's locking is
  implicit. The lockout-counter and refresh-token-theft
  paths assume single-writer; they work on all three but
  the explicit `FOR UPDATE` should be added on the
  multi-writer backends to make the contract explicit.

**Out of scope (handled by the application):**

- ID generation (UUIDs computed in Rust, never autoincrement).
- Audit hash chain (computed in Rust, persisted as bytes).
- All encryption (computed in Rust, persisted as bytes).

This is what makes the port plausible at all. Almost nothing
non-trivial happens in the database itself.

### Encrypted columns across backends

The XChaCha20-Poly1305 ciphertext is a byte string. It is
stored in `BLOB` (SQLite), `BYTEA` (Postgres),
`VARBINARY(N)` (MariaDB). The AAD is a fixed string per
column (e.g. `"users.password_hash"`) — not derived from
storage representation, so it's stable across backends.

**Acceptance test for the port**: take a database created
under SQLite, dump-restore it into Postgres, run the test
suite. Every encrypted column must round-trip. This is the
canary; if it fails, we have a portability bug somewhere
downstream of the migration types.

### Configuration

```toml
[storage]
driver  = "sqlite"        # default; or "postgres", "mariadb"

# SQLite: existing
db_path = "./sui-id.db"

# Postgres / MariaDB: new
url     = "postgres://sui_id:${SUIID_DB_PASSWORD}@db.example.internal:5432/sui_id"
# url   = "mysql://sui_id:${SUIID_DB_PASSWORD}@db.example.internal:3306/sui_id"
pool_max_connections = 10
```

The connection string supports `${VAR}` interpolation from
the process environment so the password never lives in the
config file. (`sui-id` already enforces this for SMTP and
LDAP creds; we extend the same machinery.)

`db_path` and `url` are mutually exclusive: SQLite uses
`db_path`, the others use `url`. Config validation rejects
mismatches at load time.

### Backup / restore

For SQLite: `sui-id backup --to PATH` continues to do a hot
snapshot of the `.db` file and the master key.

For Postgres / MariaDB: two options.

#### Option A: shell out to native tools

`sui-id backup --to PATH` invokes `pg_dump` / `mysqldump`
under the hood, captures stdout into PATH, optionally
encrypts with the master-key passphrase. Symmetric for
restore. Pros: works. Cons: requires the native tool to
be on PATH, error reporting is ugly, version skew is a
real source of incidents.

#### Option B: refuse, point at native tools

`sui-id backup` on a non-SQLite backend prints a clear
error: "this command operates on the SQLite backend only;
for Postgres/MariaDB use the native tools (`pg_dump`,
`mysqldump`) — see `docs/deployment.md`." Document the
operator-visible workflow in the deployment doc.

**Recommendation: Option B.** Backup is an operational
workflow, and operators on Postgres or MariaDB already
have one. Pretending sui-id's CLI replaces theirs creates
more confusion than convenience.

### Master-key rotation

`sui-id admin rotate-key` reseals every encrypted column
inside one transaction. Implementation today:

```rust
db.transaction(|tx| {
    for table_with_sealed_columns in TABLES {
        for row in tx.query(&format!("SELECT id, ... FROM {table}"))? {
            let plaintext = open(&old_key, &row.ciphertext, &row.aad())?;
            let new_ciphertext = seal(&new_key, &plaintext, &row.aad())?;
            tx.execute("UPDATE ... SET ... = ? WHERE id = ?", ...)?;
        }
    }
    tx.commit()
})
```

This works backend-agnostic. The only wrinkle is that on
Postgres / MariaDB the rotation transaction can be very
large and may want explicit batching (commit every N rows
in a savepoint). The SQLite path is fast enough to do
single-transaction; the Postgres/MariaDB path should
batch with save points to avoid holding locks for the
entire duration.

The `rotate-key` CLI gains a new flag `--batch-size N`
(default 1000) that's a no-op on SQLite and meaningful on
the others.

## Multiple implementation steps

Each step is independently shippable. SQLite remains the
default through every step.

1. **Storage trait + SQLite reimplementation.** Introduce
   the `Backend` trait, port the existing `rusqlite` code
   into `SqliteBackend` behind it. No behavioural change,
   no new backend yet. Tests pass against the new shape.
   This is the largest single piece of work.
2. **PostgreSQL backend.** Implement `PostgresBackend`.
   Per-backend migrations under `migrations/postgres/`.
   Round-trip test: SQLite-created DB dumps and restores
   into Postgres, encrypted columns intact, lib + e2e
   suites pass against the Postgres backend.
3. **MariaDB backend.** Same, with the additional fiddle
   for MariaDB's quirks (datetime precision, generated-
   column workarounds for partial indexes, `ON DUPLICATE
   KEY UPDATE` shape).
4. **CLI dispatch.** `sui-id backup` / `restore` /
   `verify-backup` gain backend-aware behaviour per the
   "refuse, point at native tools" recommendation. The
   `admin rotate-key --batch-size` flag lands here.
5. **Documentation pass.** Deployment guide gains a
   "Choosing a storage backend" section. README quickstart
   stays SQLite-default.

The first step is the bulk of the work. Steps 2–5 are
incremental and each is small enough to ship as its own
patch.

## Tests

- **Backend trait conformance.** A shared test fixture
  exercises every repo method against each backend
  implementation. Same assertions, three runs.
- **Migration runner.** Apply all migrations on each
  backend; assert schema shape (`information_schema`
  introspection or pragma equivalent) matches an
  expected snapshot.
- **Encrypted-column round-trip.** Seal under SQLite, dump
  to a backup file, restore to Postgres, open. Same for
  Postgres → MariaDB and MariaDB → SQLite.
- **Lib tests parameterised by backend.** `cargo test
  --features postgres` runs the lib suite against
  Postgres; `--features mariadb` against MariaDB. Default
  `cargo test` stays SQLite-only so the default
  contributor experience requires no database server.
- **E2E suite parameterised** the same way. The 148-test
  e2e harness should be a `Backend`-generic harness; the
  `test_app()` helper in `tests/e2e/common.rs` takes the
  backend choice from an environment variable, defaults
  to SQLite-in-memory.
- **Master-key rotation across backends.** Specifically:
  rotation while another connection is mid-transaction
  on the same table (`SELECT FOR UPDATE`-style behaviour).
  Currently impossible to test on SQLite (single-writer);
  becomes possible on Postgres/MariaDB and worth a
  dedicated test there.

## Security considerations

- **Connection-string secrets.** The DB password lives in
  an environment variable, not the config file. This is
  the same posture as SMTP and LDAP creds. The config
  loader rejects URLs with inline passwords (the parser
  warns + sui-id refuses to start, prompting the operator
  to use `${VAR}` indirection).
- **Transport security.** Postgres `sslmode=require` and
  MariaDB `ssl-mode=REQUIRED` are recommended in the
  deployment doc. sui-id refuses to connect over plain
  TCP unless `[storage] allow_insecure_connection = true`
  is explicitly set, mirroring the LDAP shape from RFC
  005.
- **Schema-level injection.** None of the new backends
  add a vector here; all SQL is parameterised through the
  `Backend` trait.
- **Network reachability as a new failure mode.** A
  Postgres outage now takes sui-id down where SQLite
  could not. This is unavoidable and the right tradeoff
  for operators who chose this backend; document it
  loudly in the deployment guide.
- **Backup posture differences.** With SQLite, `sui-id
  backup` ran under the same process and same
  filesystem. With native tools (`pg_dump`, `mysqldump`),
  the operator owns the backup pipeline — including
  encryption-at-rest of the dump. Document explicitly:
  *the encrypted-column ciphertext in a `pg_dump` output
  is still encrypted, but the dump itself is not — wrap
  it in `age` or `gpg` before sending offsite.*
- **Restore as a footgun.** `sui-id restore` on a
  Postgres/MariaDB backend, even if implemented, would
  drop the existing DB. We deliberately don't ship that
  CLI; operators using these backends use the native
  tools, where the destructive operation is at least
  named clearly (`pg_restore --clean`).
- **Encrypted-column AAD compatibility.** The AAD scheme
  must be invariant across backends. Tested by the
  round-trip canary above. A change to AAD computation in
  any backend implementation is a breaking change for
  every backend, not just that one.
- **Master-key rotation atomicity.** On Postgres/MariaDB
  the batched-save-point implementation must commit the
  *new key fingerprint* in `server_settings` only after
  every column has been re-sealed. Half-applied state is
  unrecoverable without the old key file (which the CLI
  preserves under `<original>.bak.<timestamp>` regardless
  of backend).

## Open questions

- **Async ripple cost.** If the SQLite path goes async
  via `sqlx`, the existing codebase's call sites (mostly
  already inside `async fn`) absorb it cheaply. If the
  hybrid path is taken (sync SQLite, async others), the
  trait surface gets uglier. Recommend implementer
  prototype the all-async approach first; revert to
  hybrid only if the simplification doesn't materialise.
- **MariaDB or PostgreSQL first.** PostgreSQL is the
  cleaner port (better partial-index story, better
  transaction semantics). Recommend ship Postgres first
  in step 2, MariaDB second in step 3.
- **Connection pool sizing defaults.** sui-id is
  single-instance. Pool size 10 is a reasonable default;
  expose for tuning. No leader election is needed since
  there's still only one process talking to the DB.
- **Drop SQLite eventually?** Strongly no. SQLite is
  the right default for the project's audience. The
  point of this RFC is *additive* support, not
  replacement. The "minimum operating ceremony"
  guarantee — `cp` to back up, no daemon to restart —
  must hold for the SQLite path forever.
- **Schema versioning across backends.** Migration N on
  SQLite and migration N on Postgres must produce
  semantically identical schemas. Add a `schema_version`
  table that records the migration version *and* the
  driver name; at startup, sui-id refuses to run if
  the recorded driver doesn't match the configured one
  (catches operator who tries to point a Postgres-
  initialised sui-id at a SQLite file).
