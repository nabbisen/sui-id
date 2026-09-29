# RFC 121 — A verification failure is not a pass

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** No surface reports the audit chain as intact when verification did not complete: a failure to verify is distinguishable from a verification that found nothing wrong, on every surface that shows either, and the two surfaces that show it agree. A failure is recorded where an operator will meet it.
**Tracks.** Audit integrity. Found by the RFC 119 design review, 2026-09-26; confirmed by the implementation role in the RFC 120 triage.
**Touches.** `crates/sui-id/src/http/handlers/admin/audit.rs`, `crates/sui-id/src/http/handlers/settings.rs`, `crates/sui-id/src/runtime/startup.rs`, `crates/sui-id-web/`, and the tests.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/121-a-verification-failure-is-not-a-pass/README.md`](../handoffs/121-a-verification-failure-is-not-a-pass/README.md)

## Summary

The audit viewer calls `verify_chain_tail`, and on an **error** substitutes a
report meaning "nothing wrong", from which it computes a green banner. So the
one screen an operator would look at to ask whether the audit log has been
tampered with answers "no" both when the answer is no and when the question
could not be asked.

This is the defect that hides other defects, which is why it goes first.

## Why it is worse than one bad banner

Three facts compound:

1. **The two viewers disagree.** The settings logs page maps the same error to
   an error page; the audit page maps it to success. An operator who checks the
   other screen gets a different answer to the same question.
2. **Startup does not close the gap.** It warns that verification *could not
   run*; it never reports that verification *failed*.
3. **RFC 094's entire argument** for the hash chain is that tampering is
   evident. Evidence that reports itself intact when it could not be read is
   not evidence, and this project has said so before, about a different subject:
   RFC 102's rule is that a sign-in that cannot be audited does not succeed.

## Decision

**D1 — Three outcomes, never two.** Verification yields *verified intact*,
*verified and broken at N*, or *could not verify, because E*. No surface
collapses the third into either of the first two, and no default value stands
in for a result that was not obtained.

**D2 — The surfaces agree.** Wherever chain state is shown, the same three
outcomes appear, with the same words. The present disagreement between the audit
page and the settings logs page is the evidence that one shared answer is needed
rather than two independent mappings.

**D3 — A failure to verify is recorded, not only displayed.** An operator who
never opens the page still needs it. It is logged at a level an operator sees,
and the RFC says whether it also belongs in the audit log itself — noting that
an audit row about a failure to read the audit log is of limited value if the
cause is the audit store.

**D4 — Startup distinguishes the same three.** "Could not run" and "ran and
found a break" are different sentences today only by accident; they become
different by rule.

**D5 — The test is the mutation.** A test injects a verification error and
asserts that no surface shows an intact chain. The present code passes every
test it has, which is why this defect survived.
