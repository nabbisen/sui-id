# The uniform response must be uniform, and must be shown to be

**RFC.** [RFC 124](../../proposed/124-the-uniform-response-is-uniform.md), **Proposed**.
**Author.** High-capability model, requirements-architect role.
**Baseline.** `4ebf0f7` or later.
**Two stages. Stage 2 does not start until stage 1's measurement is reviewed.**

## Why this handoff exists at all

`@nabbisen`, 2026-09-29, on the architect's suggestion that the timing be
"folded in as a measurement rather than an RFC": **"No record? No docs? No
handoff? Proceed carefully."** He was right, and this is the correction: the
measurement gets an RFC, a handoff, a committed artefact and a documentation
outcome, like any other work. A measurement done informally leaves the next
person exactly where this one started — with a claim and nothing behind it.

## Stage 1 — measure, change nothing

Measured at `e1a251d`, to be confirmed: the claim is in
`sui-id-core/src/account/forgot_password.rs:32`. The known-address path inserts
a reset token (`:174`), reads SMTP configuration, inserts into the outbox, and
writes more audit events; the unknown-address path returns after one lookup and
one event. The production mailer is the persistent outbox (`runtime/startup.rs:243`),
so the send is a local insert — **confirm this**, because if a deployment can
configure an inline SMTP mailer instead, the difference is much larger and the
RFC's framing changes.

**Produce:** a committed file under this handoff giving the method, the
environment, the sample count, and the distribution for both branches — not two
averages. Say plainly whether the difference is distinguishable, and what an
attacker would need to exploit it (how many samples, over what network). If the
answer is "not distinguishable under this method", say what method *would*
distinguish it, so the claim's limits are known.

Do not change behaviour in this stage. Do not tune the code to make the numbers
better.

## Stage 2 — dispatched only after stage 1 is reviewed

Its content depends on stage 1 and is not specified here, deliberately. If it
runs, RFC 124 D2–D5 govern it: move the unequal work off the request path or
equalise it, say which and why, write the true statement where a reader meets
it, and defend it with a test that states its own tolerance and flakiness
budget.

## Evidence

Stage 1: the measurement file, and confirmation of the four `file:line` claims
above. Stage 2: the usual package, plus the test and its stated tolerance.

## What to return

Stage 1: a review-request package naming the measurement file, with your reading
of what it shows and a recommendation — **including "this claim should simply be
deleted", if that is what the numbers say.**
