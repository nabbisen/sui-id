# RFC 094 — multi-source lane ownership design — implementability review request

**Date:** 2026-08-27 (Asia/Tokyo)
**Requested by:** `@nabbisen` (accountable owner and approver)
**Reviewer:** implementation role
**Baseline:** re-confirm against `main` and record the commit you verify against
**Subject:** RFC 094 §*Structural coverage gate*, the passage specified in `4ac9a1c`

## 1. Why this round exists, and who wrote what you are reviewing

Your A6 sweep found that `62c2680` committed RFC 094 to delivering multi-source
lane ownership while specifying no data format, discovery method, or conflict
rule. That finding was correct and it was against me. I have now written the
mechanism.

**The design under review is mine, written today.** It has had no reader but its
author. Under the review routing in RFC 018, a design goes to the role that must
build against it — which is you, and you did not write it.

This is a single focused question, not a sweep: **could you build this from what
is written, without making a design decision of your own?** If the answer needs a
"well, presumably…", that is a finding.

## 2. What to read

`rfcs/proposed/094-transactional-audit-registry.md`, §*Structural coverage gate*,
the subsections *Manifest shape*, *What condition 7 becomes*, *Why ownership
conflicts cannot occur*, *Why an owning RFC needs no Gate Matrix table of RFC
093's shape*, and *Migration*.

Against them: `ci/gate-inputs.toml` and `scripts/check-gate-inputs.sh` condition 7
as they stand today.

## 3. Claims worth attacking

**A — the behaviour-preserving migration claim.** I assert that mapping G01–G12
to `"093"` reproduces today's check exactly, so the mechanism can land and be
proven green before RFC 094's own lane exists. **This is testable and I did not
test it.** If you can construct the two tables and show condition 7's four checks
accept exactly what it accepts today and reject exactly what it rejects, that is
worth more than any argument in the RFC. If it is not behaviour-preserving, the
migration story collapses and with it the claim that this can land early.

**B — heading parsing generality.** Today condition 7 extracts RFC 093's table
with awk, from `^## Gate Matrix v1` to the next line beginning `#`. My design has
`[gate_lane_sources]` name an arbitrary heading per RFC. Does that extraction
actually generalise? Consider a heading containing regex metacharacters, an RFC
whose lane table is followed by a `###` subsection rather than a `##`, or a
heading that appears twice in one document.

**C — number-to-file resolution.** Resolution matches `NNN-*.md` across
`proposed`, `accepted`, `done`, `archive` and must yield exactly one file. I
verified today that numbers are unique within those folders and that
`rfcs/reviews/` reuses them. What happens during a lifecycle move — is there a
moment, mid-change, where an RFC is in two folders or neither, and does the check
fail loudly or silently mis-resolve?

**D — a soft spot I already know about, named rather than hidden.** My condition 7
spec says every `[gate_owners]` key must be a `[gates]` key *or* a
`[gate_matrix_exceptions]` key. It does **not** say whether an exception is
*required* to have an owner. G12 is the live case. Both readings are defensible
and I did not choose. Tell me which is right and why — that is a genuine
requirements gap in my text, and the kind of thing that becomes a silent
divergence later.

**E — anything I have not thought of.** Two rounds, two correct findings. The
useful question is what a builder hits on day one that an author does not.

## 4. What is not in scope

Do not implement the mechanism. Do not modify `check-gate-inputs.sh`,
`ci/gate-inputs.toml`, or RFC 094. A throwaway harness to test claim A is
expected and welcome — remove it afterward and say so, as you did for the
`readonly()` measurement and the template negative control.

## 5. Required output

**Path:** `rfcs/reviews/094-lane-ownership-design-review-2026-08-<DD>.md`,
committed and tracked.

**Outcome:** Approved / Conditionally Approved / Corrections Required / Design
Revision Required / Requirements Clarification Required.

Record the baseline commit you actually verified against, and state your role and
what you checked.

## 6. Standing instruction

The author of this design is the reviewer of your last two rounds. That is a
reason to attack it harder, not more gently. If the mechanism is wrong, saying so
now costs a document edit; saying so after it becomes how every future RFC
registers a CI lane costs considerably more.

---

`rfcs/reviews/094-lane-ownership-design-review-request-2026-08-27.md`
