# RFC 094 M2a — implementation start

**Prepared:** 2026-08-27 (Asia/Tokyo)
**Prepared by:** requirements-architect role
**Audience:** implementation role
**Governing RFC:** [RFC 094](../../accepted/094-transactional-audit-registry.md), Accepted 2026-08-27
**Companion:** [`migration-checklist.md`](migration-checklist.md) — the staged task list

> **Entry gate satisfied 2026-08-28.** RFC 093 closed as Implemented on
> 2026-08-27 (`9610abb`), and every other entry-gate item is met — see §2.
> Implementation is authorized.

## 1. The gate item that was unresolved, and how it closed

This document originally withheld authorization over one conflict: the handoff's
entry gate required *"RFC 093 is Implemented"*, while RFC 094's own
`Implementation prerequisites` field, amended 2026-07-28, said *"M1a complete …
**M1b is not a prerequisite**"*.

It was recorded rather than resolved, because relaxing a gate on
security-critical implementation is the owner's call even when the governing RFC
supports it. RFC 093 then closed, which satisfies the strict reading and the
loose one alike. **No judgment about which reading governs was ever needed** —
kept here because the reasoning matters more than the outcome: a handoff may not
impose a stricter gate than its RFC, and if this recurs where closure is not
imminent, the discrepancy goes to `@nabbisen`, not to whoever notices it.

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
