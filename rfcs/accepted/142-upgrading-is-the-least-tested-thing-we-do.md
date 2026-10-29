# RFC 142 — Upgrading is the operation we test least

**Status.** Accepted
**Accepted on.** 2026-10-29
**Approved by.** `@nabbisen`, 2026-10-29: *"RFC 142 is accepted with
your recommendations."* Both open questions are thereby settled on the
recommendations recorded in §Open questions — **all 37 shipped schema
versions plus the one hand-built `0.29.7`-origin fixture** for Q1, and
**D4 (a)** with its three refinements for Q2. They are repeated as
settled decisions in §Settled below so no reader has to reconstruct
them from a question heading.
**Independent design review.** [Review record, 2026-10-29](../handoffs/142-upgrading-is-the-least-tested-thing-we-do/design-review-2026-10-29.md),
which is **not** independent of this RFC, and the field says so: the architect
wrote the RFC and also wrote that review. `ROADMAP.md` §S1c prohibits
architect self-review by default and permits it where `@nabbisen` asks;
he asked, 2026-10-29: *"Review it carefully by yourself with my
fundamental philosophy."* §S1a places design review with him or an
external architect he engages; no external architect was engaged, and
his acceptance above followed the review rather than standing in for
one.
**Implementation owner.** The mid-capability model (implementation and
testing agent).
**Security review.** Required — a failed or half-understood upgrade of an
identity provider's database is an availability and integrity event, and
one of the measures proposed here touches what is written to disk before
a migration runs.

**Design prerequisites.** None.
**Implementation prerequisites.** None under RFC 000's status rule,
which this RFC now satisfies. **Stage 1 is a measurement and produces
no seed SQL** — see §Stages.
**Closure prerequisites.** A populated database at each covered
historical schema version migrates to head in CI with its rows and key
invariants intact; the supported upgrade range is stated in
`docs/src/guides/upgrade.md` and, where a floor exists, enforced in code
with a negative fixture; an operator can see what an upgrade would do
before it does it; and whatever D4 settles on is implemented with a test
that proves the refusal or the backup actually happens.
**Tracks.** Long-lived maintenance. Arises from a question about upgrade
safety, 2026-10-28.
**Touches.** `crates/sui-id-store/src/migrations.rs` and its tests,
`crates/sui-id/src/cli.rs`, `crates/sui-id/src/runtime/database.rs`,
`docs/src/guides/upgrade.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** The architect (high-capability model).

## Summary

Upgrading is the operation an operator performs most often and the one
this project exercises least. The **mechanism** is sound and this RFC
does not change it. What is missing is **evidence that it works across
the whole chain**, **a statement of what is supported**, and **a way for
an operator to find out what will happen before it happens**.

## Background — what already works, so this RFC does not re-propose it

Measured 2026-10-28 on `ff595aa`. Credit where the design is already
right:

- **Each migration runs in its own `Immediate` transaction, and the
  schema version is written in that same transaction** (`migrations.rs`
  `apply_migration`, RFC 112 D7). A failure part-way through 49
  migrations therefore leaves the database at a **well-defined earlier
  version**, not a half-migrated one, and the next start resumes.
- **A database the build does not understand is refused before anything
  is written** — `SchemaTooNew` and `SchemaVersionInvalid`, both with
  operator-facing lines, and the refusal names **which release last
  migrated the file** (`last_migrated_by`, `runtime/database.rs:83-92`).
- **The schema version is visible** — the admin Settings page
  (`handlers/settings.rs:328-339`) and `verify-backup`'s report
  (`cli.rs:130`).
- **`backup`, `restore` and `verify-backup` exist**, and the upgrade
  guide's procedure leads with a backup and warns that `cp` of a
  WAL-mode `.sqlite` can miss committed data.
- **A harness for testing a migration against an older schema exists**:
  `migrations::run_up_to(conn, N)`.

**None of that is the gap.** The gap is below.

## The gap, measured

**`run_up_to` is called for exactly two migrations.** Every call site is
in `tests_rfc021.rs`, for 0021 and 0022 — that RFC's own round. There
are **49 migrations**. **No test takes a populated database at an early
schema version to the current head.**

So the project's position today is: the chain is believed to work
because each link was reviewed when it was written, and because nobody
has reported otherwise. For a self-hosted identity provider whose
advertised install path is `cargo install sui-id`, where the operator
upgrades on their own schedule and may skip several releases, that
belief is carrying more weight than the evidence behind it.

Two smaller gaps follow from the same place:

- **Nothing states which upgrades are supported.** `check_supported`
  refuses a schema **newer** than the build; there is no floor and no
  claim. Whether `0.76 → 0.86` in one step is supported is currently a
  guess — a reasonable one, since migrations are forward-only from 0001,
  but a guess.
- **Nothing lets an operator look before leaping.** There is no way to
  ask a binary what it would do to a database. The first thing that
  happens is the thing itself.

## Design

### D1 — a migration corpus, and it is the one that matters

**For each covered historical schema version: build a populated database
at that version, migrate it to head, and assert the data survived.** In
CI, as a gate.

**Populate it by construction, not by checking in binaries.**
`run_up_to(conn, N)` brings a fresh database to version `N`; per-version
seed SQL then inserts rows using the schema *as it was at N*. No binary
fixtures in git, and the corpus stays honest as migrations change,
because a renumbering breaks the helper loudly — `sql_for_version`
already panics on a missing version for exactly that reason.

**Both helpers are `#[cfg(test)] pub(crate)`** (`migrations.rs:623` and
`:641`), so the corpus lives in `sui-id-store`'s own test tree, not in an
integration test, which is a separate crate and cannot reach them.

**Assert more than "it did not error."** Per covered version: row counts
per table before and after, the specific invariants each intervening
migration claims to preserve, and that the final schema version equals
`MAX_SCHEMA_VERSION`. A migration that silently drops rows passes a
"no error" test and fails this one.

**Cost, stated plainly:** reaching a version is free — `run_up_to`
replays with current code, so no release needs checking out or
building. The cost is **per-version seed SQL**. It is *not* proportional
to the version count, because one seed script stays valid across every
consecutive version in which the tables it writes are shape-stable: a
script covers a **range**, not a point.

**D1's first deliverable is therefore a measurement, not code** — the
seed-script range boundaries across the covered versions, reported
before any seed SQL is written. See the open questions.

**Replay is faithful only while migration SQL is immutable after
release, and once it was not.** Measured 2026-10-29 across all 49: three
files have a second commit. `0009` changed only its comment block and
`0044`'s two commits are both inside `0.79.0`, so neither ever shipped
divergent. **`0021` did.** Its `0.29.7` text rebuilt `users`,
`credentials`, `clients`, `signing_keys` and `user_totp` in place,
relying on `PRAGMA foreign_keys = OFF` — a no-op inside a transaction,
so `DROP TABLE users` fired `ON DELETE CASCADE` across the child tables.
`0.29.8` removed the rebuilds. **Both record `schema_version = 21`**:

> The stored schema version is not a unique key for the schema's actual
> shape. There is one historical case where two databases report the
> same version and differ structurally.

The window is bounded — `0022_boolean_checks.sql` rebuilds the same five
tables safely and **converges both lineages**, so they are identical
from schema 22 onward, and `0.29.7`/`0.29.8` are the same day. But the
corpus must contain **one hand-built fixture for the `0.29.7`-origin
schema-21 shape**, because it is the single starting state replay cannot
reproduce, and therefore the only fixture in the set that could still
fail.

### D2 — state the supported range, and enforce a floor if one exists

Once D1 runs, we will know whether every covered version reaches head.

- **If all do**: say so in `docs/src/guides/upgrade.md` — any supported
  release upgrades directly to any later one, no stepping-stone
  required. That sentence is worth more to an operator than any amount
  of mechanism.
- **If some do not**: that is a defect to fix, or a floor to declare.
  A declared floor gets a `SchemaTooOld` refusal **symmetric with
  `SchemaTooNew`** — refused before anything is written, naming the
  version found, the floor, and the release to upgrade through first.
  With a negative fixture, like every refusal this project has.

**Do not declare a floor to avoid fixing a migration.** A floor is a
statement that an upgrade path is unsupported, and it strands whoever is
below it.

### D3 — let an operator see what will happen

A read-only subcommand that opens nothing for writing and reports: the
stored schema version, the release that last migrated the file, the
list of migrations that **would** apply, and the version it would end
at. Then exits.

This is the second half of the project's philosophy applied to the
riskiest routine operation an operator performs. Today the way to find
out what an upgrade does is to do it.

**It must not write.** Not the version, not `sui_meta`, not a WAL frame
it did not need. The existing `database::check` path already refuses
without opening for writing and is the model.

### D4 — the backup the guide already asks for

The upgrade guide's first step is a backup. **Nothing enforces it**, and
the failure a backup protects against — a migration that corrupts or
loses data — is precisely the one that cannot be undone afterwards.

Two shapes, and I do not think this is mine to pick:

- **(a) Take one automatically** before the first migration of a run,
  to a path derived from the database path, with `--no-backup` to
  decline. Costs disk and time on every upgrade, including the many
  that are trivial, and an operator who already backs up gets a
  duplicate.
- **(b) Refuse to migrate without one**, unless the operator passes a
  flag asserting they have one. Costs nothing and helps nobody who
  ignores the prompt; it converts a documented step into an enforced
  one, which is honest but adds friction to every upgrade.

**My recommendation is (a)**, because the operator who most needs the
backup is the one who did not read the guide, and (b) does not help that
person. But (a) writes to an operator's disk without being asked, which
is the kind of thing that should be a deliberate decision rather than an
implementation detail.

## Alternatives rejected

- **Down migrations.** Forward-only is deliberate and should stay. A
  down migration that cannot restore dropped data is worse than no down
  migration, because it looks like a way back. Restoring a backup is the
  way back, which is why D4 matters more than this would.
- **Checking in binary fixture databases.** Faster to write and
  immediately stale: a fixture built once stops reflecting what the
  schema at that version actually was the moment anything is corrected.
  D1's construction approach fails loudly instead.
- **Testing only the newest few versions.** The operator most at risk is
  the one who upgrades least often, which is exactly the operator this
  would not cover.

## Settled

**Both questions below were settled by `@nabbisen` on 2026-10-29** —
*"RFC 142 is accepted with your recommendations."* They are recorded
here as decisions so that no reader has to infer a decision from a
heading that still says "questions".

- **Coverage.** The corpus covers **all 37 shipped schema versions**,
  built by replay, **plus one hand-built fixture** for the
  `0.29.7`-origin schema-21 shape — **38 in total**. It is generated,
  not committed: no binary `.db` files in the tree.
- **D4.** Shape **(a)**, with its three refinements: act only when a
  migration will actually run; `VACUUM INTO` rather than a file copy;
  free-space check before writing. A plain snapshot beside the
  database, not the encrypted `backup.rs` path, so a restore needs
  `sqlite3` and nothing else.

## Stages

**Stage 1 — measure, and build nothing.** Produce the seed-script
range table: for each of the 37 shipped schema versions, the shape of
the tables a seed must write, and the boundaries where that shape
changes. The deliverable is the table and the resulting count of seed
scripts. **No seed SQL, no fixtures, no gate.** This stage exists
because the cost of D1 is the number of *ranges*, which is currently
unknown, and committing to seed authorship before counting them is the
error this RFC's own Q1 already made once.

**Stage 2 — the corpus.** The replay generator, one seed script per
range, the 37 fixtures, the hand-built `0.29.7` fixture, and D1's
assertions (per-table row counts before and after, each intervening
migration's claimed invariant, final version equals
`MAX_SCHEMA_VERSION`). Becomes a gate.

**Stage 3 — D2.** Only after stage 2 reports. State the supported
range in `docs/src/guides/upgrade.md`; if and only if some version
does not reach head, either fix it or declare a floor with a
`SchemaTooOld` refusal symmetric with `SchemaTooNew` and a negative
fixture. **A floor is not to be declared to avoid fixing a migration.**

**Stage 4 — D3.** The read-only preview subcommand. It must not write.

**Stage 5 — D4.** The backup, in the settled shape above, with a test
that proves the backup actually happens and a test that proves it is
skipped when nothing is pending.

**Release scheduling is not settled here.** It is proposed separately,
under the standing arrangement recorded in `ROADMAP.md`.

## Open questions for the approver

> **All settled 2026-10-29 — see §Settled above.** The two entries are
> kept below because the reasoning that produced them, and the
> correction of the first recommendation, are part of the record.



1. **Which historical versions does the corpus cover?**
   **Recommendation revised 2026-10-29 after measuring; the earlier
   answer in this RFC was wrong.** It had been "four releases — the
   `0.76` line's first published version, one mid-point, and the two
   most recent minors". That samples the wrong axis and misplaces the
   cost. An upgrade starts from a recorded **schema version**, not a
   release: mapping all **137 tags** to the highest migration each
   carries gives **37 distinct shipped schema versions** (1–7, 9–13,
   15–18, 20–26, 28–34, 36, 38, 43, 44, 46, 48, 49). Four releases
   cover 4 of those 37. Twelve of the 49 migrations never shipped at
   all (8, 14, 19, 27, 35, 37, 39–42, 45, 47), so "every migration"
   would be wrong in the other direction — no operator can be at 41.

   **Recommendation: all 37, plus the one hand-built `0.29.7`-origin
   schema-21 fixture — 38.** Coverage is bounded by seed-script
   *ranges*, not by version count, so sampling saves much less than the
   earlier answer assumed while giving up 33 of 37 starting points.

   **Generate the corpus; do not commit it.** A sampled corpus needs
   someone to remember to extend it; a derived one cannot fall behind —
   when migration `0050` lands the corpus grows by one with no edit.
   That is the shape `scripts/local-matrix.sh` already uses against
   `ci.yml`, and G20 against `commands.rs`.
2. **D4: (a) or (b)?** **Recommended (a)**, refined three ways after
   review, each of which removes one of (a)'s stated costs:
   **(i) act only when a migration will actually run** — the common
   startup has zero pending and must then do nothing at all, which
   retires the duplicate-backup objection; **(ii) `VACUUM INTO`, not a
   file copy** — `db.rs:43` sets `journal_mode = WAL`, and a
   filesystem copy of a live WAL database is not a valid backup;
   **(iii) check free space against the database size first** and
   refuse naming both numbers, so the safety measure cannot fill the
   disk and cause the failure it prevents.

   (b)'s flag is a lie detector that always passes, and any container
   restart would carry it permanently — at which point (b) is (a)
   without the backup.

   **One sub-decision to make deliberately:**
   `crates/sui-id-store/src/backup.rs` (RFC 040 / RFC 075) already does
   encrypted backup and restore, and two backup concepts would be a
   mistake. But an encrypted snapshot is restorable only with the key,
   at the worst possible moment. **Recommendation: a plain
   `VACUUM INTO` snapshot beside the database**, so a panicking
   operator can restore it with `sqlite3` and nothing else — recorded
   here as a choice, not an accident.
