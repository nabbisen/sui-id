# RFC 100 — the crash-injection harness, for approval

**Proposed.** 2026-11-14, by the architect. This is cycle M's D1.
**What it is for.** RFC 100's last implementation prerequisite, *"the
crash-injection harness approved"* (RFC 100 `:10-15`). With it approved, the
build can be dispatched for `0.92.0`.
**Whose turn.** Yours, for the two decisions in §8.
**Measured on.** `7c5cf93`.
**The prototype.** `.git-exclude/prototypes/rfc100-power-loss-model/`
(`src/main.rs`, std only, 242 lines), with its output in
`results-2026-11-14.txt`. It is a model of the filesystem, not the harness,
built to test this design before proposing it.

## 1 — what the harness must show

RFC 100's acceptance criteria: *every recovery-table row has an executed
crash-injection test*, and *no prefix produces mixed ciphertext, a missing
active key, or an overstated audit record*. Also its six invariants M1–M6
(`crash-matrix.md`), at every injection point.

## 2 — three failure modes, not two

RFC 100's test plan injects **short writes** and **process termination**
(`:300`). Neither one loses data that was written but not synced. A
killed process's writes stay in the page cache, and the kernel still writes
them out. The failures the refresh measured are **power losses**. So the
harness adds a third mode: **loss of everything not yet synced**.

**The prototype, on today's rotation order** (`cli.rs:621-660`). It counts
the states each model can leave, and how many break a property:

| Rotation | Model | States | M2 broken¹ | Wrong after success² |
|---|---|---|---|---|
| `0.90.0` order | kill | 6 | **0** | **0** |
| `0.90.0` order | power loss | 63 | **23** | **30** |
| stage 1's syncs | kill | 10 | 0 | 0 |
| stage 1's syncs | power loss | 24 | 0 | **1** |
| stage 1's syncs + A2's checkpoint | power loss | 19 | 0 | **0** |

¹ M2: some file still holds the key the database is sealed under.
² After the command reports success, the active path holds that key.

**The kill model passes the `0.90.0` order, which the power-loss model breaks
in 30 states.** A harness without the third mode would pass code with no
fsync at all.

**It also catches each fsync on its own.** With A2 in place, the prototype
removes one sync at a time, then reruns:
- the key file's `sync_all`: 8 M2 breaks;
- the directory sync before the reseal: 4;
- the directory sync after the last rename: 1 wrong after success;
- the directory sync after the first rename: **none**. In one directory, the
  final directory sync persists both renames. That sync is harmless, but it
  is not load-bearing for any property this checks. It stays in stage 1;
  the model only shows it is not what makes rotation safe.

## 3 — what the prototype found that the RFC had wrong

**The database is not durable when K07 returns.** `sui-id-store/src/db.rs:43-44`
opens connections with WAL and `synchronous=NORMAL`. SQLite says a commit in
that mode *"might roll back following a power loss or system crash"*. It
becomes durable at the next checkpoint. Two documents assumed otherwise, and
both are the architect's:
- RFC 100's refresh, prefix 1 (`:82-89`): *"The database's commit is
  durable"*;
- stage 1's premise, *"SQLite fsyncs that commit"*. That line is now
  corrected in place.

Hence the one state stage 1 leaves wrong: the reseal rolls back while the new
key is already active. **Already dispatched** as stage 1's part A2: a
`wal_checkpoint(TRUNCATE)` inside K07, after commit and before return —
[`0.91.0-stage-1-addendum-the-reseal-is-not-durable-on-return-2026-11-14.md`](0.91.0-stage-1-addendum-the-reseal-is-not-durable-on-return-2026-11-14.md).

**For RFC 100's build, this is a fourth prefix.** The state machine's
`DbCommitted` must mean *durable*, not only *committed*. K05 needs the same
treatment as K07.

## 4 — the design

**One platform adapter, three implementations.** RFC 100 already requires the
adapter (`implementation.md`, stage 1): durable create, atomic replacement,
atomic no-replace publication, and file and directory fsync. The harness
makes it the **only** way rotation and recovery touch the key files.

| Implementation | Where | What it does |
|---|---|---|
| **Real** | production | `std` only. `OpenOptions::create_new` with mode `0600`, `sync_all`, `rename` (atomic replacement), `hard_link` then unlink (atomic no-replace: `link(2)` refuses an existing name), and `File::open(dir)?.sync_all()`. Owner checks compare against the owner of the workspace this process just created, so no `geteuid`, no `unsafe`, and **no new dependency** |
| **Failpoint** | tests, process termination and short writes | wraps Real. At a named point, given by an environment variable read only under a test-only cargo feature, it either aborts the process (`std::process::abort`), or makes the write return short or fail. Driven through the **real binary**, as `tests/e2e/key_rotation.rs` already does |
| **Simulated** | tests, power loss | records every operation per inode and per directory entry, and what each sync made durable, as the prototype does. At each crash point it lists every disk a power loss could leave |

**The database takes part in the power-loss mode.** Rotation tells the
adapter when K07 (later K05) commits, and when it is durable. These are no-ops
in Real. In Simulated, a commit is pending until the checkpoint, exactly like
an unsynced directory entry.

**From a simulated state to a real test.** Each state the Simulated adapter
lists is **materialized** into a real temporary directory:
- the key files, as the state has them;
- the database file, as the snapshot taken before or after the reseal,
  according to whether that state kept the commit.

Then the **real binary starts against it**. The oracle is what it does:
- start against a consistent database;
- or refuse before readiness, with every file preserved.

It checks M1–M6, and M4 by reading the audit rows in the materialized
database. Recovery is therefore tested as built, not as modelled. Only the
crash itself is simulated.

**The crash model, stated so it can be reviewed.**
- Pessimistic POSIX. Only data that was synced, and directory entries whose
  directory was synced, are durable.
- Every unsynced operation may or may not have persisted. Directory
  operations that persisted did so in issue order.
- A write that persisted may have persisted partly, as a prefix. This one is
  **not in the prototype yet**, and the harness adds it.
- One filesystem. RFC 100 refuses to rotate across two.

**Not modelled:** a disk that lies about fsync, and SQLite's own corruption
bugs. Each is out of RFC 100's scope, and no test can repair either.

**Its cost.** States grow exponentially with the operations pending between
two syncs. The journal syncs after every step, so the set stays small: the
prototype's largest is 63, for code with no syncs at all. Everything runs
under `cargo test`, with no privilege and no special filesystem. So it runs
in CI on every change, not on a schedule.

## 5 — what approval settles, and what it does not

**Settles:** the three failure modes; the adapter as the only path to the key
files; Real with no new dependency; materializing simulated states and running
the real binary as the oracle; the crash model above.

**Does not settle:**
- **the build's stage plan.** D2 brings the RFC 100 handoff
  (`implementation.md`, `crash-matrix.md`) up to the refreshed design, and to
  this harness, after your approval, so the crash matrix names the modes you
  approve;
- **the closure.** That still needs an independent adversarial review of the
  evidence (RFC 100, *Closure prerequisites*).

## 6 — order in the build

The harness is built **first**, before any code it tests, and its first
commit runs it against today's rotation. It must show the numbers in §2's
table for today's order: the A2 order passes, and dropping a sync fails. Only
then is the journal built against it. That is the handoff's own rule,
*"recovery lands before the thing it recovers"*, applied to the harness.

## 7 — what this changes in RFC 100's text

**Its design: nothing.** The invariants, the state machine and the recovery
table stand.

**Two sentences would change**, and RFC 100 is accepted, so neither is the
architect's to edit alone:
1. *Test plan* (`:300`): *"Inject short writes and process termination"*
   becomes *"Inject short writes, process termination, and the loss of
   everything not yet synced"*. The crash matrix's *"Both failure modes"*
   becomes *"All three"*.
2. *Refresh*, prefix 1 (`:82-89`): replace *"The database's commit is
   durable"* with what is measured. Add the fourth prefix of §3, and its fix:
   stage 1's A2 now, `DbCommitted` meaning *durable* in the build.

Neither changes a design decision. Both widen evidence, or correct a
measurement. So the architect proposes recording them as a **non-material
amendment**, with your approval as its record. If you count either as
material, it goes through RFC 000's amendment path instead.

## 8 — your decisions

1. **Approve this harness**: §4's design, its three failure modes, and §6's
   order. Recommended. This meets RFC 100's last implementation prerequisite.
2. **The two sentences of §7**, as a non-material amendment to RFC 100.
   Recommended.
