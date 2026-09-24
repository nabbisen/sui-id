# G11: an RFC header may not legislate

**Authorized by.** `@nabbisen`, 2026-09-22: *"Make the clear and solid guard
against reproduction of the similar issues. No similar troubles any more."*
**Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Source of the rule being guarded.** [RFC 000](../../done/000-rfc-lifecycle-policy.md)
and [`ROADMAP.md`](../../../ROADMAP.md) §S1. **RFC 000 is not amended by this
package and must not be.**

## What went wrong, twice

The same mechanic produced both incidents: a governance sentence is written into
an RFC header, attributed upward to an authority that does not contain it, and
nothing checks the attribution.

- **2026-07-30.** A vendor-independence rule was written into four RFC headers
  and attributed to an "Owner ruling, 2026-07-28" that cannot be evidenced. It
  blocked seven RFCs for four weeks. Withdrawn 2026-08-26 (§S1).
- **2026-09-22.** Eleven RFC headers (093–103) carried an
  `**Independent security and closure reviewer.**` field asserting "the reviewer
  must not have authored, implemented, or previously approved this RFC" — a bar
  RFC 000 does not contain — attributed to "per RFC 000", and in two cases to
  the disposed RFC 018. Three of the eleven routed review differently from each
  other. All eleven were removed the same day.

Deleting the clauses fixes the instance. This package fixes the mechanic, so
that vigilance is not what stands between the project and a third occurrence.

## Superseded — read the RFC, not this section

**Corrected 2026-09-24.** Everything between here and *Out of scope* described
the design as first proposed. Its independent design review overturned the
central mechanism, [RFC 110](../../accepted/110-rfc-header-governance-guard.md)
was amended to match, and **this page was not** — so it went on specifying five
phrases including RFC 000's own sentence, an exact-regex label rule, a
`Reviewer-rule exemption` field, nine tests, and no archive scope, all of which
the accepted RFC rejects.

The implementer found the contradiction, built the RFC because the RFC is the
authority, and said so in the review package rather than picking one quietly.
That was the right call, and the stale page was the architect's error.

**What was built, and what governs:** RFC 110's decisions D1–D9. In outline —
the **label allowlist leads** (six recorded labels; anything else matching
review, approval, independence or authority fails), **four** phrases rather than
five, matched normalised across the whole header; **no exemption field**;
conditions scoped to `proposed/`, `accepted/` and `done/`; a closed
`[archive_citations]` allowlist in `ci/rfc-policy.toml` for condition 15's one
legitimate case; and the template and RFC 111's handoff corrected in the same
package. Landed in `738c233` with 59 self-tests.

## Out of scope

- **RFC 000 is not touched.** It is shared across projects and has not produced
  this failure anywhere else. The harm came from this project's own restatements
  of it.
- No change to what any RFC requires, to any closure prerequisite, or to who may
  review anything. This package makes an existing rule unrestatable; it does not
  write a new one.
- The contradiction between the owner decision of 2026-08-26 and the ruling RFC
  098 attributes to 2026-09-09 is recorded in §S1 and is `@nabbisen`'s to
  settle. Do not resolve it in code, and do not let this gate depend on which
  way it goes.
