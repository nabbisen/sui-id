# RFC 133 — handoff

**RFC.** [`../../done/133-an-approval-names-what-it-settles.md`](../../done/133-an-approval-names-what-it-settles.md)
**Status.** **Implemented — closed to `done/` 2026-10-03**, approved by `@nabbisen` ("Approved."), on the closure review below plus the implementation role's independent re-derivation of its measurements. Previously: **Accepted** 2026-10-02 by `@nabbisen` ("Accepted."), with the
security review already in hand.
**Security review.** [`security-review-2026-10-02.md`](./security-review-2026-10-02.md)
— the architect's own, written before acceptance, returning two required changes
**against the implementation** and one recommendation.

## Dispatched 2026-10-02

Small, and the two required changes are the part that is easy to get wrong.

### D1 — the template

`rfcs/README.md`'s template gains, against `Approved by.`: where a design review
returned changes, the approval **names the decision it settles** — by number, or
by recording that the owner ruled.

**No gate. Deliberately.** Do not add one, and do not "improve" this by adding
one. D1a sets out why: a gate is satisfied by citing *a* decision, and RFC 112
shows a well-intentioned author citing the wrong ones — a gate would convert a
visible gap into a green tick.

### D2 — condition 9 learns that a review is of its own RFC

`scripts/check-rfc-integrity.py` condition 9: at least one citation in
`Independent design review` resolves under this RFC's own `handoffs/<N>-`
directory, unless the RFC carries an allowlist entry in
`contracts/rfc-policy.toml`.

Expected result on the tree today: **exactly three allowlist entries** —
095→094, 096→094, 103→102 — and everything else passes, RFC 126 included.

**An extension to condition 9, not a new gate and not a new lane.** The Gate
Matrix stays at twenty-one.

## The two required changes from the security review

These bind you, not the RFC text. Both close a gap between what the RFC says and
what an implementer would otherwise be free to do.

**1. State the rule's scope explicitly.** The architect's simulation enumerated
citations matching `handoffs/<NNN>-` and **silently skipped any RFC whose
citations matched none** — which the rule as written would fail. Exactly one such
RFC exists (`archive/018`, field present, no link) and it is outside scope. **So
"exactly three" is correct by luck of the corpus, not because the simulation
covered the space.** Scope the check to the same set condition 9 already governs
— Accepted/Done, `Security review: Required` — in the code, not by inheritance
from whatever a script happened to enumerate.

**If your own run produces a number other than three, say so and stop.** That is
a finding, not a fixture to adjust.

**2. An allowlist entry is a reviewable act.** The gate cannot distinguish a
genuine shared review from an entry added to turn a red gate green, and the
required reason can be written for a bad entry as easily as a good one. So:

- every entry carries a reason, and **a blank reason fails** (the rule
  `contracts/contract-paths.toml` already applies);
- adding an entry is **declared in the package that adds it**, naming the shared
  document — never landed quietly beside the change that needed it.

The three entries this dispatch adds are declared by the RFC itself, so they need
no separate declaration; the rule binds the next one.

## The recommendation, if you want it

`CONTRIBUTING.md`'s RFC-lifecycle paragraph (new from RFC 132 D4) gains a clause
pointing at the template for what an approval must contain — one sentence,
closing the gap between the document that sends a contributor into the RFC
process and the document that defines it. Optional; say if you skip it.

## Docstring requirement

Condition 9's docstring must state that **filing location is a proxy for
subject**: the gate verifies where a review is filed, not that it is about this
RFC. A wrongly-filed review passes. This is not a caveat to bury — it is the
difference between a check honest about its reach and one believed to prove more.

## Protocol

As before: hand over a working tree, do not commit, do not push; state the parent
commit as the baseline; declare every hunk's hash; run gates through
`scripts/ci-gate.sh <GATE_ID>`; and measure any number handed to you rather than
applying it — including the "exactly three" above, which is the architect's and
has already been found to rest on a narrower simulation than it claimed.
