# RFC 128 — G11's conditions derive from RFC 000, not from a retired RFC

**Status.** Accepted
**Accepted on.** 2026-10-01
**Approved by.** `@nabbisen`, 2026-10-01: "RFC 128 is accepted." 
**Security review.** Required
**Independent design review.** [Security review 2026-10-01](../handoffs/128-g11-derives-from-rfc-000/security-review-2026-10-01.md) — **by the architect, who authored this RFC, and therefore not independent**; carried by `@nabbisen` under `ROADMAP.md` R1's residual. **This RFC was refused `accepted/` by the very condition it removes**, and satisfying that condition was the only way to land the fix for it. The field's name overstates the document; the document says so in its first lines.
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** Every G11 condition traces to a live document: RFC 000, or the RFC that legitimately owns it. No condition enforces a formulation whose only source is an archived RFC. Where RFC 000 leaves something undefined, the gate does not supply a definition inherited from elsewhere, and says so.
**Tracks.** Documentation authority. Raised by `@nabbisen`, 2026-10-01, on finding that RFC 124 was blocked by a rule he had not made.
**Touches.** `scripts/check-rfc-integrity.py`, `contracts/rfc-policy.toml`, `rfcs/done/093-build-toolchain-release-gates.md` (a lifecycle amendment), and the RFC headers the change affects.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/128-g11-derives-from-rfc-000/README.md`](../handoffs/128-g11-derives-from-rfc-000/README.md)

## Summary

G11 condition 9 refuses an Accepted RFC whose security review is Required and
which carries no `Independent design review` field with a durable reference.
**RFC 000 requires the substance.** It does not supply the formulation the gate
enforces — that comes from [RFC 018](../archive/018-rfc-lifecycle-policy.md),
which is **archived**, retired on the ground that "RFC 000 is the lifecycle
policy", and which describes itself in its own opening lines as *"344 lines of
independent drift, a project copy edited out of step"*.

## What is RFC 000's, and what is not

Measured, not recalled:

| Concept | RFC 000 | RFC 018 (archived) |
|---|---|---|
| independent design review | yes (2 mentions) | yes (6) |
| closure evidence, closure reviewed on | yes | yes |
| implementation owner | yes | yes |
| **`N/A` is prohibited** | **no** | yes |
| **what "independent" means** | **never defined** | *"role independence: the reviewer did not author…"* |

**The substance is RFC 000's and is not in question.** Two things are not:

1. The prohibition on `N/A`.
2. **The definition of independence.** RFC 000 requires "a named independent
   design reviewer" and never says what independence is. RFC 018 does — and its
   definition is what disqualifies an author from reviewing their own RFC.

## Why this is worth an RFC rather than a one-line fix

`@nabbisen` established the provenance on 2026-10-01: **RFC 093 and RFC 018 were
written in the same month, July 2026, by the same architect** — one whose work
this programme has since had reason to distrust, and whose duplicate lifecycle
policy was retired for drifting from RFC 000.

G11 is RFC 093's. So the gate that enforces lifecycle policy was written by the
author of the drifted copy, at the same time as the drifted copy. **Condition 9
is unlikely to be the only inheritance, and a spot fix would leave the rest
unexamined.** That is the reason for D2.

## The concrete harm, already observed

RFC 124 is a security-sensitive RFC. `@nabbisen` assigned its security review to
the architect who wrote it. That review was done and returned a finding that
changed the design — the response is uniform but the work is a six-rung ladder,
and three rungs disclose more than existence.

**The gate cannot record that.** It demands a field named for independence the
review does not have, so the honest entry has to say, inside a field called
`Independent design review`, that the review was not independent. A record whose
field name contradicts its content is not a record.

## The audit criterion, given by `@nabbisen` on 2026-10-01

He stated it sharper than the first draft had: the doubtful statements are those
that **presuppose the existence of a skilled architect external to this team**.
*"No such a person out of our team."* That is what "too ideal" means, and it is a
testable question to ask of each statement rather than a judgement about tone.

**And `ROADMAP.md` §S1 already records the fact that makes it testable:**
`codex-project-architect`, `codex-developer` and
`codex-independent-architecture-security-reviewer` are **agent identities from
one vendor**, with `@nabbisen` as sole human owner. So RFC 093's own
"independent design review" — the review this RFC's condition 9 demands of
everyone else — was by a same-vendor agent, not an external architect. The
roadmap says so in its own words: *"a change authored, implemented and approved
by one party has had no review at all."*

**The mechanism for this case already exists and predates this RFC.** R1's
residual column reads: *"Design judgments no role but the author can assess,
**carried explicitly by the owner**"*, and already lists two — RFC 094's
`ReadConn` sufficiency and RFC 096's B1/B2. **That is the honest home for a
review only the author can do**, and it needs no new rule: the judgement is
recorded as the owner's to carry, not as a completed independent review.

## Decision

**D1 — Condition 9 is re-derived from RFC 000 alone.** Record who reviewed and a
durable repository-relative reference. Nothing inherited from RFC 018: no `N/A`
prohibition, and **no definition of independence**.

**D2 — Every other G11 condition is traced.** Each to RFC 000, or to the RFC
that legitimately owns it, or removed. The provenance above makes this an audit,
not a courtesy. Conditions already known to be sound are still cited, because
"we checked and it was fine" is the output.

**D3 — Where RFC 000 leaves something undefined, the gate leaves it undefined.**
Independence is the case in hand. A gate that supplies a missing definition is
legislating, which is what [RFC 110](../accepted/110-rfc-header-governance-guard.md)
forbids a header from doing — the same fault one layer down. **Whether an author
may review their own RFC is `@nabbisen`'s to decide, and this RFC does not
decide it.**

**D4 — The field records what is true.** A review by the author is recordable as
a review by the author. The twenty RFCs carrying the field today then say what
those reviews actually were, rather than all claiming a property only some of
them have.

**D4b — A review only the author can do is carried by the owner under R1, not
dressed as independent.** The RFC records who reviewed and their relation to the
author; the judgement itself joins R1's residual list, where two already sit.
**RFC 124 is the first entry added under this decision.**

**D5 — Amending G11 amends RFC 093, which is Done.** A lifecycle act, recorded
in RFC 093 as dated and attributed to `@nabbisen`'s approval of 2026-10-01 — not
to the architect, and not to this RFC's author.

## What this RFC does not do

It does not weaken the requirement for review. RFC 000's rule stands: a
security-sensitive RFC needs a named reviewer and durable evidence, and the
implementer cannot be the sole approver. What changes is that the gate stops
enforcing a retired document's additions to that rule.
