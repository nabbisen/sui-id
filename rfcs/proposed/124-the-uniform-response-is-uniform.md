# RFC 124 — The uniform response must be uniform, and must be shown to be

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None. **Stage 1 is a measurement and changes no behaviour.**
**Closure prerequisites.** The account-recovery request does not tell an unauthenticated caller whether an address exists — by response, by status, or by the time it takes — and the request path's work is **identical in both branches by construction**, so that no measurement is needed to defend it and no later change can lose it silently. And **no screen asserts something that may be false**: what the user is told is true for every caller, and a user whose address is not registered is given a route forward rather than left waiting.
**Tracks.** User enumeration. Found by the RFC 119 design review, 2026-09-26; narrowed by the implementation role in the RFC 120 triage.
**Touches.** `crates/sui-id-core/src/account/forgot_password.rs`, `crates/sui-id/src/http/handlers/forgot_password.rs`, `crates/sui-id-i18n/src/locale/{en,ja,zh_hans}.rs`, `crates/sui-id-web/`, their tests, and whatever documentation states the property.
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

## Re-reviewed 2026-10-01, and the RFC was too narrow

`@nabbisen` asked for this RFC to be re-read against his standing philosophy —
*"finally clean, safe and secure, and robust sophisticated design"*, and *"APIs
and UI/UX for users not to be confused or misunderstand"*. Both halves found
something the first draft missed.

**The user-facing half is worse than the timing half, and nobody had raised
it.** The page a user sees after submitting says, in all three locales:

| | Title | Body |
|---|---|---|
| en | **"Email sent"** | "**If an account exists** for the address you provided, we have sent a reset link." |
| ja | **「メールを送信しました」** | 「アカウントが存在する場合、…お送りしています。」 |
| zh | **"邮件已发送"** | "**如果**您提供的地址存在账户，我们已发送重置链接。" |

**The title asserts what the body immediately retracts.** For a caller whose
address is not registered, "Email sent" is simply false — and it is the largest
text on the page, which is what a hurried reader takes away. The body's hedge is
the honest sentence and the title contradicts it.

The consequence is concrete: **a user who mistypes their address is told an email
was sent, waits, checks spam as the page suggests, and is given no reason to
suspect the address was wrong and no route forward.** That is the confusion his
second principle names, produced by a security measure implemented without asking
what it does to the person reading it.

**And the design was measured where it should have been structural.** The first
draft's stage 1 measures the timing difference and *then* decides what to do. A
measurement is true of one machine at one moment; it cannot stop a later change
from reintroducing the asymmetry, and it makes the security property depend on
someone re-measuring. Making the two branches do **the same work** needs no
measurement to defend and cannot regress quietly. The measurement still has a
job — establishing what today's exposure actually is — but it is evidence, not
the design.

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

**D1 — The request path does the same work in both branches, by construction.**
Whether the address exists decides what happens *afterwards*, not what the
request does. The handler performs the lookup and hands off; token creation and
mail belong to the work that follows, not to the response. Then the response
time is the lookup's, which is one query either way, and the property holds
without anyone measuring it again.

The amplification this invites — a caller enqueuing work for addresses that do
not exist — is already bounded by the existing per-IP limit of five requests per
sixty seconds. **Confirm that bound is sufficient rather than assuming it**; if it
is not, the RFC says what bound is.

**D1b — Measure, to establish the exposure, not to decide the design.** Stage 1 produces numbers for both
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

**D6 — No screen asserts what may be false.** The title becomes true for every
caller. It should say what actually happened — *the request was received* — and
leave the conditional to the sentence that already carries it correctly. All
three locales change together; a title that is honest in English and assertive in
Japanese is the same defect in a different language.

**D7 — A user whose address is not registered is given a route forward.** Today
the page offers only "check your spam folder", which is advice for the wrong
problem. It must also say, without revealing which case the reader is in, that if
nothing arrives the address may not be the one registered — and what to do then.
**This costs no secrecy**: it is equally true for both callers, so it
distinguishes nobody, and it is the difference between a user who recovers their
account and one who concludes the system is broken.

**D8 — The wording is reviewed by a native reader before it ships.** Three
locales, and the Japanese and Chinese strings in this project already await a
native read. This RFC does not add more unreviewed strings to that queue; it is
the same queue and the same reviewer, and `@nabbisen` is that reviewer.
