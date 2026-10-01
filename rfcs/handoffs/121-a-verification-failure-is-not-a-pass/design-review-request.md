# RFC 121 — independent design review request

**RFC.** [RFC 121 — A verification failure is not a pass](../../done/121-a-verification-failure-is-not-a-pass.md). **Proposed.**
**Reviewer.** Mid-capability model, implementation role. It authored neither the
RFC nor its handoff — though it confirmed this finding in the RFC 120 triage,
which is disclosed here rather than left for someone to notice.
**Route.** As every design review in this programme: the role that must build
against the design reviews it.
**Baseline.** `4ebf0f7` or later.
**Scope.** Read-only. Report findings.
**Why only this one.** RFCs 122, 123 and 124 are written and wait behind this
review, so the dev team holds one review and no build. They are not hidden from
you: read them if it helps, but do not review them here.

## What to attack

1. **Confirm the defect at the baseline**, with `file:line`: the audit viewer
   substitutes a "nothing wrong" report for an error and computes a green
   banner from it; the settings logs page maps the same error to an error page.
   **Find every caller of `verify_chain_tail`** — the RFC assumes two and one
   startup path, and an assumption is what this review is for.
2. **Is "three outcomes" the right shape?** D1 says *verified intact*,
   *verified and broken at N*, *could not verify*. Is there a fourth this misses
   — a partial verification, a window that did not cover what the operator
   thinks it covered? `verify_chain_tail` verifies a **tail**; state plainly
   what an "intact" answer does and does not cover, because that is a second
   way this screen can mislead and the RFC does not address it.
3. **D3 — should a failure to verify be written to the audit log?** Argue both
   sides. A row saying the audit log could not be read is of doubtful value when
   the cause is the audit store itself.
4. **D4 — startup.** Say what it does today for each of the three outcomes, and
   whether a failure there should be fatal. RFC 112 just made the binary refuse
   a database it does not understand; **is an audit chain that cannot be
   verified the same kind of thing, or a different kind?** This is the question
   the architect is least sure of, and it is the one most likely to be decided
   wrongly by analogy.
5. **D5 — the test.** How is a verification error injected without a test-only
   branch in production code? If it needs one, say so and say what it costs.
6. **Anything else**, including whether this is worth doing at all before the
   chain has an external anchor — `ROADMAP.md` §S2 records that it is
   tamper-evident only within its trust boundary. Make that argument if you
   believe it; "this fixes a display bug on a control that cannot do what its
   users think" would be a finding worth having.

## What to return

The usual package: the confirmation with `file:line`, findings ranked
blocker / high / medium / low, your answers to 2–6 each citing what you read,
and a recommendation — accept as written, accept with the changes you name, or
do not accept.
