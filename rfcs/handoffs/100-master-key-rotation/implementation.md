# RFC 100 implementation stages

**Governing RFC:** [RFC 100](../../accepted/100-master-key-rotation-recovery.md),
**Accepted** 2026-11-14 and amended (non-material) the same day.
**The harness:** [`crash-injection-harness-design-2026-11-14.md`](./crash-injection-harness-design-2026-11-14.md),
**approved** 2026-11-14.
**Rewritten 2026-11-14** (cycle M, D2) to the RFC's *Refresh against the code
as built* and to the approved harness. The earlier version built `K04`, named
the events `database_resealed` and `activated`, and had no stage for the
startup key check. Each was replaced by the refresh.
**Status.** Planning companion. **Not dispatched.** Each stage gets its own
dispatch, in `0.92.0`'s cycle.

## Names, as built

RFC 100's body keeps the names it was written with. Its refresh maps them
(`:105-118`). Build with these:

| RFC 100's body says | Build | Why |
|---|---|---|
| `K04` | **K07**, `commands::reseal_master_key` (`sui-id-store/src/commands.rs:6755`), extended | it already is the database phase; `K04` stays retired |
| `admin.master_key.database_resealed` | **`admin.master_key.resealed`** | shipped; it already asserts only that the database phase committed (M4) |
| `admin.master_key.activated` | **`admin.master_key.rotated`**, made Class-A in K05 | shipped as a best-effort append (`cli.rs:671`); K05 makes it a Class-A record |
| `DbCommitted` | **durable**, not only committed | prefix 4: WAL with `synchronous=NORMAL` (`db.rs:43-44`) |

## The rules every stage keeps

- **Recovery lands before the thing it recovers.** Never build a write path
  whose crash prefixes have no resolution yet.
- **The adapter is the only path to the key files.** No `std::fs` call on a
  key path outside the Real adapter. Stage 0 adds the check that enforces it.
- **Every stage's tests run in all three failure modes**: short write,
  process termination, and power loss. "Durability can't be observed from a
  test" is no longer true. The Simulated adapter observes it.
- **Fingerprints, never key material**, in any log, test output, journal or
  audit record.

## Stage 0 — the harness, first

**Surfaces:** a key-file adapter module in `crates/sui-id` (where rotation
and `keyring` live); a test-only cargo feature for failpoints; a test-support
module for the Simulated adapter and materialization.

The adapter, exactly the primitives RFC 100 names: durable create
(`create_new`, mode `0600`), `write_all`, `sync_file`, atomic replacement
(`rename`), atomic no-replace publication (`hard_link`, then unlink the
temporary), `sync_dir`, `read`, `remove`, and metadata (owner, mode,
symlink). Three implementations, as the design's §4 gives them:
- **Real**: `std` only, with **no new dependency** and no `unsafe`.
- **Failpoint**: wraps Real. At a named point it aborts the process, or makes
  a write return short or fail. Points are read from an environment variable,
  only under the test-only feature.
- **Simulated**: records operations per inode and per directory entry,
  enumerates every disk a power loss could leave, and **materializes** each
  one into a real temporary directory. The database is part of it: a commit
  stays pending until it is made durable.

**Move today's rotation onto it**, behaviour unchanged. That is stage 1's
fixed order (`0.91.0`), with A2's checkpoint.

**Tests: the prototype's results, reproduced against the real code.**
- Today's order passes in all three modes. No state is wrong after success.
- **Remove each sync, one at a time**, and the power-loss mode fails for
  each, except the directory sync after the first rename. The design's §2
  records that one as not load-bearing. Record each result.
- **Remove A2's checkpoint**, and the power-loss mode finds the rolled-back
  reseal under the new key.
- **The kill-only mode passes the `0.90.0` order.** This one demonstrates the
  blindness. It is a test with that expected result, named so it says so.
- A check that no key path is touched by `std::fs` outside Real. A grep-based
  test is enough. Mutate it by adding one call, and it fails.

## Stage 1 — the startup key check, and no key generated over a sealed database

**Surfaces:** a migration (the sentinel, the `master_key_rotation` row);
`runtime/keyring.rs`; `runtime/startup.rs`; every CLI command that calls
`resolve`.

RFC 100's refresh, changes 2 and 3:
- **A sentinel**: a small value sealed under the master key, in the database.
  **Checked before readiness on every start**, and by every CLI command that
  opens the database. A key that does not open it refuses, naming
  fingerprints, never key bytes.
- **`resolve` generates a key only for a database that has never been
  sealed.** With a sentinel present, a missing key file fails closed.

**Upgrade: the one design point to measure first.** An existing database has
no sentinel. Proposed (the architect's, to be confirmed in this stage's
dispatch):
- If any row of the six tables K07 reseals exists, the database **has been
  sealed**. The active key must then open one of those rows before the
  sentinel is written. If it cannot, refuse to start, because the key is
  wrong.
- If none exists, the database has never been sealed, so write the sentinel
  under the active key.

**Do not infer "never sealed" from a missing sentinel alone.** That is the
regenerate-over-a-sealed-database hazard (prefix 2) in a new form.

Add the singleton `master_key_rotation` row here too, unused until stage 6:
`rotation_id`, old and new fingerprints, `state`
(`db_committed_pending_activation` | `complete`), timestamps, backup filename,
and the custody disposition.

**Tests**, in all three modes where a file is touched:
- a wrong key refuses before readiness;
- a missing key file over a sealed database refuses, and writes nothing;
- a missing key file over a never-sealed database generates one;
- the upgrade path in both directions: sealed rows that the key opens, and
  sealed rows that it doesn't;
- migration up, on a database from the previous release.

## Stage 2 — preconditions and `--check`

Rotation refuses **before creating anything** unless:
- the active, workspace, next, temporary and backup paths share one
  filesystem;
- the adapter's primitives work there (probe them);
- the key came from the file, not the environment (stage 1 of `0.91.0`
  already refuses `Env`; keep that test);
- the active key opens the sentinel;
- no workspace and no `.new` is left over.

**`admin rotate-key --check`** runs exactly these refusals and creates
nothing. This is the RFC's proposed answer 1, accepted with the RFC: a
read-only check, not a dry-run.

**Tests:** each refusal, asserted before any file exists (Simulated: no
operation recorded); `--check` records no operation in Simulated, on success
or refusal.

## Stage 3 — the journal

Unchanged from the RFC's *Preparation and the journal*:
- the workspace `<active>.rotation-work`, mode `0700`, no symlink, whose
  existence is the lock;
- `intent.write`, then checked write, fsync, readback and verification of
  both tags, then no-replace publication to `intent.json`, then a directory
  fsync;
- no key material in it;
- domain-separated tags derived from the old and new key;
- monotonic states `Intent -> NextTempVerified -> NextPublished ->
  BackupTempVerified -> BackupPublished -> Prepared`, each advanced through
  its own reserved temporary.

**Tests:** the lock; tag verification with each key; a partial
`intent.write` is non-authoritative and safely deletable; every state advance
is atomic, **in all three modes**.

## Stage 4 — startup recovery

**Before stage 5.** It implements every row of
[`crash-matrix.md`](./crash-matrix.md)'s recovery table, before normal
database open:
- authenticate the journal with whichever key opens the **sentinel**;
- act only on paths the journal binds;
- delete only bound partial temporaries;
- never regenerate a lost key under the old fingerprint;
- on any mismatch, fail closed before readiness and preserve every file.

**Tests:** every row, its state built directly. In the power-loss mode, the
rows are also entered by materializing the Simulated adapter's states at
every injection point. Each state must land in a row, and the real binary
must start consistently or refuse before readiness.

## Stage 5 — preparation

New-key and backup publication by the journal's protocol: bound temporary,
`create_new` `0600`, checked write, fsync, readback, fingerprint check,
`*TempVerified`, no-replace publication, directory fsync, `*Published`.
**The custody disposition is required before `Intent`**:
`RetainUntil { review_at, custody_reference }` or
`RemoveAfterVerified { verification_reference }`.

**Tests:** every injection point of the crash matrix's preparation list, in
all three modes; a fingerprint mismatch on readback; no `Intent` without a
disposition.

## Stage 6 — K07 extended, activation, K05

- **K07** reseals the sentinel with every sealed column, writes the
  `master_key_rotation` row as `db_committed_pending_activation`, and appends
  `admin.master_key.resealed`, all in its one Class-A transaction. It is then
  **made durable before it returns** (`0.91.0` stage 1, A2).
- **Activation** atomically replaces the active key path with `next.ready`
  (`rename`, never remove-then-rename), then fsyncs the directory.
- **K05** reopens with the new key, marks the row `complete`, and appends
  `admin.master_key.rotated` in one Class-A transaction, **made durable the
  same way**. The best-effort append at `cli.rs:671` is deleted.
- **CLI success prints only after K05 is durable.**

**Tests:**
- injection at K07's mutation, append, commit and checkpoint;
- around the replacement;
- K05 run twice: idempotent, with no duplicate event;
- no path prints success before K05 is durable;
- the audit log after a crash between K07 and activation says *resealed*,
  pending activation, and never *rotated* (M4).

## Stage 7 — custody, and the operator surface

- `K05` cannot mark `complete` without the disposition.
- A retained backup is never a startup candidate, and raises an operator
  finding after `review_at` until it is renewed or removed. A finding, never
  automatic removal: the RFC's proposed answer 2, accepted.
- Removal unlinks only after the referenced pre-rotation backup is retired or
  re-encrypted, and records the time and the operator.

Operator documentation: rotation, `--check`, recovery, custody. **Remove the
"not yet crash-safe" warning only in the change that closes M2c**, and replace
it with the recovery procedure.

**Tests:** the retained backup is not a candidate; the finding fires after
`review_at` and repeats; removal records identity.
