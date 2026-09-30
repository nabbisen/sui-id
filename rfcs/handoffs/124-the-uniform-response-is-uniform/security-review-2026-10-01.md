# RFC 124 — security review

**Reviewer.** High-capability model, security-reviewer role — **and the author of
this RFC.** `@nabbisen` assigned this review on 2026-10-01.
**This is not an independent review, and is not recorded as one.** RFC 000
requires a named independent design reviewer for a security-sensitive RFC; the
honest state is that no role but the author has reviewed this design, and that
gap is stated here rather than dressed as a completed independent review. The
approver is `@nabbisen`, who is not the author, so RFC 000's actual constraint —
that the implementer cannot be the sole approver — holds.
**Method.** Read every branch of `sui-id-core/src/account/forgot_password.rs`'s
`request_reset` and the handler above it. No execution; no measurement (that is
D1b's, and it is not this review's to pre-empt).

## The finding: the property is not binary, and the RFC's premise understates it

RFC 124 is written about **two** branches — the address exists, or it does not.
**There are six**, and the work on the request path increases monotonically
across them:

| # | Branch | Work before responding |
|---|---|---|
| 1 | Address unknown | 1 lookup, 1 event |
| 2 | Found, **non-Local source** (directory or upstream) | 1 lookup, in-memory filter, 1 event |
| 3 | Found, Local, **no credentials row** — never activated | 1 lookup **+ a second query** (`credentials::get`), 1 event |
| 4 | Found, Local, credentialed, **disabled or deleted** | lookup + `credentials::get`, 1 event **with an actor** |
| 5 | Found, Local, credentialed, active, **at the outstanding-token cap** | + `count_active_for_user`, 1 throttle event |
| 6 | Found, Local, credentialed, active, under the cap | + token mint, token insert, SMTP config read, outbox insert, further events |

All six return the identical response. **The response is uniform; the work is a
ladder.** Three of the rungs disclose something sharper than existence:

- **3 against 1** distinguishes an address belonging to a **provisioned account
  that has never been activated** — exactly the accounts RFC 115 exists to
  protect. An administrator typed that address and nothing has verified it,
  which makes such accounts the most attractive targets on the system, and this
  rung names them.
- **4 against 6** distinguishes **disabled or deleted** accounts.
- **5 against 6** distinguishes **an account that already has reset tokens
  outstanding** — that is, an account already being recovered, or already under
  attack. An attacker learns that someone else is mid-flow.

**2 against 1 is the one good case:** a directory-sourced address is
indistinguishable from an unknown one by work as well as by response, which is
RFC 103 D13 holding exactly as designed.

## What this does to the design

**D1 as written is not achievable, because "both branches" do not exist.**
Equalising two paths is a small job; equalising six, and keeping them equal
through every future change to any of them, is not — and every one of the six is
a place a later edit can add a query.

**The structural answer is stronger than equalisation: the request path must not
classify at all.** Accept the address, hand it off, respond. Every decision —
source, credentials, disabled, throttle, token, mail — happens after the response
has been written. Then there is exactly **one** code path before the response,
the ladder is unobservable because it does not run in the request, and no future
edit to any branch can reintroduce a timing channel, because no branch is inside
the request any more.

That is simpler than what the RFC asked for, and it is the difference between a
property that is maintained and one that is structural.

## Residual risks of the structural design, named

1. **Amplification.** Handing off unconditionally means an unknown address costs
   a queued item. Bounded today by the per-IP limit of five requests per sixty
   seconds; **that bound must be confirmed, not assumed**, and it is the only
   thing standing between this design and a cheap way to fill a queue.
2. **The hand-off must not be conditional on anything address-derived**, or the
   ladder returns in a new place. This is the thing to check in review of the
   implementation, not at design time.
3. **Audit ordering.** Events currently carry an actor for branches 4–6 and none
   for 1–3. Moving classification after the response must not change which
   events exist or their content — only when they are written. RFC 102's rule
   that a sign-in which cannot be audited does not succeed is not in play here
   (this is not a sign-in), but the audit trail must not become weaker.
4. **What the caller is told does not change**, and D6/D7's honesty fix is
   independent of this: a page that asserts "Email sent" is wrong whether the
   work is symmetric or not.

## Verdict

**The RFC needs amending before it is accepted**, on one point: D1's "the same
work in both branches" becomes "the request path performs no classification".
The rest of the RFC — D1b's measurement as evidence, D2–D5, and D6–D8's honesty
and route-forward decisions — stands.

**And the enumeration above belongs in `docs/`**, not only here: it is precisely
the kind of fact RFC 127 says must not live only in code or a review.
