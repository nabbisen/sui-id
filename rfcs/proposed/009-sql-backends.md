# RFC 009 — Pluggable SQL Backends (PostgreSQL, MariaDB/MySQL)

**Status.** Proposed. **Refreshed 2026-11-25** against the code as built (`b18c862`); see *Refresh* below.
**Direction.** `@nabbisen`, 2026-11-25: *"Do you want us to be limited to SQLite ? SQLite's limitations become our
limitations. It is not my expect."*, and, on the architect's proposal to refresh this RFC: *"Yes. Approved."* That
approves the refresh as design work. **It is not this RFC's acceptance**, and nothing here may be built until it is
Accepted (RFC 000).
**Priority.** Was *Low, post-1.0*. **Now a direction**: SQLite must not be sui-id's only database. Scheduling the build is
proposed in *Refresh, §6*.
**Security review.** Required. It changes where every sealed column, the audit chain and the master-key rotation live.
**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted. Stage 1 (*Refresh, §5*) needs nothing else. Stage 2 needs stage 1
landed, and the driver decision (*§4*) recorded as an amendment.
**Closure prerequisites.** PostgreSQL is supported on the same terms as SQLite: every guarantee in *Refresh, §2* has a
conformance test that passes on both, in CI, and the RFC 097 inventory carries the new boundary. SQLite's behaviour is
unchanged, and the default deployment still needs no database server.
**Tracks.** ROADMAP / Longer term, *"Pluggable SQL backends"*. **Frozen**: the ROADMAP's feature freeze names alternative
SQL backends, so building any stage needs a freeze exception (*Refresh, §6*). It is designed together with the unscheduled
store-boundary RFC (*Refresh, §5*).
**Touches.** `sui-id-store` (the store interface, then a second backend); `sui-id` (`[storage]` configuration; `backup`,
`restore`, master-key rotation); CI (a PostgreSQL service); the RFC 097 inventory; docs.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role. The original text (2026-07-01) is kept
below the refresh. **Where the two disagree, the refresh governs.**

## Refresh against the code as built (2026-11-25)

### §1 — why the original text is not enough any more

It was written before RFC 094 (the audit row in the same transaction as its effect), RFC 096-B1's `ReadConn`, RFC 100
(crash-safe master-key rotation) and 37 of today's 55 migrations. **Three of its claims are no longer true**:
- **"Step 1 done" (the index, `rfcs/README.md`).** A `Backend` trait exists (`crates/sui-id-store/src/backend.rs`), but its
  closures take `&rusqlite::Connection` and `&rusqlite::Transaction`. So every one of the store's 391 SQL call sites is
  written against SQLite through it, and the seam is SQLite-shaped. **Stage 1 is not done.**
- **"`--batch-size N` commits in save-points" (*Master-key rotation*, and P5).** Committing a partial reseal leaves the
  database sealed under two keys. That is exactly RFC 100's invariant M1, and RFC 100 forbids it. **Withdrawn**: the reseal
  stays one transaction on every backend. Savepoints inside it are fine; commits are not.
- **"Single-instance" (*Non-goals*)** is right, but it is only written down. On SQLite, the file and RFC 100's liveness
  lock keep it true. On a networked database, nothing would (§2, row 9).

### §2 — what each backend must re-prove, measured

Each row is a guarantee sui-id ships today, the SQLite fact it rests on, and what another backend needs. **A backend is
supported only when every row has a conformance test that passes on it.**

| # | Guarantee | Rests today on | Another backend needs |
|---|---|---|---|
| 1 | **The audit row commits with its effect** (RFC 094 Class A) | `db.class_a(…, \|tx\| …)`: a **synchronous** closure over one `rusqlite::Transaction`. It cannot `await` anything (a hash, a network call) while it holds the write lock, **by construction** | The same by construction. Whatever the driver, the transaction body gets a handle that offers database operations only. A test fails if a command's body can reach anything else |
| 2 | **The audit chain never forks** | `append_within_tx` reads the last `hash` (`ORDER BY seq DESC LIMIT 1`), then inserts. That is safe only because SQLite has one writer, and sui-id one connection | **Explicit serialization of the chain head**: a locked chain-head row or a transaction-scoped advisory lock. A test runs N concurrent appends and asserts one unbroken chain. PostgreSQL's default `READ COMMITTED` would fork it |
| 3 | **Read-only handles cannot write** (`ReadConn`) | `sqlite3_stmt_readonly`, per statement, plus refusing `PRAGMA` and similar by keyword (`read_conn.rs`) | The server's own read-only transaction (PostgreSQL `SET TRANSACTION READ ONLY`, MariaDB `START TRANSACTION READ ONLY`). That is **stronger** than today's check, and is tested by a write attempt that must fail |
| 4 | **Lock-before-read where a guard is read** (C17, RFC 096-B2 S4) | `BEGIN IMMEDIATE` (`with_tx_immediate_erased`) | An explicit row lock (`SELECT … FOR UPDATE`) or an advisory lock at each such site. The sites are enumerated in stage 1, not found later |
| 5 | **Rollback is proven, not assumed** (RFC 094 M2a, lane G20) | Real commit failure injected through rusqlite's `commit_hook` | A per-backend way to make a real commit fail. **Without one, G20 cannot run on that backend, and it is not supported** |
| 6 | **The floor and invariants hold in the store** | 55 SQLite migrations: `CHECK`s (for example `password_min_len BETWEEN 12 AND 128`), a `RAISE(ABORT)` guard (0053), one partial index, `PRAGMA`s in 5 and date functions in 5 | Per-driver migrations, as the original proposes. **And a gate that compares the constraint sets across drivers**, so a `CHECK` that is security-bearing on SQLite cannot be missing on PostgreSQL |
| 7 | **Sealed columns open on every backend** (original P1) | XChaCha20-Poly1305. The AAD is a constant per column, or is built from row identifiers rendered as text (for example `federation_login_attempt`) | P1 as written, plus: an identifier used in an AAD is stored and read back **as the same text**. A native `uuid` column type is allowed only if a round-trip test proves the bytes identical |
| 8 | **Master-key rotation is crash-safe and refuses beside a running sui-id** (RFC 100, P7) | One reseal transaction (K08). A journal beside the key file. **A `flock` on `<key>.rotation-lock`**, which every process holding the database takes shared | The reseal as row 1. **The liveness guard must also live in the database.** `flock` sees only one host, and a networked database can be opened from any host. Rotation stays offline |
| 9 | **One instance** | One file and one process. The rate limiter (`runtime/ratelimit.rs`), the revoked-token cache and the signing-key cache (`sui-id-core/src/cache.rs`) are **process memory** | **An instance lease in the database, so a second instance refuses to start**, rather than a sentence saying not to start one. Two instances would multiply every rate limit. And a token revoked through one would be accepted by the other until its cache expired. Multi-instance is a later, separate RFC |
| 10 | **Backups carry the database and its key** | `backup create`: SQLite's backup API (`cli/handlers.rs`), with the key file, in one tarball | The original's Option B, refined. On a networked backend, `backup create` **refuses the database part and names the native tool**. It still saves the key file, because the database dump is unreadable without it, and saying only "use `pg_dump`" would leave the key behind |
| 11 | **Embedding** (RFC 119) | `libsqlite3-sys` declares `links = "sqlite3"`: one version per build | Stage 1's interface is RFC 119's store boundary too. The driver decision (§4) weighs the embedder's version coupling |

### §3 — what stays from the original

Its motivation, and SQLite as the default for ever. P1 (with row 7), P2 (the password only through `${VAR}`), P3 (TLS
required unless explicitly allowed, **and the server certificate verified**), P4 and P6. Per-driver migration directories.
PostgreSQL before MariaDB.

### §4 — the driver: decided by prototype, against stated criteria

The original recommended sqlx behind a shim. **The refresh does not decide**: the guarantees in §2 decide, measured on a
prototype in stage 2. The candidates:
- **sqlx for every backend**, including SQLite;
- **a hybrid**: rusqlite for SQLite, and sqlx (or a native driver) for the networked backends;
- **a native driver per backend** (for example `tokio-postgres`).

The criteria, in order:
1. rows 1 to 5 of §2 hold **by construction** on that driver;
2. a security fix in the bundled SQLite reaches a sui-id release without waiting for an intermediate crate (the
   RUSTSEC-2026-0459 bump of rusqlite to 0.41 is the precedent);
3. the build needs no running database, so no compile-time query checking against a live server;
4. dependency weight and supply chain;
5. an embedder's version coupling (row 11).

The decision is recorded as an amendment, with the prototype's measurements.

### §5 — stages

1. **The store, database-neutral.** Commands and repositories become typed operations behind an interface **no driver type
   crosses**. SQLite stays on rusqlite and behaves byte for byte as today. This stage also enumerates row 4's sites and row 9's
   in-process state. **It changes no behaviour, and it is the largest stage.**

   **Two further items for this stage, added 2026-11-26** (RFC 119's
   [host setup design](../handoffs/119-embedding-contract/host-setup-design-2026-11-26.md), §2 and §3):
   - **Unique table names.** Every table gets one fixed prefix, `sui_id_`, built into its name, through one migration that
     renames the 31 tables. It is not configurable. Only `sui_meta` is namespaced today, and names like `users` and `sessions`
     collide in a database shared with a host. PostgreSQL also gets a schema of sui-id's own.
   - **The liveness guard is taken by the store's own `open`**, keyed on the database rather than the key file, so every
     opener holds it without being told to (row 8).

   **It is not the ROADMAP's store-boundary RFC, and the two must be designed together.** That RFC (unscheduled, no number)
   puts a trait between `sui-id-core` and `sui-id-store`, for embedding. This stage puts the line lower, between the
   store's commands and the SQL a driver runs. **The lower line is the one that keeps every §2 guarantee implemented once.**
   Class A, the audit chain and the registry stay in shared code, and only SQL and locking differ per backend. A backend
   that implemented the upper trait from scratch would implement every guarantee again, and could get one wrong
   independently. The upper trait can still come later, above this one; neither forecloses the other.
2. **The driver prototype and decision** (§4), recorded as an amendment.
3. **PostgreSQL**:
   - its migrations;
   - rows 1 to 10, each with its conformance test;
   - CI with a PostgreSQL service;
   - the instance lease;
   - the database-level liveness guard;
   - the RFC 097 inventory's new boundary (a database reached over the network).
4. **The operator's side**: `[storage]`, `backup` and `restore` dispatch, and the guide (*Choosing a storage backend*).
5. **MariaDB/MySQL**, on the same terms.

### §6 — sequencing, proposed

- **Now, during cycle Q, beside RFC 149**: the review of this refresh, and its acceptance.
- **Stage 1: the cycle after Q**, or Q's second half if RFC 149 lands early. It touches every store call site, so it must not
  run beside other store-heavy work.
- **Stages 2 to 4 follow, one cycle each at most.**
- **Not before the `0.94.0` cut, and it does not displace RFC 149.** This sequencing is a proposal.
- **Every build stage needs a freeze exception.** The ROADMAP's freeze admits four kinds of work: urgent security fixes,
  remediation-enabling refactors, dependency and security maintenance, and repairs that keep approved gates executable.
  Alternative SQL backends are named as frozen. **Stage 1 is none of the four**: it enables a feature, not the
  remediation. The exception is the owner's to grant or refuse. Designing and reviewing this refresh needs none.

### §7 — open questions, for the review and the owner

1. **Multi-instance.** Row 9 keeps it out, enforced. Is a multi-instance sui-id on PostgreSQL wanted at all? If so, it is the
   next RFC after this one, because every in-process state in row 9 then needs a shared home.
2. **MySQL as well as MariaDB**, or MariaDB only? They have diverged enough that "MariaDB/MySQL" is two test targets, not
   one.
3. **Moving an existing deployment between backends.** The original leaves it to the operator: a dump, a restore, and the
   sealed-column canary. Should sui-id offer a verified `migrate-storage` command instead? Half 2 argues yes, because a
   hand-made migration that silently drops a `CHECK` is the failure row 6 exists for.

---

*The original text, 2026-07-01, follows. The refresh above governs where they differ.*

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
