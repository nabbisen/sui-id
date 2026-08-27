# RFC 094 M2a — implementation start

**Prepared:** 2026-08-27 (Asia/Tokyo)
**Prepared by:** requirements-architect role
**Audience:** implementation role
**Governing RFC:** [RFC 094](../../accepted/094-transactional-audit-registry.md), Accepted 2026-08-27
**Companion:** [`migration-checklist.md`](migration-checklist.md) — the staged task list

> **This document does not authorize coding.** One entry-gate item is unresolved
> and is `@nabbisen`'s to settle — see §1. Read everything below, raise anything
> that looks wrong, and start only when the owner has settled §1.

## 1. The one unresolved gate item

The handoff's entry gate read *"RFC 093 is Implemented"*. RFC 094's own
`Implementation prerequisites` field, amended 2026-07-28, reads *"M1a complete …
**M1b is not a prerequisite**"*.

Those disagree. RFC 093 is Accepted, not Implemented: its M1a closed 2026-07-30,
its M1b work is done, but its **closure review is not signed**.

The RFC governs and is the newer text, so the strict reading is not obviously
right — but relaxing a gate on security-critical implementation is not the
architect's call to make quietly, and closing RFC 093 satisfies both readings.
**Recorded, not resolved.** Do not start on the strength of this document alone.

## 2. Prerequisites that *are* satisfied

| Requirement | State |
|---|---|
| RFC 094 under `rfcs/accepted/` with complete approval metadata | **Yes** — accepted 2026-08-27, `3fd9449` |
| M1a complete, gate matrix passing on one clean commit | **Yes** — closed 2026-07-30 |
| Class-A inventory and threat delta independently approved | **Yes** — base inventory 2026-07-17; C17/C18/C23/F01–F06 amendment 2026-07-21 |
| Owner confirms implementation owner | **Yes** — recorded in the RFC |
| No competing change owns the same files | **Yes** — the federation split touches `handlers/federation.rs`; RFC 094 touches the audit/store surface |

## 3. Where to start, and where not to

Start at **Stage 0** (inventory freeze) and **Stage 1** (registry foundation) in
`migration-checklist.md`. Stage 1 is the seam everything else is built on.

**Do not start with the conversion waves.** The RFC is explicit that partial
conversion leaves a residual, and that no unconverted command may be described as
Class-A atomic. Waves come after the foundation exists and its failure tests pass.

**The M2a exit condition added on 2026-08-26 is an exit condition, not an entry
one.** Confining raw database access to `sui-id-store` by the dependency graph is
sequenced last, because two of its three blockers dissolve as conversion proceeds.
Do not begin M2a by trying to seal the crate.

## 4. Two things carried in from review, which are requirements not suggestions

**The negative fixtures are grouped by controlling mechanism, and the grouping is
load-bearing.** `ATTACH`, `DETACH` and transaction control report
`readonly() == true` — measured, twice, on two SQLite builds. A fixture asserting
the runtime check rejects them specifies a test that cannot pass by the route it
names. Read the grouping in RFC 094 before writing any fixture, and if a fixture
passes for a reason the RFC does not name, that is a finding.

**The lane-ownership mechanism needs a differential test, not example cases.**
When you implement it, prove behaviour preservation by running the old and new
algorithms over generated manifests and asserting identical verdicts. The review
that found the two dropped checks used eight hand-built cases and said plainly it
could not know whether those were the only gaps. Do not repeat that method as the
proof.

## 5. Standing expectations

Nothing here changes how you have been working, and all three of the last rounds
have been right, so: measure rather than argue where a question is executable;
say what you did not check rather than implying coverage; refuse a framing that is
wrong and say why; and hash-pin candidates.

**Stop and return to review** — do not work around it — if any stop condition in
the handoff README fires. The one most likely to fire early: a production write
that cannot fit one of the sealed capabilities.

## 6. What to send back first

Before writing implementation code, send a short plan: which Stage 0/1 items you
intend to take, in what order, and anything in the RFC or checklist you found
under-specified while reading. That is cheaper to correct than code.

---

`rfcs/handoffs/094-transactional-audit/m2a-implementation-start.md`
