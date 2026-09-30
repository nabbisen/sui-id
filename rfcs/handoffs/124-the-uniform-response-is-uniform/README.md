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

**Not dispatched.** RFC 124 is Proposed; this stage is specified so that it is
ready, not started. It runs when `@nabbisen` accepts the RFC.

### Confirm first, at the baseline

- The claim itself: `sui-id-core/src/account/forgot_password.rs:32` states the
  request "takes roughly the same time in both branches".
- The known-address path inserts a reset token (`:174`), reads SMTP
  configuration, inserts into the outbox, and writes more audit events; the
  unknown-address path returns after one lookup and one event.
- **The production mailer is the persistent outbox** (`runtime/startup.rs:243`),
  so the send is a local encrypt-and-insert, not inline SMTP. **Confirm this**,
  and confirm whether a deployment can configure an inline SMTP mailer instead —
  if it can, the difference is far larger and this RFC's framing changes.

### The measurement

**Method, stated so it can be re-run rather than believed.** Both branches, same
process, enough samples to say something, **distribution not average** — p10,
median, p90, max, as RFC 123's own measurement reported. Name the environment:
machine, build profile, and whether the timing is taken in-process or through
the real HTTP path. In-process is acceptable and RFC 123's precedent shows why —
a large enough gap survives network jitter without careful statistics — but
**say which you measured**, because the honest conclusion depends on it.

**What the numbers must answer, explicitly:**

1. Is the difference distinguishable at all under this method?
2. If it is, **what would an attacker need** — how many samples per address, over
   what network — to classify "this address exists" with confidence? A difference
   that exists but needs ten thousand samples per guess is a different finding
   from one that needs three.
3. If it is **not** distinguishable, say what method *would* distinguish it, so
   the claim's limits are known rather than assumed absent.

**The artefact is a committed file** under this handoff, with the method, the
environment, the raw numbers and the code that produced them — the shape RFC
123's `d4-measurement.md` established. A measurement nobody can re-run is an
opinion with a number attached.

### Do not

Change behaviour. Tune the code to improve the numbers. Or report a conclusion
the numbers do not carry — **"this claim should simply be deleted" is a
legitimate outcome**, and so is "the asymmetry is real but unexploitable, and
here is why".

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
