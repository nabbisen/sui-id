# RFC 142 — design review record

**Date.** 2026-10-29
**Reviewer.** The architect (high-capability model), at `@nabbisen`'s
request: *"Review it carefully by yourself with my fundamental
philosophy."*

**Outcome.** `@nabbisen` accepted RFC 142 on 2026-10-29 with these
recommendations — *"RFC 142 is accepted with your recommendations."*
The RFC's §Settled records what that decided. This file is the
reasoning behind it, kept because the first recommendation on Q1 was
wrong and the correction is part of the record.

**This review also answered a question about RFC 140 §5 (the consent
screen logo: upload, not server-side fetch). That part is not
reproduced here** — it is recorded in RFC 140 §5 itself, which is a
separate, still-open decision.
**Self-review.** The RFC is mine. `ROADMAP.md` §S1c prohibits
architect self-review by default and permits it where `@nabbisen` asks;
he asked, in the words above. §S1a still places **design review** with
him or an external architect he engages. **Nothing in this file was a decision** when it was written; RFC 142
was **Proposed** and build-prohibited under RFC 000. The acceptance
above came afterwards.

---

## Verdict

| | My earlier recommendation | After measuring |
|---|---|---|
| **142 Q1** — corpus coverage | four releases | **wrong** — all **37 shipped schema versions** by replay, **+1** hand-built; cost is seed scripts, which span ranges |
| **142 Q2** — D4 backup | (a) | **(a) still**, refined three ways |

---

# Part 1 — RFC 142 Q1: which historical versions the corpus covers

## My recommendation of four releases was wrong on both axes

**It samples the wrong axis.** The migration runner replays forward from
the version recorded in `sui_meta.schema_version`. The starting state of
an upgrade is therefore a **schema version**, not a release. Mapping all
**137 tags** to the highest migration each one carries:

> 37 distinct schema versions have ever shipped: 1–7, 9–13, 15–18,
> 20–26, 28–34, 36, 38, 43, 44, 46, 48, 49.

Four releases cover **4 of 37** starting points and say nothing about
the other 33. Note also that **12 of the 49 migrations never shipped in
any release** (8, 14, 19, 27, 35, 37, 39–42, 45, 47) — they were
intermediate within a development cycle. So "every migration" would be
wrong too, in the other direction: no operator can be sitting at 41.
**37 is the real population**, and it is knowable exactly rather than
estimated.

**It misplaces the cost — including against my own D1.** D1 already
specifies replay: `run_up_to(conn, N)` brings a fresh database to
version *N* with today's code, so **no old checkout or old binary is
needed for any of the 37**. Reaching a version is free. What costs is
the other half of D1: **per-version seed SQL**, which must insert rows
using the schema *as it was at N*.

So the real question is not "how many versions can we afford to reach"
but **"how many distinct seed scripts do 37 versions actually need"** —
and that is fewer than 37, because a seed script stays valid across
every consecutive version in which the tables it writes are
shape-stable. One script covers a range, not a point.

**I am not going to guess that number.** Counting it means diffing the
seeded tables' shape across 37 versions, which is stage 1's first task
and exactly the kind of figure I have previously gotten wrong by
projecting one measurement onto an inventory. **D1 should measure the
range boundaries and report them before any seed SQL is written.**

**With the cost in the right place, the recommendation inverts:** cover
all 37, because coverage is bounded by seed-script *ranges*, not by
version count, and sampling four saves far less than I assumed while
giving up 33 of 37 starting points.

## The validity condition, and the one place it fails

Replay is only faithful if **migration SQL is immutable after release**.
Measured across all 49: three files have more than one commit.

| Migration | Second commit | Divergent in the field? |
|---|---|---|
| `0009_audit_hash_chain` | `fc356bb` (RFC 125) | **No** — comment block only; SQL body unchanged |
| `0044_forgot_password_requests` | `6af817b` | **No** — both commits inside `0.79.0`, never shipped apart |
| `0021_schema_invariants` | `8b4fadf` | **Yes, materially** |

**`0021` is a real divergence.** The `0.29.7` text rebuilt `users`,
`credentials`, `clients`, `signing_keys` and `user_totp` in place to add
boolean `CHECK`s, relying on `PRAGMA foreign_keys = OFF` — which SQLite
documents as a no-op *inside a transaction*, and each migration runs in
one. `DROP TABLE users` therefore fired `ON DELETE CASCADE` across the
child tables. `0.29.8` removed the rebuilds and kept only the three
additive changes.

**Both record `schema_version = 21`.** So:

> **The stored schema version is not a unique key for the schema's
> actual shape.** There is at least one historical case where two
> databases report the same version and differ structurally.

That sentence is the finding. It is the kind of thing RFC 142 exists to
catch, and no test, gate or document states it today.

**The exposure is bounded, and I should say so rather than dramatise
it.** `0022_boolean_checks.sql` rebuilds the same five tables properly
(`MIGRATION:FK_DISABLE_REQUIRED`, PRAGMA set *before* `BEGIN`), so it
**converges both lineages** — a `0.29.7`-origin database and a
`0.29.8`-origin one are identical from schema 22 onward. The window is
exactly *"a database that stopped at schema 21 having run `0.29.7`"*,
and `0.29.7` and `0.29.8` are the same day, 2026-05-09.

## The corpus, therefore

**38 fixtures: 37 by replay, plus one hand-built.** The hand-built one
is the `0.29.7`-origin schema-21 shape — the single point where replay
tells a comfortable lie. Building it from the `0.29.7` text of `0021`
and proving it reaches head through `0022` is the whole point: it is the
only fixture in the set that could still fail.

## Generate it; do not commit it

- **Derived, not sampled.** A sampled corpus needs a human to remember
  to extend it. A derived one cannot fall behind: when migration `0050`
  lands, the corpus grows by one with no edit and no reminder. This is
  the same shape as `scripts/local-matrix.sh` deriving its set from
  `ci.yml`, and G20 deriving the declared command set from
  `commands.rs`. The project already prefers this; the corpus should
  not be the exception.
- **No binary `.db` files in the tree.** They are unreviewable in a
  diff, and they rot silently against the code that reads them.

## What this changes for D2

D2 asks whether every covered version reaches head. With four fixtures
that is an inference; with 37 it is a statement. **A floor, if one
exists, gets discovered rather than guessed** — which matters, because
D2 already warns against declaring a floor to avoid fixing a migration,
and only full coverage makes that warning enforceable.

---

# Part 2 — RFC 142 Q2 (D4): (a) or (b)?

**(a) stands**, refined in three ways that remove its stated costs.

**1. Only when a migration will actually run.** D4 as written takes a
backup "before the first migration of a run", and I costed that as
"disk and time on every upgrade, including the many that are trivial".
That cost is avoidable: the overwhelmingly common startup has **zero**
pending migrations and must then do **nothing at all** — no copy, no
message, no flag. The duplicate-backup objection disappears, and so does
most of the friction argument for (b).

**2. `VACUUM INTO`, not a file copy.** `db.rs:43` sets
`journal_mode = WAL`. A filesystem copy of a live WAL database is **not
a valid backup** — it can miss committed frames. The backup must be a
consistent snapshot that fails loudly rather than a `std::fs::copy`,
which is exactly what would otherwise be written.

**3. Check free space before writing anything.** Compare free space
against the database size and refuse up front, naming both numbers.
Otherwise the safety measure half-writes a copy, fills the disk, and
causes the class of failure it was added to prevent.

**Why not (b).** Its flag — "I have a backup" — is a lie detector that
always passes, and any container or service restart would have to carry
it permanently, at which point (b) has degraded into (a) *without the
backup*. It converts a safety property into a box to tick. That is a
control built so an operator can misunderstand it, which is the half of
his philosophy I am supposed to weigh equally.

**One thing I am not deciding, and flagging instead.**
`crates/sui-id-store/src/backup.rs` (RFC 040, refactored RFC 075)
already implements **encrypted** backup and restore, and already refuses
a backup taken on a newer sui-id. Two backup concepts would be a
mistake. But an encrypted pre-migration snapshot is restorable only with
the key, at the worst possible moment. **My recommendation is a plain
`VACUUM INTO` snapshot beside the database**, precisely so a panicking
operator can restore it with `sqlite3` and nothing else — but the
encrypted path exists, and choosing *not* to reuse it should be a
deliberate decision in D4, not an implementation accident.

---

# Status and what happens next

- **Accepted 2026-10-29.** The RFC moved to `rfcs/accepted/` and its
  §Stages were written from this review: **stage 1 is a measurement
  and produces no seed SQL** — the seed-script range boundaries across
  the 37 versions — because committing to seed authorship before
  counting the ranges is precisely the error Q1 already made once.
- **Q1's original recommendation in the RFC was wrong and is corrected
  in the document itself**, so the accepted text matches the
  measurement rather than the earlier estimate.
- **D2's floor question cannot be answered before stage 2 reports.**
