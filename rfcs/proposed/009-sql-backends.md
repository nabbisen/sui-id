# RFC 009 — Pluggable SQL Backends (PostgreSQL, MariaDB/MySQL)

**Status.** Proposed (longer-term, no scheduled delivery)
**Priority.** Low. Large, multi-step storage feature. SQLite remains the
default and recommendation. Requires explicit owner direction before
scheduling.
**Tracks.** ROADMAP / Longer term — "Pluggable SQL backends".
**Touches.** `sui-id-store` (wide rewrite — repository implementations and
the migration runner behind a `Backend` trait); `sui-id` (config gains
`[storage] driver`; backup/restore/verify CLI learn to dispatch);
`Cargo.toml` (`sqlx` with selectable driver features); docs (deployment
guidance, "which backend" essay).

## Summary

sui-id today persists everything to a single SQLite file — the right
default: zero ops surface, `cp`-to-back-up, runs offline. This RFC adds
*optional* PostgreSQL and MariaDB/MySQL backends behind a config flag, so
an operator who already runs a database cluster can point sui-id at it
instead of introducing a SQLite file alongside.

SQLite remains the default, the single-binary distribution stays
single-binary (drivers are compile-time features shipped in the release
binary), and nothing in sui-id's "minimum operating ceremony" stance
changes for the existing user base. The hard part is not query rewriting;
it is keeping encrypted-column AAD bit-identical across backends and
keeping transaction-isolation assumptions explicit.

## Motivation

Deployments that already operate Postgres or MariaDB would rather not
introduce and separately back up a new SQLite file. Meeting them where
they are — additive backend support, SQLite untouched — widens sui-id's
applicability without compromising the simple default. This is additive,
never a replacement: the `cp`-to-back-up, no-daemon guarantee must hold
for the SQLite path forever.

## Background — why this is hard, not just tedious

The SQLite-shaped assumptions in `sui-id-store` are not all visible at the
call sites:

- **Column-encryption AAD.** XChaCha20-Poly1305 with an AAD scheme
  validated against SQLite's byte-exact storage. Postgres/MariaDB store
  `BYTEA` / `VARBINARY` with their own conventions; the AAD computation
  must stay bit-identical across backends or every encrypted column
  becomes unreadable after a dump-restore migration.
- **Migrations.** 18+ forward-only migrations written as raw SQLite SQL.
  `AUTOINCREMENT` / `SERIAL` / `AUTO_INCREMENT`, datetime types, partial
  indexes, `WITHOUT ROWID` do not translate cleanly.
- **`admin rotate-key`.** Reseals every encrypted column inside one
  transaction. The atomicity guarantee is straightforward on all three
  (all have transactions) but the mechanics must be backend-agnostic.
- **Backup / restore.** `sui-id backup` does a hot SQLite snapshot.
  Postgres/MariaDB have entirely different primitives (`pg_dump`,
  `mysqldump`, filesystem snapshots).
- **Theft detection, audit hash chain, lockout counters.** All
  read-modify-write under a transaction. Isolation defaults differ:
  Postgres `READ COMMITTED`, MariaDB `REPEATABLE READ`, SQLite effectively
  `SERIALIZABLE` via its locking model. The current code assumes the
  strongest guarantee implicitly.

The mechanical work (query rewriting, type mapping) is large but
tractable. The semantic work above is what keeps this longer-term.

## Target code areas

- **`sui-id-store`** — a `Backend` trait wrapping connect/execute/query/
  transaction; `SqliteBackend` (port of the existing `rusqlite` code);
  `PostgresBackend` and `MariadbBackend` over `sqlx`; per-driver migration
  directories; a `Backend::upsert()` helper for `ON CONFLICT` /
  `ON DUPLICATE KEY UPDATE` divergence.
- **`sui-id` config** — `[storage] driver`, `url` with `${VAR}`
  interpolation, `pool_max_connections`, `allow_insecure_connection`.
- **`sui-id` CLI** — `backup` / `restore` / `verify-backup` backend
  dispatch; `admin rotate-key --batch-size N`.
- **docs** — "Choosing a storage backend" deployment section.

## Security properties / invariants

- **P1 (AAD invariant across backends).** The AAD is a fixed per-column
  string (e.g. `"users.password_hash"`), not derived from storage
  representation; ciphertext is byte-for-byte identical for the same
  plaintext-and-key on every backend. A change to AAD computation in any
  backend is a breaking change for *all* backends.
- **P2 (connection-string secrets via env).** The DB password lives in an
  environment variable; the config loader rejects inline passwords and
  requires `${VAR}` indirection (same posture as SMTP and the RFC 005 LDAP
  bind secret).
- **P3 (transport security).** Postgres `sslmode=require` / MariaDB
  `ssl-mode=REQUIRED` recommended; sui-id refuses plain TCP unless
  `[storage] allow_insecure_connection = true` is explicitly set
  (mirrors RFC 005).
- **P4 (parameterised SQL only).** All SQL goes through the `Backend`
  trait with bound parameters; no new injection vector.
- **P5 (rotation atomicity).** On Postgres/MariaDB the batched
  save-point rotation commits the new key fingerprint in `server_settings`
  *only after* every column is re-sealed; the old key file is preserved
  under `<original>.bak.<timestamp>` on every backend.
- **P6 (driver/data match enforced).** A `schema_version` row records the
  migration version *and* the driver name; at startup sui-id refuses to
  run if the recorded driver does not match the configured one (catches an
  operator pointing a Postgres-initialised sui-id at a SQLite file).

## Non-goals

- Dropping or deprecating SQLite — strongly out of scope; SQLite is the
  forever-default for the project's audience.
- A `sui-id restore` CLI for non-SQLite backends (operators use native
  tools where the destructive operation is named clearly).
- Multi-instance / leader election — sui-id is single-instance; one
  process talks to the DB regardless of backend.
- ID generation, audit hash chain, and encryption moving into the
  database (they stay in Rust, which is what makes the port plausible).

## Proposed design

### Library choice — recommend Option C (`sqlx` + thin `Backend` shim)

- **Option A — `sqlx` multi-driver.** One well-maintained dependency,
  pool/async/null-handling in one, per-driver migrations supported.
  Costs: compile time; query-macro validation is single-backend; async
  ripples upward.
- **Option B — hand-rolled trait over three driver crates.** Maximum
  control, keeps `rusqlite` sync for SQLite. Costs: re-inventing pooling,
  prepared statements, null handling — months of work; heterogeneous
  errors.
- **Option C (recommended) — `sqlx` for I/O wrapped in a sui-id `Backend`
  trait.** A ~200-line shim keeps `sqlx` out of call sites and preserves
  escape velocity if `sqlx`'s direction ever diverges.

### Async ripple

`rusqlite` is sync; `sqlx` is async. Going to `sqlx` makes the repo layer
async, which the handlers (already async under Axum) absorb. A pragmatic
hybrid keeps `rusqlite` for SQLite (wrapped in `spawn_blocking` behind the
async `Backend` trait) and uses `sqlx` only for Postgres/MariaDB — uglier,
but preserves SQLite's no-runtime-needed posture for embedded tests.
Recommend the implementer prototype all-async first and fall back to
hybrid only if test-runtime simplicity demands it.

### Schema portability

Trivial type differences (`TEXT`/`BYTEA`/`VARBINARY`,
`BOOLEAN`/`TINYINT(1)`, datetime conventions) are handled by **per-driver
migration directories** (`migrations/sqlite/`, `migrations/postgres/`,
`migrations/mariadb/`) keyed by driver — the migrations are short and the
explicit duplication is reviewable, with no templating layer to go wrong.

Non-trivial differences: partial indexes (SQLite yes, MariaDB limited — use
a generated column + plain unique index, or rely on MariaDB's
NULL-distinct default where it matches intent); `UPSERT` (`ON CONFLICT` vs
`ON DUPLICATE KEY UPDATE` via a `Backend::upsert()` helper); concurrency
primitives (`SELECT ... FOR UPDATE` made explicit on the multi-writer
backends where SQLite's locking was implicit).

### Encrypted columns across backends

The XChaCha20-Poly1305 ciphertext is a byte string stored in `BLOB` /
`BYTEA` / `VARBINARY`. The AAD is a fixed per-column string (P1). The
canary acceptance test: create a DB under SQLite, dump-restore into
Postgres, run the suite — every encrypted column must round-trip.

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

`db_path` and `url` are mutually exclusive; config validation rejects
mismatches at load (P2).

### Backup / restore — recommend Option B (refuse, point at native tools)

For SQLite, `sui-id backup` is unchanged. For Postgres/MariaDB, the CLI
prints a clear error pointing at `pg_dump` / `mysqldump` and the
deployment doc, rather than shelling out (shelling out works but couples
sui-id to native-tool version skew and produces ugly errors). Backup is an
operational workflow operators on these backends already own.

### Master-key rotation

The existing reseal-every-column-in-one-transaction logic is
backend-agnostic. On Postgres/MariaDB the transaction can be large; a new
`--batch-size N` flag (default 1000; no-op on SQLite) commits in
save-points to avoid holding locks for the whole duration. The new key
fingerprint commits only after all columns are re-sealed (P5).

## Data model impact

No change to logical schema (same tables, columns, semantics). Physical
type mappings differ per backend via per-driver migration directories. A
`schema_version` row gains a driver-name field (P6). Encrypted-column
bytes are identical across backends (P1).

## API impact

None at the HTTP layer. The config schema gains `[storage]` fields and the
CLI gains backend-aware backup/restore/verify behaviour plus
`rotate-key --batch-size`.

## Testing strategy

- **Backend trait conformance.** A shared fixture exercises every repo
  method against each backend implementation — same assertions, three
  runs.
- **Migration runner.** Apply all migrations on each backend; assert
  schema shape (introspection snapshot).
- **Encrypted-column round-trip.** Seal under SQLite, dump, restore to
  Postgres, open. Same for Postgres→MariaDB and MariaDB→SQLite (P1).
- **Lib + e2e parameterised by backend.** `cargo test --features postgres`
  / `--features mariadb`; default `cargo test` stays SQLite-only so the
  default contributor needs no database server. The e2e `test_app()`
  helper takes the backend from an env var, defaulting to SQLite
  in-memory.
- **Rotation under concurrency.** Rotation while another connection holds
  a `SELECT ... FOR UPDATE` on the same table — impossible to test on
  SQLite (single-writer), worth a dedicated test on the multi-writer
  backends (P5).

## Migration strategy

Per-driver migration directories. A deployment does not migrate *between*
backends automatically — it chooses one at deploy time. Moving an existing
SQLite deployment to Postgres is an operator workflow (dump-restore +
config change), validated by the encrypted-column round-trip canary. The
driver/data match check (P6) prevents accidentally pointing a
backend-mismatched config at existing data.

## Rollout plan

Five independently-shippable steps; SQLite stays the default through every
one:

1. **Storage trait + SQLite reimplementation.** Introduce `Backend`, port
   `rusqlite` into `SqliteBackend`. No behavioural change. The largest
   single piece.
2. **PostgreSQL backend.** `PostgresBackend`, `migrations/postgres/`,
   round-trip + lib + e2e against Postgres.
3. **MariaDB backend.** Same, plus MariaDB quirks (datetime precision,
   generated-column workaround for partial indexes,
   `ON DUPLICATE KEY UPDATE`).
4. **CLI dispatch.** Backend-aware backup/restore/verify;
   `rotate-key --batch-size`.
5. **Documentation pass.** "Choosing a storage backend" section; README
   quickstart stays SQLite-default.

PostgreSQL ships before MariaDB (cleaner partial-index and transaction
story). No version designation without owner direction and soak.

## Risks and mitigations

- *Risk:* encrypted columns unreadable after a backend change.
  *Mitigation:* P1 + the round-trip canary acceptance test.
- *Risk:* connection-string secret leakage. *Mitigation:* P2 — env
  indirection, loader rejects inline passwords.
- *Risk:* a DB outage takes sui-id down where SQLite could not.
  *Mitigation:* unavoidable for these backends; documented loudly as the
  operator's chosen tradeoff.
- *Risk:* half-applied master-key rotation. *Mitigation:* P5 — batched
  save-points, fingerprint last, old key file preserved.
- *Risk:* operator points a mismatched config at existing data.
  *Mitigation:* P6 — startup driver/data match check.

## Acceptance criteria

- Default `cargo test` and the default deployment require no database
  server; SQLite behaviour is byte-for-byte unchanged.
- Every repo method passes the shared conformance fixture on all three
  backends.
- Encrypted columns round-trip across all backend pairs.
- The config loader rejects inline DB passwords and (without explicit
  opt-in) plain-TCP connections.
- Master-key rotation is atomic on all three backends.
- Startup refuses a driver/data mismatch.
- 0 warnings; full suite green; all CI gates hold.

## Open questions

- All-async vs hybrid SQLite path — prototype all-async first; revert to
  hybrid only if simplification fails to materialise.
- Postgres or MariaDB first? **Postgres** (cleaner port).
- Connection-pool default — 10 is reasonable for a single instance;
  expose for tuning.
- Drop SQLite eventually? **Strongly no** — additive only.
- Schema-version-with-driver table shape — record version + driver;
  refuse mismatched startup (P6).
