# RFC 142 stage 1 — count the ranges, build nothing

**RFC.** [142 — Upgrading is the operation we test least](../../accepted/142-upgrading-is-the-least-tested-thing-we-do.md)
**Status of that RFC.** **Accepted** 2026-10-29. Implementation is
permitted under RFC 000's status rule.
**Stage.** 1 of 5. See the RFC's §Stages.
**This stage writes no seed SQL, no fixtures and no gate.** If you
finish it and nothing is in `crates/`, that is the expected outcome.

---

## 1 — why this round is a measurement

The corpus in D1 needs, for each covered schema version, a seed that
inserts rows **using the schema as it was at that version**. The cost
of D1 is therefore the number of *seed scripts*, and a seed script
stays valid across every consecutive version in which the tables it
writes keep the same INSERT shape. One script covers a **range**, not
a point.

**Nobody has counted the ranges.** The RFC's first recommendation
sampled four releases precisely because that number was unknown and I
assumed it was large. It was corrected once already; it will not be
guessed a second time. **Count first, then we scope stage 2.**

## 2 — the deliverable: one table, and a number

For the covered schema versions, a table with one row per **range**:

| Range | Seeded tables in scope at the range start | Boundary migration that opens it, and why | Boundary that closes it |
|---|---|---|---|

Plus the headline: **how many seed scripts the corpus needs.**

**Covered versions: re-derive the set yourself; do not take mine.** I
measured 37 distinct shipped schema versions by mapping all 137 tags
to the highest migration each carries. State the number you get and,
if it differs from 37, say where. Twelve of the 49 migrations never
shipped in any release, so the covered set is **not** `1..=49`.

## 3 — how to decide a boundary

A migration opens a new range if it changes whether an existing seed's
`INSERT` would still work or still mean the same thing on the tables
in scope. Examples, not an exhaustive rule:

- **Opens one:** `ADD COLUMN ... NOT NULL` with no default; a new
  `CHECK` the seeded values could violate; a table dropped, renamed, or
  rebuilt with a different column set; a `NOT NULL` tightened.
- **Does not:** a nullable `ADD COLUMN`; a new index; a change to a
  table not in the seeded set.
- **Grows the set:** `CREATE TABLE` for a table whose data an operator
  would miss if an upgrade lost it.

**State the seeded-table set and justify it.** The corpus exists to
prove data survives, so the set is "tables holding operator or end-user
data", not every table. Write down what you included and what you left
out.

## 4 — four things to settle, which the table will not

1. **Can the boundaries be derived mechanically from the migration SQL,
   or do they need human judgement?** This matters more than the count.
   If mechanical, stage 2's generator computes them and the table is a
   report that can never go stale — the shape `scripts/local-matrix.sh`
   uses against `ci.yml` and G20 against `commands.rs`. If judgement is
   needed, say which decisions need it; those are the ones that rot.
2. **Does the seeded set include `audit_log`?** Migration `0009` puts a
   hash chain on it, so seeding it means producing rows whose chain
   verifies — materially more work than an `INSERT`, and it may deserve
   its own fixture rather than a place in every seed. Recommend one way
   and say why.
3. **Confirm the `0.29.7` lineage.** The original `0021` text is the
   blob at `68ad4f3`; `8b4fadf` replaced it. Confirm which tags contain
   each, and therefore that the only divergent starting state is
   "schema 21, reached by `0.29.7`". This is the hand-built fixture
   stage 2 will need, and stage 1 should prove it is exactly one
   fixture and not more.
4. **Is any covered version unreachable by `run_up_to` from an empty
   database?** Expected: none. Confirm rather than assume.

## 5 — what not to do

- **No seed SQL, no fixtures, no gate, no CI change.** Stage 2.
- **Do not change `migrations.rs`.** If measuring needs a helper,
  say so in the return and leave it unwritten.
- **Do not try to make `run_up_to` replay historical SQL.** It replays
  today's `MIGRATIONS`, which is correct and is the whole reason the
  `0.29.7` fixture is hand-built. Changing it would hide the one
  divergence the corpus exists to catch.
- **Do not widen scope to D2, D3 or D4.** D2 in particular cannot be
  answered before stage 2 reports.
- **No `CHANGELOG` entry.** This stage ships nothing to an operator.

## 6 — things worth knowing before you start

- `run_up_to(conn, max_version)` — `migrations.rs:623` — and
  `sql_for_version(version)` — `:641` — are both
  `#[cfg(test)] pub(crate)`. **The corpus therefore lives in
  `sui-id-store`'s own test tree**, not an integration test, which is a
  separate crate and cannot reach them. Worth confirming now, because
  it constrains stage 2's file layout.
- `run_up_to` uses the same `apply_migration()` as `run()`, so the
  `MIGRATION:FK_DISABLE_REQUIRED` path in `0022` is handled identically.
- The RFC's §Settled records what was decided and what was not. Read it
  before the design sections; the open-questions section below it is
  kept for its reasoning, not as live questions.

## 7 — what to return

A single document: the range table, the seed-script count, the covered
version set with its derivation, answers to §4's four points, and
anything the measurement turned up that changes stage 2's shape. If the
count comes out high enough to make the full 38-fixture corpus look
wrong, **say so** — the coverage decision was taken on the assumption
that ranges are far fewer than versions, and if that is false it is
escalated for a fresh decision, not absorbed quietly.
