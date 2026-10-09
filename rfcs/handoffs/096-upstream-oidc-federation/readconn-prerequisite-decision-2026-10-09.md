# `ReadConn` — a declared prerequisite that did not exist, and the decision taken

**Recorded.** 2026-10-09 JST, by the architect.
**Status.** **Settled.** `@nabbisen` chose option 1 on 2026-10-09: build it, as
096-B1's stage 0. **This document records the finding and the decision, not an
open item.**
**Why it is in git now.** It was raised in `.git-exclude/` because the repo is
public and `.github/SECURITY.md` forbids publishing an unfixed finding. With the
decision taken, the gap is scheduled work rather than an unfixed weakness, so
the reasoning belongs where a reader can find it.
**Placement note.** This is in the handoff package rather than amended into RFC
096's body, deliberately: an amendment shifts every subsequent line and
invalidates the bare `:NNN` citations the project uses heavily — 61 of them had
to be remapped for the `exp` amendment hours earlier. Say if you would rather it
sat in the RFC body and I will amend and remap.

## The finding

RFC 096 `:89-93` names the M2a foundation 096-B1 requires:

> the sealed `declare_write_command!` capability, `ReadConn`, the command
> manifest, and both the `WriteTx<Protocol>` and `WriteTx<AtomicAudit>` runners

Measured on 2026-10-09, while checking 096-B1's prerequisites **before** writing
its stage plan rather than after:

| Piece | State |
|---|---|
| `declare_write_command!` | present, 39 references in `sui-id-store/src` |
| `WriteTx<Protocol>` runner | present and complete — `Database::protocol()`, `registry.rs:897-914` |
| `WriteTx<AtomicAudit>` runner | present — `registry.rs:701`, `:793` |
| command manifest, F01–F06 | present as target-state rows, `contracts/write-commands.toml:794+` |
| **`ReadConn`** | **absent — zero references in `crates/` and `contracts/`** |

It is not a renaming. Reads go through `db.rs`'s `with_conn` / `with_conn_sync`,
which return an **untyped** connection: nothing in the type separates a read
path from a write path. Neither of the two controls RFC 094's amendments attach
to `ReadConn` exists — the per-statement `sqlite3_stmt_readonly` interrogation
(its 2026-08-12 amendment, *"a **required** M2a control"*) or the assertion that
`rusqlite`'s `functions`, `vtab` and `load_extension` features stay disabled
(its 2026-08-26 amendment, *"a required M2a assertion"*, added because
statement-level read-only status does not constrain side-effecting application
functions or virtual tables).

**Severity, as assessed:** a missing defence-in-depth control, not a live
exploitable defect. The absence means no *enforced* guarantee that a read path
cannot mutate — not that one does. Confirming that is the separate audit below.

## Why M2a closed with it missing

**M2a's seven consolidated closure criteria never mention `ReadConn`.** All
seven concern the write path: the transaction seam, rollback on injected append
failure, exactly-once evidence, C15 atomicity, the structural gate, the coverage
matrix, and `rusqlite` confinement to `sui-id-store`.

So **M2a closed legitimately against its own criteria**, while RFC 094's
amendment prose declares controls those criteria do not require.

**This is the second instance of the same divergence in RFC 094.** On 2026-10-03
its M2a exit criteria were consolidated precisely because two lists had diverged
*"in both directions, which meant M2a could be closed against either and look
complete"* (`../094-transactional-audit/m2a-exit-criteria-divergence-2026-10-03.md`).
That consolidation reconciled `ROADMAP.md`'s cell with the RFC's criteria list.
**It did not reconcile the RFC's amendment prose with that list**, and this fell
through the remaining gap.

M2a's closure is **not** reopened. It satisfied the seven criteria it was closed
against, four of them gate-asserted.

## The decision

**Option 1, authorized 2026-10-09: build `ReadConn` as 096-B1 stage 0** — the
typed wrapper, the per-statement interrogation, and the feature assertion,
before any attempt-state work. 096-B1 is the stage that finally routes live
traffic through 096-A's validation and establishes sessions; building it on a
prerequisite its own RFC names and that is absent was the wrong trade.

Rejected: amending `:89-93` to drop `ReadConn` and scheduling it separately —
defensible, since 096-B1's own reads are of its attempt rows, but it leaves the
declared-versus-built gap open and out of sight. Also rejected: treating it as an
M2a defect and reopening that closure, which would cost more than the control.

## Two follow-ups

**1. The read-path audit**, authorized as its own task: does anything reached
from a read-shaped call site perform a write? Honest scope is every `with_conn`
call site in the workspace. **Sequenced before stage 0**, not after — its output
is precisely the input stage 0 needs, namely which call sites are reads. The
authorization was for a separate task, which this remains; only the ordering is
the architect's.

**2. RFC 094's prose and its closure criteria want reconciling** so a third
instance cannot occur. This is a proposal, not a decision. Amending RFC 094 needs
`@nabbisen`'s go-ahead, and it is raised in the 096-B1 stage plan's schedule
rather than acted on here.
