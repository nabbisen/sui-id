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

## Required behaviour

- **Missing row** (`QueryReturnedNoRows`, a fresh database): version 0, as now.
- **Any other read error:** return it. No migration runs.
- **A value that does not parse as a non-negative integer:** a new
  `StoreError::SchemaVersionInvalid`. No migration runs.
- **Stored version > `MAX_SCHEMA_VERSION`:** a new
  `StoreError::SchemaTooNew { found, supported }`. No migration runs, and nothing
  is written.

The server must exit non-zero with a one-line message that names both versions
and says to run the newer binary or restore the pre-upgrade backup. Check how
`Database::open`'s callers report errors, and make the message reach the
operator as that line. `open_in_memory` keeps its behaviour.

## Evidence

- One test per branch above, including a database stamped at
  `MAX_SCHEMA_VERSION + 1` whose other tables are left untouched: compare row
  counts and `sqlite_master` before and after.
- A read-error test. Use an injected fault or a locked database, whichever the
  store's test utilities support; say which.
- Run the binary against a database stamped `MAX_SCHEMA_VERSION + 1`, and give
  the exit code and stderr.
- Mutation check: remove each guard in turn, and show the matching test failing.
- fmt, both clippy scopes, `cargo test --workspace` count before and after, and
  MSRV 1.95.
- Update `docs/src/guides/upgrade.md` in the same package. Dispatch 15 makes it
  state today's behaviour; once this lands, it states the refusal.
