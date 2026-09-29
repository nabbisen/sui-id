# RFC 124 — The uniform response must be uniform, and must be shown to be

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None. **Stage 1 is a measurement and changes no behaviour.**
**Closure prerequisites.** The account-recovery request does not tell an unauthenticated caller whether an address exists — by response, by status, or by the time it takes — and that is **demonstrated by a committed measurement and a test that fails when the property is lost**, not asserted by a comment. Where the property cannot be held, the documentation says what it is instead.
**Tracks.** User enumeration. Found by the RFC 119 design review, 2026-09-26; narrowed by the implementation role in the RFC 120 triage.
**Touches.** `crates/sui-id-core/src/account/forgot_password.rs`, `crates/sui-id/src/http/handlers/forgot_password.rs`, their tests, and whatever documentation states the property.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/124-the-uniform-response-is-uniform/README.md`](../handoffs/124-the-uniform-response-is-uniform/README.md)

## Summary

`/forgot-password` answers identically whether or not the address exists — that
is the design, and the response body and status do hold it. The module's own
documentation goes further and says the request "takes roughly the same time in
both branches".

**Nothing has ever measured that**, and the two branches plainly do different
amounts of work: a known address inserts a reset token, reads the SMTP
configuration, inserts into the mail outbox and writes more audit events; an
unknown address returns after one lookup and one event.

## The finding is the unchecked claim, not the milliseconds

The first report of this said the real-account path sends mail inline. It does
not: the production mailer is the persistent outbox, so the send is a local
encrypt-and-insert. That correction matters, because it makes the difference
smaller — and it changes nothing about the actual defect, which is that
**a security property is stated in the tree and no test or measurement stands
behind it.**

This project has met that shape before. RFC 098 exists because documents made
claims the code did not support. A doc comment asserting a timing invariant is
the same thing one layer down, and it is more dangerous there, because a reader
of the code takes it for a property the code has.

## Decision

**D1 — Measure before changing anything.** Stage 1 produces numbers for both
branches, by a stated method, in a stated environment, committed as a file that
someone else can re-run. No behaviour changes in stage 1. **A measurement that
is not written down did not happen.**

**D2 — The measurement decides the work.** If the difference is not
distinguishable under the stated method, the claim is narrowed to what was shown
and the RFC closes there. If it is, stage 2 closes it — by moving the unequal
work off the request path, or by equalising it — and the decision names which
and why.

**D3 — Whatever is true is written where a reader will meet it.** The doc
comment either states a property a test defends, or states the narrower thing
that is true. A comment that promises more than the code delivers is removed,
not softened.

**D4 — A test defends whatever is claimed.** If the claim survives, a test fails
when it stops holding. A timing test is noisy, so the RFC requires the test to
state its own tolerance and its own flakiness budget rather than pretend
otherwise — a test that fails at random will be deleted by someone, and then the
property is unguarded again.

**D5 — The equal-response property that already holds is not disturbed.** Body,
status and the presence of an audit event are uniform today. Nothing in this RFC
makes them less so, and stage 2's tests assert that too.
