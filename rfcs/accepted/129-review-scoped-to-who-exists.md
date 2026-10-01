# RFC 129 — Review is scoped to who exists, and an external reviewer is engaged once, deliberately

**Status.** Accepted
**Accepted on.** 2026-10-01
**Approved by.** `@nabbisen`, 2026-10-01: "RFC 129 was reviewed and accepted." 
**Security review.** Required
**Independent design review.** [Owner review 2026-10-01](../handoffs/129-review-scoped-to-who-exists/owner-review-2026-10-01.md) — by `@nabbisen`, who did **not** author this RFC. **This is the first entry in this line that the word "independent" fits in its ordinary sense**, rather than in the sense a retired RFC supplied or the sense R1's residual covers when nobody is available.
**Design prerequisites.** [RFC 128](../accepted/128-g11-derives-from-rfc-000.md)'s stage 0, complete 2026-10-01. This RFC acts on its Part 1.
**Implementation prerequisites.** None.
**Closure prerequisites.** No milestone exit gate and no RFC closure prerequisite requires a reviewer who does not exist. Every such statement either names a role this project has, or names the one deliberately scheduled external engagement and says why that point and no other.
**Tracks.** Programme viability.
**Touches.** `ROADMAP.md` (seven milestone exit gates), `rfcs/accepted/094-transactional-audit-registry.md`, `095`, `096` (five closure prerequisites).
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/129-review-scoped-to-who-exists/README.md`](../handoffs/129-review-scoped-to-who-exists/README.md)

## Summary

RFC 128's audit found that **every milestone exit gate from M2b onward**, and
**five closure prerequisites across RFCs 094, 095 and 096**, require an
"independent" reviewer. `@nabbisen` has stated there is no such person outside
this team, and that he can engage one temporarily but that it is not ordinary.

**So the programme cannot finish**, and has not been able to since July 2026.

## What the requirement actually buys, measured

| Claimed benefit | Present state |
|---|---|
| A second mind on the design | **Held by `@nabbisen`**, who reviews and approves; the dev team's measurement and enumeration supply the facts such a review needs, and did so this fortnight for an authentication bypass and an audit chain that never verified linkage |
| Non-collusion | **Already secured by RFC 000** — the implementer is not the sole approver, and the approver is the owner. The extra label adds no control |
| External credibility | **Not drawn on.** `README.md`, `docs/src/` and `.github/SECURITY.md` make **no claim** of independent or third-party review. When the claim was made internally it was false: `ROADMAP.md` §S1 records the reviewers as agent identities from one vendor |

**Two of the three are already held, and the third is not being used.** Against
that sits a programme that stops at M2b.

## Decision

**D1 — Every blocking statement is re-scoped to a role this project has.** The
seven exit gates and five closure prerequisites name `@nabbisen`, or the
architect, or the dev team, or they say what evidence must exist and who accepts
it. **None names a role that does not exist.**

**D2 — One external engagement, at one point, stated with its reason.** External
review is worth its cost exactly where a **public claim** is made — the first
release that presents this system as ready for someone else to run. That is
**M6/M7, soak entry and readiness**, and nowhere earlier. One engagement is
"temporary and not ordinary"; seven standing gates were neither.

**D3 — Nothing claims externally what it has not had.** No document asserts
independent or third-party review before D2's engagement happens and produces a
durable reference. The project currently makes no such claim, and this RFC's job
is to keep it that way rather than to create one.

**D4 — Where no role but the author can judge, `@nabbisen` carries it under R1.**
The mechanism exists, predates this RFC and already holds four entries. It is the
standing answer, not an exception.

**D5 — The re-scoping is recorded as a correction, not a relaxation.** Each
amended statement says what it used to require and why that was unsatisfiable.
A reader in a year must not find a weakened gate and wonder whether it was
quietly loosened to make a date.

## What this RFC does not do

It does not reduce the evidence any milestone requires. Rollback evidence,
crash-injection evidence, hostile-provider corpora, live-integration evidence:
all unchanged. **What changes is who accepts them** — a person who exists rather
than one who does not.
