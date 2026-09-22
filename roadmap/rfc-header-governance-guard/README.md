# G11: an RFC header may not legislate

**Authorized by.** `@nabbisen`, 2026-09-22: *"Make the clear and solid guard
against reproduction of the similar issues. No similar troubles any more."*
**Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Source of the rule being guarded.** [RFC 000](../../rfcs/done/000-rfc-lifecycle-policy.md)
and [`ROADMAP.md`](../../ROADMAP.md) §S1. **RFC 000 is not amended by this
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

## Required

Two new conditions in `scripts/check-rfc-integrity.py` (G11), which already
parses every RFC's header block — the title line up to but excluding the first
`## ` heading — into bold, period-terminated `**Label.** value` fields. The gate
command in `ci/gate-inputs.toml` does not change.

### Condition 14 — no RFC header states a rule about who may review

Reject any RFC whose **header block** contains a review-rule phrase. The list is
literal, closed, and small, so that a failure is unambiguous and a false
positive is rare:

- `must not have authored`
- `Role independence`
- `independence means` (any case)
- `vendor is not a criterion`
- `cannot be the sole approver`

Also reject any header **field label** matching
`Independent security and <anything> reviewer` — the field that carried all
eleven clauses. It has no legitimate use: `Independent design review` (a record
of a review that happened) and `Accountable owner and approver` are the fields
that carry reviewer facts, and both stay required.

**The distinction the gate is drawing**, and the failure message must say it: a
header may **record** who reviewed something. It may not **rule** on who is
allowed to.

**Exemption, so that a real exception is visible rather than invisible.** A
header may carry `**Reviewer-rule exemption.**` whose value states the necessity
and cites the owner's dated approval. When that field is present, condition 14
does not fire for that RFC. An RFC with the exemption field and no such citation
fails.

### Condition 15 — no RFC header cites an archived RFC as authority

Reject any `RFC NNN` reference in the **header block** where `NNN` resolves to a
file under `rfcs/archive/`. Bodies may discuss an archived RFC historically;
a header is normative metadata and must not rest on a disposed document.

Today this catches nothing — RFC 099's and RFC 100's "per RFC 018" went out with
the clauses — which is the point: it catches the next one.

## Evidence

Negative self-tests in `scripts/tests/test_rfc_integrity.py`, following the
existing fixtures there. One per branch, each asserting the exit status **and**
that the message names the offending file and phrase:

| Test | Expect |
|---|---|
| Header containing `must not have authored` | fail |
| Header containing `Role independence` | fail |
| Header with an `Independent security and closure reviewer` field | fail |
| The same, plus a `Reviewer-rule exemption` field citing a dated owner approval | pass |
| The same, plus an exemption field with no citation | fail |
| Header citing an RFC that lives in `rfcs/archive/` | fail |
| Header citing an RFC that lives in `done/` | pass |
| The same phrase in an RFC **body**, not its header | pass |
| The repository as it stands | pass |

The last row matters: the gate must be green on the tree it lands in, without
editing any RFC to make it so.

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
