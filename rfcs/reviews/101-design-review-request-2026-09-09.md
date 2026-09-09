# RFC 101 — design review request

**Date:** 2026-09-09
**Requested by:** `@nabbisen` (accountable owner and approver)
**Reviewer:** implementation role
**Subject:** `rfcs/proposed/101-email-lifecycle-verification-and-change.md`
**Baseline:** record the commit you verify against

## 1. Why this needs you

`@nabbisen` has accepted RFC 101. It cannot move to `accepted/` until it has a
design review, and **I wrote it** — role independence (RFC 018) puts it out of my
hands, and G11 will refuse the transition without a tracked review document
regardless.

This is the first RFC in this engagement you have been asked to review as a
design rather than as a conversion. The three rounds where you reviewed my
designs — the lane-ownership mechanism, the addendum constructor, the wave
scopings — each found something real, twice things I had argued for confidently.
Read it that way.

> **Amended 2026-09-10, after this request went out.** The owner settled §6.1 and
> §6.3, and answering them raised a fifth question (§6.5). **Review the current
> text, not the version at `b238d94`** — record the commit you actually verify
> against. §6.1's answer in particular changes what to attack in check B below:
> the gate is now `require_fresh_step_up`, so the question is whether *that* gate
> is sufficient here, not whether some gate should exist.

## 2. What to check, in the order that matters

**A — is the threat model right?** §2 claims three consequences, and the third is
the load-bearing one: that address reuse transfers account recovery to whoever
later controls a stale address, with no compromise of sui-id required. If that
argument is wrong, the RFC's justification is wrong.

**B — is D4 actually achieved?** The claim is that a compromised session is not
sufficient to move the recovery channel, because confirmation happens at the new
address. **Attack it.** Can a session-holding attacker reach the same outcome by
another route — initiating a change to an address they control and simply waiting,
racing a legitimate change, or using the pending state to interfere with recovery?

**C — is D2's "live address stays authoritative" airtight?** The security
consideration says a pending change can never strand a user's recovery. Check
whether that holds through every interleaving: pending change plus concurrent
password reset, two pending changes, a pending change whose token expires.

**D — implementability.** Your usual pass. What is under-specified, what cannot
be built as written, what artifact does it depend on that does not exist. The
token repo is described only as "the `password_reset_tokens` shape" — say if that
is too thin to build from.

**E — the RFC 094 seam.** §D5 says every mutation here is Class-A and lands on the
runner, and that verification and pending-change consumption need inventory rows
of their own. Check that against what the runner actually offers now — in
particular whether the actor shape works, since a verification token is presented
by someone who may or may not have a session, which is the U10 principal question
again.

## 3. What is not yours to settle

The four open questions in §6 are `@nabbisen`'s. Do not answer them — but **do**
say if any of them is not actually open, or if there is a fifth I did not see.
Two of the three "decisions" I put to the owner in Wave B dissolved when checked,
so that failure mode is live.

## 4. Required output

**Path:** `rfcs/reviews/101-design-review-2026-09-<DD>.md`, committed and tracked.
G11 requires the `Independent design review` field's target to resolve and be
tracked; `.git-exclude/` fails.

**Outcome:** Approved / Conditionally Approved / Corrections Required / Design
Revision Required / Requirements Clarification Required.

**State your role and what you checked**, so the field can record it truthfully.

## 5. Standing instruction

Reject the framing of anything above if it is the wrong question. The author of
this RFC is the reviewer of your last several rounds; that is a reason to attack
it harder, not more gently. A design defect found now costs an edit; found after
implementation it costs the implementation.

---

`rfcs/reviews/101-design-review-request-2026-09-09.md`
