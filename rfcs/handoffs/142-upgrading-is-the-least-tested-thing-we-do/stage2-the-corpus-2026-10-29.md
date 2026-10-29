# RFC 142 stage 2 — build the corpus

**RFC.** [142 — Upgrading is the operation we test least](../../accepted/142-upgrading-is-the-least-tested-thing-we-do.md), **Accepted**.
**Stage.** 2 of 5.
**Input.** [stage 1's measurement](./stage1-measurement-2026-10-29.md),
accepted — **read it first**, including its header, which records the
one correction made in review.
**This stage ships a gate.** It is the stage the RFC exists for.

---

## 1 — what to build

**38 fixtures. Each seeds a populated database at a covered starting
schema version, migrates it to head, and asserts the data survived.**

From stage 1, the seed material is **13 pieces**:

- **10 seed scripts**, one per range (stage 1's R1–R10 with R3 split
  at `0005`; R9 has no shipped starting version and needs none).
- **2 `audit_log` fixtures** — see §3.
- **1 hand-built `0.29.7` fixture** — see §4.

**The seeded set is 14 tables**, as amended in review. Take it from
the RFC's §Settled, not from the body of the stage 1 report, which
predates the correction.

## 2 — assertions, per D1

"It did not error" is not the test. Per fixture:

- **Row counts per seeded table, before and after.** A migration that
  silently drops rows passes a no-error test and must fail this one.
- **The specific invariant each intervening migration claims to
  preserve.** Where a migration's comment states a property, assert
  that property, not a proxy for it.
- **Final `schema_version` equals `MAX_SCHEMA_VERSION`.**

**At least one assertion must be a value, not a count.** A fixture
that only counts rows passes a migration that preserves the row and
corrupts the column. Seed distinguishable values and read them back.

## 3 — `audit_log`: two fixtures, and the smaller one matters more

Stage 1 established that `append_within_tx` (`repos/audit.rs:264`)
`SELECT`s `hash` and `INSERT`s `prev_hash, hash`, so it cannot run
against a database below schema 9. Hence:

- **Starting version ≥ 9** — seed through `append_within_tx`. Do not
  duplicate `compute_hash`; it is private to that module and a copy
  would drift.
- **Starting version < 9** — a plain `INSERT` with no hash columns.

**The second fixture is the point, not the leftover.** It is the only
thing in the corpus that exercises **migration `0009` against
pre-existing unhashed rows**. Assert all three of: the pre-existing
audit rows still exist after migrating to head; they are treated as
**legacy** rather than as corruption; and chain verification over the
resulting table does not fail wholesale because of them. **Write those
as named assertions.** Do not let them be implied by the fixture
merely existing — that is how a test ends up proving less than its
name claims, which is the failure RFC 125 was itself written about.

This is not re-proving RFC 125's correctness tests. Theirs ask whether
verification is right; this asks whether an **upgrade** across `0009`
preserves history.

## 4 — the `0.29.7` fixture

The only starting state replay cannot reproduce. Build it from the
original `0021` text — the blob at `68ad4f3` — and migrate it to head
through `0022`, which converges the two lineages.

**Assert what makes it worth having**: that a database carrying the
`0.29.7` shape (parent tables already rebuilt, with `CHECK`s) reaches
head intact, and that it ends structurally identical to a replayed
`0.29.8`-origin database at the same version. If those two do not
converge, that is a finding and stops the stage.

**Do not make `run_up_to` replay historical SQL** to generate this.
It replays today's `MIGRATIONS`, which is correct, and changing it
would hide the one divergence the corpus exists to catch.

## 5 — derive the ranges; do not hand-maintain them

Stage 1's §4.1 is the design, and it is the part of this stage most
worth getting right. The generator derives boundaries mechanically and
**stops and asks** on exactly three events:

1. a new table created — is it in the seeded set?
2. a new `CHECK` on a seeded column;
3. a `DROP COLUMN` on a seeded table.

"Asks" means **the build fails with a message naming the migration and
the question**, until a human records the answer somewhere the
generator reads. A warning nobody sees is not a question. When
migration `0050` lands, the corpus must either grow by itself or stop
and say why — never silently keep testing the old set.

## 6 — the gate

Add it to `contracts/gate-inputs.toml` and `.github/workflows/ci.yml`
in the established shape, and **register it in `scripts/local-matrix.sh`'s
derived set** — confirm the local matrix count rises by the number of
lanes you added, and say what it went from and to. A3.4 applies if you
touch a workflow's `uses:`.

## 7 — what not to do

- **Do not commit binary `.db` files.** The corpus is generated.
- **Do not widen to D2, D3 or D4.** D2 in particular is stage 3 and is
  answered *by* this stage's results, not alongside them.
- **Do not drop the dead `consents` table.** It is dead, stage 1 proved
  it, and dropping it is a migration — the thing this corpus exists to
  make safe. It is a follow-up gated on this stage landing, deliberately.
- **Do not change `migrations.rs`'s behaviour.** Adding a test-only
  helper is fine if needed; changing how migrations apply is not.

## 8 — `CHANGELOG`

**Yes, one entry**, under `Fixed`/`Changed` as fits: this is the first
release in which an upgrade from an old database is tested rather than
assumed. Say that in operator terms — what is now verified that was
not — and do not describe the harness.

## 9 — what to return

The usual package, plus: whether any of the 38 fixtures **failed on
first run**, and if so exactly what it found. That answer is the
reason the stage exists, and a clean sweep with nothing found should
be stated as plainly as a failure would be. If a fixture cannot reach
head, **stop and report rather than declaring a floor** — a floor is
stage 3's decision and the RFC forbids using one to avoid fixing a
migration.
