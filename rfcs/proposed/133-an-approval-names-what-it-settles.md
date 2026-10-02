# RFC 133 — An approval names what it settles, and a design review is of the RFC that cites it

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None. The design is the measured output of the re-review of the fifteen, 2026-10-02, and both decisions were approved by `@nabbisen` that day before this RFC was written — recorded below rather than claimed as novel.
**Implementation prerequisites.** None.
**Closure prerequisites.** `rfcs/README.md`'s template states what an `Approved by.` field should contain and why. G11 cannot pass an `Independent design review` field whose only citation is a review of a different RFC. Every allowlist entry carries a reason, and a blank reason fails. The gate's docstring states that it checks filing location as a **proxy** for subject, so no reader mistakes it for a stronger guarantee.
**Tracks.** Governance integrity. Both decisions arise from the re-review of the fifteen (RFC 128 stage 0).
**Touches.** `rfcs/README.md` (the template), `scripts/check-rfc-integrity.py` (condition 9), `contracts/rfc-policy.toml` (a third closed allowlist), `scripts/tests/test_rfc_integrity.py`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

## Summary

Two decisions `@nabbisen` approved on 2026-10-02, carried here because both change
a contract and RFC 000 does not let a contract change land on a conversation.

They are one RFC because they are one subject: **the record of a review and the
record of its approval, each saying what it actually is.** The re-review of the
fifteen found no design defect in any package — every judgment was right — and
instead found that in five cases the approval did not name the choice it settled,
and in one the review field cited a review of a different RFC.

## D1 — The template says what an approval should contain

`rfcs/README.md`'s template gains, against `Approved by.`: **where a design review
returned changes, the approval names the decision it settles — by number, or by
recording that the owner ruled.**

**Its home is the template and not RFC 000.** This governs how an approval is
*written*, not who may approve; and RFC 000 is the July document whose authority
this programme has agreed not to lean on (RFC 128; RFC 130's "practice, not
authority" section).

**Measured, not asserted.** Across the twelve RFCs the re-review covered, a rule
*"cite a decision by number, or record that the owner ruled"* separates the record
cleanly:

| Re-review verdict | RFCs | Rule |
|---|---|---|
| He ruled the judgment | 102, 103, 115, 116 | all **pass** |
| Approval named the change | 118 | **passes** |
| Visibility gap | 110, 121, 122, 123 | all **fail**, correctly |
| Accepted before the review | 117, 126 | both **fail**, correctly |
| Partial gap | 112 | **passes, and should not** |

**Six of seven problem cases, five of five clean cases, one blind spot.** RFC 112
cites `D2 and D3` — real decisions, but not the two whose visibility was in
question.

### D1a — and it is not a gate

**Deliberate.** Closing RFC 112's blind spot would require the gate to decide
*which* change was the deciding one. That is the gate supplying a judgment the
document withholds — the fault RFC 110 exists to forbid and RFC 128 found G11
had been described as committing.

A convention that catches six of seven by being read is worth more than a gate
that catches the same six and invites a vacuous citation to satisfy it. **The
template is the enforcement, and a reader is the mechanism.**

## D2 — A design review must be of the RFC that cites it

G11 condition 9 today checks that an `Independent design review` reference is
**durable and resolvable**. It does not check that the review is of *this* RFC.
That is how RFC 126's field could cite the review of **RFC 123** and pass — a
defect RFC 126 discloses in its own header, and which it notes G11 could not see.

**The check.** At least one citation in the field resolves under this RFC's own
`handoffs/<N>-` directory, unless the RFC carries an allowlist entry.

**Simulated across all 26 RFCs carrying the field**, by the architect and
independently by the implementation role, with the same result: **exactly three
allowlist entries** — 095→094, 096→094, 103→102, all genuine shared review
documents — **RFC 126 passes** because it now cites its own review alongside its
credit to RFC 123, and **it would have failed RFC 126 in its original state.**

### D2a — Three conditions, because the check is weaker than it looks

1. **An extension to condition 9, not a new gate or lane.** The Gate Matrix has
   twenty-one lanes and a standing caution against complexity.
2. **Every allowlist entry carries a reason; a blank reason fails** — the rule
   `contracts/contract-paths.toml` already applies. This would be
   `rfc-policy.toml`'s **third** closed allowlist, beside `[archive_citations]`
   and `[historical_rfc_mi]`, and exception lists are where rules quietly stop
   meaning things.
3. **The docstring states that filing location is a proxy for subject.** The gate
   cannot verify a document is *about* this RFC; it verifies where it is filed. A
   wrongly-filed review passes. Saying so is the difference between a check that
   is honest about its reach and one that is believed to prove more.

## What this is not

It is not a response to a design defect: **there was none in any of the fifteen.**
Both decisions address the *record*, which is the only thing the re-review found
wanting.

D1 adds no gate. D2 adds no lane. Together they are one template sentence, one
condition extension and one allowlist of three — which is the whole of what the
re-review's findings support, and no more.

## Risk

**D2's proxy is the risk, and it is stated rather than mitigated.** A review filed
under the right directory is assumed to be about the right RFC. If reviews are
ever legitimately filed elsewhere, the rule breaks and the allowlist will absorb
the breakage until it means nothing. **The condition attached: revisit the rule in
that case rather than allowlist around it.**

**D1's risk is that a convention without a gate decays.** Accepted knowingly: the
alternative is a gate that can be satisfied by citing any decision, which would
convert a real convention into a box-tick — and RFC 112 shows that failure mode is
reachable by accident, not only by bad faith.
