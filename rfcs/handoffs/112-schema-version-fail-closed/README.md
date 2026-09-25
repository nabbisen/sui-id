# Refuse to run against a database this build does not understand

**Authorized by.** [`ROADMAP.md`](../../../ROADMAP.md) §Non-RFC work packages. Owner
authorization, 2026-09-16.
**Implementer.** Mid-capability model. **Baseline.** `9f6acdb` or later.
**Runs in parallel** with the backup move. That move takes
`migrations::MAX_SCHEMA_VERSION` as it is; this package changes `migrations::run`
and adds one error variant.

## The defect

This was measured by reading `crates/sui-id-store/src/migrations.rs`, lines
278–303, at `9f6acdb`. RFC 098 dispatch 14 found it, and the architect confirmed
it.

1. **An older binary runs against a newer schema without complaint.** `run` skips
   every migration at or below the stored `schema_version` and returns `Ok`. A
   binary built before migration N starts against a database at N and reads and
   writes tables whose shape it does not know. `restore` already refuses a backup
   newer than `MAX_SCHEMA_VERSION`. The server does not apply the same rule to its
   own database.
2. **A failed version read is treated as a fresh database.** The stored version is
   read with `.query_row(...).ok().and_then(parse).unwrap_or(0)`. Any read error
   (a busy database, an I/O fault) and any unparsable value become `0`, and the
   runner then re-applies every migration from 0001 against a populated database.

## Required behaviour — rewritten 2026-09-25 to match RFC 112 D1–D9

Dispatched in **two stages**. Stage 1 is the store; stage 2 is what the
operator sees. Stage 2 does not start until stage 1 lands.

### Stage 1 — the store refuses, and writes nothing doing it

- **One reader, one rule (D4).** Land
  `migrations::read_stored_version(&Connection) -> Result<StoredVersion, SchemaError>`
  and `migrations::check_supported(v)`. Route the runner, **backup create**
  (`backup/ops.rs:120-133`, which reads with `unwrap_or(0)` **twice** today),
  restore, verify, and the settings page (`http/handlers/settings.rs:327`,
  which shows `MAX_SCHEMA_VERSION` under the label "schema version" — the
  binary's ceiling, not the database's value) through them. RFC 106 consumes
  these; do not leave it a third reader to write.
- **The check runs first, on a read-only connection (D3).** `Database::open`
  today sets `journal_mode = WAL` and two more pragmas **before**
  `migrations::run`, and `run` executes `CREATE TABLE IF NOT EXISTS sui_meta`
  before it reads the version. Open `SQLITE_OPEN_READ_ONLY`, read, decide;
  only then the pragmas and `run`. `open_in_memory` keeps its behaviour.
- **Classification (D2, D6).** Version `0` **only** when the database has no
  tables other than `sui_meta`. Otherwise:
  - stored > `MAX_SCHEMA_VERSION` → `SchemaTooNew { found: i64, supported: i32 }`;
  - unparsable, negative, empty, a BLOB, **or absent from a database that has
    application tables** → `SchemaVersionInvalid`, naming the table count;
  - parse as **`i64`**: today `current` is `i32`, so `99999999999` overflows
    and is reported invalid when it is unmistakably too new;
  - a migration that fails to apply → `MigrationFailed { version, source }`,
    which today surfaces as `StoreError::Db`, Display **"database I/O error"**,
    sending an operator to look at their disk.
  - `+43` parses today. Refuse non-canonical forms.
- **Read the version inside `BEGIN IMMEDIATE`**, which closes the race between
  two **new** binaries starting together (the loser currently fails with
  `duplicate column name`). It does **not** close the older-binary-already-
  running case: that is D8(a)'s stated non-goal.
- **Stamp the release (D7).** `sui_meta.last_migrated_by = <CARGO_PKG_VERSION>`,
  written in the **same transaction** as the version. No schema change.

**Evidence for stage 1.** One test per branch above. The no-write property is
asserted for a database in **rollback-journal mode and in WAL mode**, each
stamped `MAX_SCHEMA_VERSION + 1`: `Database::open` returns `SchemaTooNew`, and
afterwards the **main file's bytes**, `sqlite_master` and **every table's row
count** are identical and `sui_meta.schema_version` is unchanged. **Do not
assert the absence of `-wal`/`-shm`** — a read-only connection leaves empty
ones. Include a **foreign SQLite file** (one unrelated table): it must be
refused, not migrated; today it gains 26 tables. Include a populated database
with the version row deleted. Mutation check: move the check back after the
pragma and show the rollback-journal case failing; remove each guard in turn
and show the matching test failing.

### Stage 2 — the operator is told, once, in a line they can act on

- **One handler (D5)**, called by `startup::prepare` and by all seven CLI
  openers. Every caller today wraps the open in `.context("opening database")`,
  so the operator sees `Error: opening database` with the real cause on line
  four. The line goes to **stderr, first and alone**.
- **Exit `65`**, for **both** refusal variants. See D5: a code covering one of
  them covers nothing.
- **One `tracing::error!`** carrying `found`, `supported` and `db_path` where
  tracing exists (`serve` initialises it before the open; the CLI has none and
  stderr is its record).
- **The line itself** is drafted in the design review §4 item 11. It names the
  path, both versions, the release that stamped it when `last_migrated_by` is
  present, the route back, and what **not** to do. It contains "schema" and
  "migrat" so that the `journalctl | grep -i migrat` that `deployment.md`
  already teaches will find it.
- **Docs (D9).** `docs/src/guides/upgrade.md:94-98` says an older binary
  "**starts without complaint**" — it becomes the refusal and its recovery.
  `docs/src/guides/deployment.md`: the downgrade sequence, the
  `Restart=on-failure` stanza at `:227` (a refusing service restarts until the
  start limit trips — state `RestartPreventExitStatus=65`), and the three-line
  exit-code table D5 establishes. State D8(a) as "one binary version per
  database at a time".

**Evidence for stage 2.** Run the real binary against a database stamped
`MAX_SCHEMA_VERSION + 1` and against a garbled stamp: give the **exact stderr
and the exit code** for each, for `serve` and for one CLI subcommand. Show the
`tracing` event. Show `mdbook build docs` green.

### Both stages

fmt, both clippy scopes, `cargo test --workspace` count before and after, and
MSRV 1.95. **Cite by function, not by line** — the numbers in this document
moved once already.

## Independent design review returned 2026-09-25 — this specification is superseded in part

[`design-review-2026-09-25.md`](design-review-2026-09-25.md) returned **accept
with changes**: three high, four medium, three low findings, no blocker. The
RFC's Decision section now carries D1–D9 answering them, and **those decisions
win over the "Required behaviour" section above** wherever the two differ. The
differences that matter: "missing row → version 0" is replaced by "no
application tables"; the check moves ahead of the pragmas onto a read-only
connection; the refusal needs a shared handler, a distinct exit code and a
tracing event, not a new error variant alone; and one shared
`read_stored_version` / `check_supported` replaces the guard-inside-`run`.

**The severity in "The defect" above is overstated and is corrected in the
RFC:** migration 0001 is all `CREATE TABLE IF NOT EXISTS`, so on a populated
database the re-run succeeds silently, stamps the version `1`, and 0002 then
fails. No data is damaged; the true version is destroyed and the database is
left permanently unstartable. This holds only while 0001 and 0002 stay
DDL-only.

**Three low findings carried here rather than into the RFC**, to be handled
when implementation is dispatched:

1. `+43` parses and is accepted. A canonical-form check would refuse it.
2. Reading the version inside `BEGIN IMMEDIATE` closes the race between two
   **new** binaries starting together (the loser currently fails with
   `duplicate column`). Cheap while the function is open; it does not close the
   older-binary-already-running case, which D8(a) records as a non-goal.
3. This document's line citations (`278–303` at `9f6acdb`) have moved to
   `298–322`. **Cite by function, not by line.**

**Before implementation is dispatched**, this specification is rewritten to
match D1–D9, and the RFC names D5's exit code.
