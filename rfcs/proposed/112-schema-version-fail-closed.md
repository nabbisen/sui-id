# RFC 112 — Refuse to run against a database this build does not understand

**Status.** Proposed
**Security review.** Required
**Independent design review.** [Design review 2026-09-25](../handoffs/112-schema-version-fail-closed/design-review-2026-09-25.md) by the implementation role, which authored neither this RFC nor its handoff. It **measured the defect and the damage** in a throwaway worktree, against the real binary as well as the store, and returned three high, four medium and three low findings with **no blocker**. Its verdict was **accept with changes**, and it was right on all three high findings: each is answered below. It also **corrected this RFC's own framing** of the severity.
**Design prerequisites.** None.
**Implementation prerequisites.** Runs in parallel with the backup move, which takes `migrations::MAX_SCHEMA_VERSION` as it is.
**Closure prerequisites.** A binary refuses to open a database it does not understand — newer than its ceiling, or carrying application tables with no readable version — **before it writes anything to the file**, and the operator is told in one line, first and alone on stderr, naming both versions, the path and the route back; a failed version read is a refusal, not a re-run; every reader of the stored version uses the same reader and the same rule; each has a test, and the no-write property is asserted in both journal modes.
**Tracks.** Data integrity. Found by RFC 098 dispatch 14.
**Touches.** `crates/sui-id-store/src/migrations.rs`, `crates/sui-id-store/src/db.rs`, `crates/sui-id-store/src/errors.rs`, `crates/sui-id-store/src/backup/ops.rs`, `crates/sui-id/src/runtime/startup.rs`, `crates/sui-id/src/cli.rs`, `crates/sui-id/src/http/handlers/settings.rs`, `docs/src/guides/upgrade.md`, `docs/src/guides/deployment.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/112-schema-version-fail-closed/README.md`](../handoffs/112-schema-version-fail-closed/README.md)

## Summary

`migrations::run` skips every migration at or below the stored `schema_version` and returns `Ok`, so a binary built before migration N starts happily against a database at N and reads and writes tables whose shape it does not know. `restore` already refuses a backup newer than `MAX_SCHEMA_VERSION`; the server applies no such rule to the database it opens. A failed read of the stored version re-runs every migration.

## Why this is an RFC

This work was carried under `roadmap/schema-version-fail-closed/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

*Amended 2026-09-25 by the architect, on the independent design review, **while
this RFC is Proposed**. The decisions below are the architect's proposal
answering that review; they are not yet approved. `@nabbisen` accepts or
rejects them.*

The review measured what the current behaviour actually costs, and it is not
what this RFC first claimed. **Migration 0001 is entirely
`CREATE TABLE IF NOT EXISTS`**, so on a populated database it succeeds
silently and **stamps the version `1`**; 0002 then fails on
`ALTER TABLE … ADD COLUMN`. No row and no table is damaged — but the real
version is **overwritten and lost**, the database is **permanently
unstartable**, and the operator reads "database I/O error". So the defect is
**availability and information loss, not data corruption**, and it stays that
way only while 0001 and 0002 remain DDL-only. This RFC's framing is corrected
here rather than left to be discovered.

**D1 — Refusing is the rule, and there is no override flag.** "Additive" is a
property of *some* migrations, not of the runner: five migrations drop or
rebuild (0013, 0019, 0021, 0022, 0043), and 0043 dropped
`credentials.must_change`, so an older binary would fail **on every credential
operation at runtime** rather than at start. An identity provider whose
operator learns of the failure from its users is the worse outcome. A flag
would recreate the hazard; the supported recovery — run the newer binary, or
`restore --force` from the pre-upgrade backup — is already documented.

**D2 — "Fresh" means no application tables, not no row.** Version `0` is
assumed only when the database has no tables other than `sui_meta`. A
populated database with no version row, and a **foreign SQLite file**, are
`SchemaVersionInvalid`. The review measured the hole: against a foreign
database with one table, `Database::open` returns `Ok` and **adds 26 tables to
it**. Without this, the closure prerequisite "a failed version read is a
refusal, not a re-run" is false.

**D3 — The check runs first, on a read-only connection.** `Database::open`
sets `journal_mode = WAL` **before** `migrations::run`, and `run` creates
`sui_meta` before reading the version, so a refusal inside `run` happens after
the file has been written to. The review measured a rollback-journal database
converted to WAL before any check could refuse it. The version is therefore
read on a `SQLITE_OPEN_READ_ONLY` connection before the pragmas and before any
`CREATE TABLE`; the pragmas and `run` follow only if it passes. The test
asserts the main file's bytes, `sqlite_master` and every row count are
unchanged **in both journal modes**, and does **not** assert the absence of
`-wal`/`-shm`, which a read-only connection leaves behind.

**D4 — One reader, one rule, for every caller.** `migrations::read_stored_version`
and `migrations::check_supported` are landed by this RFC and used by the
runner, backup create, restore, verify and the settings page. Today backup
create reads the version with `unwrap_or(0)` **twice**, so a snapshot whose
version cannot be read is stamped `0` in its manifest and passes restore's
check — the same defect in a second place — and the settings page shows the
binary's ceiling under the label "schema version". [RFC 106](106-backup-restore-hardening.md)
consumes these rather than writing a third reader.

**D5 — The refusal reaches the operator, or it has not happened.** Every
caller wraps the open in `.context("opening database")`, so today the operator
sees `Error: opening database` and the real cause on line four, at exit code
`1`, not logged through tracing on either path — and under the shipped
`Restart=on-failure` unit a refusing service is restarted until the start
limit trips. One handler, shared by `startup::prepare` and the CLI openers,
prints the line **first and alone** on stderr, emits one `tracing::error!`
carrying `found`, `supported` and `db_path` where tracing exists, and exits
with a **distinct code** so `RestartPreventExitStatus=` can be set. The RFC
names that code before implementation starts.

**D6 — The taxonomy, with the hole closed.** `SchemaTooNew { found, supported }`,
`SchemaVersionInvalid`, and a new `MigrationFailed { version, source }` — a
migration that fails to apply currently surfaces as "database I/O error",
which sends an operator to look at their disk. `found` is parsed as `i64`:
today it is `i32`, so a stored `99999999999` **overflows and is reported
invalid** when it is unmistakably too new.

**D7 — The database records which release stamped it.** One extra `sui_meta`
key, `last_migrated_by = <CARGO_PKG_VERSION>`, written in the same transaction
as the version. It costs one line and no schema change, and it is the
difference between telling the operator "run a newer binary" and "run
0.79.0". If the key is absent, the message says "a newer sui-id".

**D8 — Two limits are stated rather than silently left.** (a) The check is at
open time, so a newer binary migrating while an older one already runs is
**not** closed by this RFC; it is recorded as a non-goal here and as "one
binary version per database at a time" in `deployment.md`, and gets its own
RFC if it is to be closed. (b) `startup.rs` deliberately does **not** refuse
on an audit-chain mismatch, because that would let an attacker deny service by
corrupting one row. Refusing on a version stamp is consistent, and the RFC
says why in one sentence: a chain mismatch means the data is suspect but its
**shape is known**, while a too-new stamp means the shape is **unknown** and
the binary would be writing into it.

**D9 — The documents this makes false are corrected with it.**
`docs/src/guides/upgrade.md` states today that an older binary "**starts
without complaint**"; after this it refuses, and the paragraph becomes the
refusal and its recovery. `docs/src/guides/deployment.md`'s downgrade sequence
and its `Restart=on-failure` stanza change with D5's exit code. The refusal
line contains "schema" and "migrat" so the `journalctl | grep -i migrat` that
guide already teaches will find it.

The [handoff](../handoffs/112-schema-version-fail-closed/README.md) carries the
specification and the evidence requirements, and is updated to match these
decisions before implementation is dispatched.
