# RFC 142 stage 3 — state the supported range

**RFC.** [142 — Upgrading is the operation we test least](../../accepted/142-upgrading-is-the-least-tested-thing-we-do.md), **Accepted**.
**Stage.** 3 of 5. D2.
**Input.** [stage 2's record](./stage2-corpus-2026-10-29.md), accepted.
**Small stage.** Mostly one sentence, written carefully.

---

## 1 — stage 2 already answered the question

D2 asks whether every covered version reaches head. **It does** — all
37 shipped schema versions, with data intact, proven by G22 on every
run. So D2 takes its first branch:

> **If all do**: say so in `docs/src/guides/upgrade.md` — any supported
> release upgrades directly to any later one, no stepping-stone
> required. That sentence is worth more to an operator than any amount
> of mechanism.

**There is no floor, and none is to be declared.** The RFC's warning
against declaring a floor to avoid fixing a migration does not arise;
do not add a `SchemaTooOld` refusal, and do not add a negative fixture
for a floor that does not exist. If you find yourself writing one,
something has gone wrong — stop and say so.

## 2 — what to write

In `docs/src/guides/upgrade.md`, state plainly:

- **Any released version upgrades directly to any later one.** No
  intermediate release, no ordered sequence of upgrades.
- **What backs that claim**: every schema version the project has
  shipped is populated with data and brought forward to the current
  schema on every CI run. Say it in one clause, in operator language.
  Do not name `G22`, the fixture count, or the test module — an
  operator does not run those and should not have to learn them.
- **The one honest caveat**, in the operator's terms rather than ours:
  the guarantee is about the database. It does not promise that
  configuration-file keys or CLI flags never change between releases;
  the `CHANGELOG` remains the place for those.

**Write it for someone mid-upgrade at an awkward hour.** The second
half of the project's philosophy is the whole point of this stage —
the sentence exists so an operator does not have to guess whether
their four-releases-behind instance needs stepping stones.

## 3 — check the guide does not already contradict itself

Read the whole of `upgrade.md` before adding to it. If anything there
currently hedges — advice to upgrade one minor at a time, or wording
that implies an untested jump is risky — **it is now false and must
go**, not sit a paragraph above the new sentence. A guide that says
both things is worse than one that says neither.

Grep the rest of `docs/` for the same hedge; `docs/src/guides/` and
any operator-facing README are the likely places. Report what you
found and what you changed, even if the answer is nothing.

## 4 — what not to do

- **No floor, no `SchemaTooOld`** (§1).
- **Do not touch D3 or D4.** Stages 4 and 5.
- **Do not restate the corpus's mechanics in the guide.** The
  `CHANGELOG` entry from stage 2 already told operators the property
  in their own terms; this is the reference-doc counterpart, not a
  second announcement.
- **Do not drop the dead `consents` table.** Still a follow-up, still
  gated — and now gated on something that exists, so it is ready to be
  scoped separately rather than folded in here.

## 5 — `CHANGELOG`

**No new entry.** Stage 2's `[Unreleased]` entry already states the
property to operators; this stage documents it in the guide. Two
entries for one property would be noise.

## 6 — what to return

The usual package, plus the answer to §3: what the guide said before,
and whether anything in `docs/` had to be corrected rather than only
added to.
