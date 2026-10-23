# RFC 123 D4 on `/oauth2/token` — the two decisions, and the dispatch

> ## Whose turn it is: **the implementation role's.**
> Both decisions below are settled. Neither is reopenable here.

**Written and dispatched.** 2026-10-24 JST, by the architect.
**Governing RFC.** [RFC 123](../../done/123-authenticating-a-client-costs-the-caller.md),
**Implemented (v0.79.0)** — D1, D2, D3 and D4. This completes D4 on the one
endpoint it never reached.
**Origin.** [`stage4-design-…-2026-10-21.md`](../095-dynamic-client-registration/stage4-design-the-authentication-grant-matrix-2026-10-21.md)
§G item 3.
**Baseline.** Read the tip with `git log -1`.

## 0 — who settled this, and two corrections first

**The architect settled both decisions at the owner's request**, in his
words on 2026-10-24: *"review them carefully by yourself with my
fundamental philosophy."* That is `ROADMAP.md` §S1c's exception —
self-review is prohibited by default and permitted only where he asks, with
the record saying he asked. This section is that record.

**Correction 1, and it is embarrassing.** The architect wrote twice that
`/oauth2/token` has no rate limiter. It has one:
`handlers/oidc.rs:371-377`, the handler's first statement, **60 requests
per 60 seconds per IP** (`runtime/ratelimit.rs:134`). **RFC 123 D2 says so
in its own text** — *"it asks only that the new limiter check sit where
`/oauth2/token`'s already does — the handler's first statement"* — and the
architect cited D2 while contradicting it.

**Correction 2.** The load argument built on that error is withdrawn. Sixty
probes a minute from one address is roughly two seconds of blocking-pool
time against ~34 ms hashing. Not a saturation risk.

## 1 — Decision A: equalise **both** channels, not the clock alone

`exchange_code` and `exchange_refresh` are identical here. Each returns
twice before `authenticate_client`:

```
:451 / :625   StoreError::NotFound        → invalid_client      "unknown client"
:458 / :632   is_disabled || is_deleted   → unauthorized_client "client is not allowed"
:919          wrong secret                → invalid_client      "client authentication failed"
```

**Adding a dummy hash to the first two and stopping there would be
theatre.** It closes the slow channel and leaves the fast one wide open:
the caller still reads the answer straight off the response body. A uniform
34 ms that still says *"unknown client"* is worse than useless, because it
looks like the problem was solved.

RFC 123's own compliant function does both — `oauth_token::authenticate_client`
returns `CoreError::Unauthenticated` on **every** branch: bad id, not
found, public, disabled, deleted, no stored hash, wrong secret. One
response, one cost.

**So: one error for every pre-authentication failure, and a dummy
verification on every branch that reaches one.**

- **The code is `invalid_client`** (RFC 6749 §5.2), matching what a wrong
  secret already returns.
- **`unauthorized_client` is dropped here, and it was the wrong code
  anyway.** §5.2 defines it as the client not being authorized to use *this
  grant type* — which is exactly what stage 4b's grant check uses it for at
  `authorize.rs:666`, correctly. Using it for "disabled" conflated an
  account state with a grant restriction.
- **The description must be identical too**, not merely similar. Two
  descriptions that differ by a word are still an oracle.

**The rule this establishes, and state it in the code:** *before client
authentication succeeds, every failure is indistinguishable in both cost
and response; after it succeeds, errors may be specific.* Stage 4b's grant
and scope checks (`:666`, `:685`) run **after** authentication and are
measured to — they may keep `unauthorized_client` and `invalid_scope`, and
should.

## 2 — the cost of Decision A, and how it is paid

A developer with a mistyped `client_id` now gets *"client authentication
failed"* instead of *"unknown client"*. That is a real cost, and this
project does not pretend otherwise: stage 4b's §7 argued the opposite way
for the channel errors, because a registered auth method is not a secret
and a clear message saved an integrator a debugging session.

**Client existence is a secret here; an auth method was not.** That is the
whole difference, and it is why the answer differs.

**But vagueness to everyone is not the answer either** — that is the second
half of this project's standard failing quietly. **The precise reason goes
to the server log**, at `warn`, naming which of the three it was and the
`client_id` claimed. The operator who is entitled to the truth gets it
where it leaks to nobody; the caller gets one answer for everything.

**This improves on RFC 123's precedent rather than copying it.**
`oauth_token::authenticate_client` returns uniformly and logs *nothing* —
measured, there is no `tracing::` call on any of its branches. An operator
debugging a failing integration there has no way to tell an unknown id from
a disabled client. Do not reproduce that. (Bringing the introspection path
up to the same logging is §G item 4's business, not this round's.)

## 3 — Decision B: **no** per-client-id bucket on `/oauth2/token`

This was the open question. The answer is no, and RFC 123 D3 already
contains the reasoning:

> *"A `client_id` is not confidential — it travels in `/oauth2/authorize`
> URLs a browser carries — so this is a targeted denial of a named
> integration, available for free. **That is a sharper problem than the one
> it would be built to solve.**"*

D3 wrote that about a failure *counter*, not a volume bucket, so it is not
a decision on this question — but its test is the right one, and applying
it here gives a harder answer than it gave there.

**The blast radii are not comparable.** D1's `introspect_revoke_client`
bucket is keyed on the same public identifier, and the project accepted it.
Exhausting it denies an RP its *token introspection* — one integration's
validation path, which that RP controls both ends of. Exhausting a bucket
on `/oauth2/token` denies **every end user of that application the ability
to sign in**. Same mechanism, same free attack, and the damage lands on the
people the product exists to serve.

**And after Decision A there is nothing left for the bucket to defend.**
Its motivation was a distributed prober learning which client ids exist.
With one response and one cost for every pre-authentication failure, there
is nothing to learn. What remains is secret guessing, bounded by 60/min/IP
and by Argon2 — which is what D1's closing sentence already says is enough:
*"The volume buckets in D1 already bound failed guesses against one id,
because they bound all calls against it. That is enough."*

**So the bucket would buy nothing and could take an application offline.**
Not built, and the reasoning is recorded here so it is not re-proposed as
an obvious symmetry with introspect.

## 4 — what to build

In `authorize.rs`, for **both** `exchange_code` and `exchange_refresh`:

1. The `NotFound` arm and the `is_disabled || is_deleted` arm each run a
   dummy verification before returning, using the existing
   `dummy_reject`-shaped path rather than a second mechanism.
2. All three pre-authentication outcomes return the **same** code and the
   **same** description.
3. Each logs its real reason at `warn` with the claimed `client_id`.

**One thing to get right that the above does not state.** `dummy_reject`
currently takes the presented secret; on the `NotFound` branch there may be
no client row and possibly no secret. Decide what it hashes so the cost is
the same as a real verification, and say what you chose — an empty string
through `DUMMY_PHC` costs the same Argon2 work regardless of input length,
but check that rather than taking it from me.

## 5 — evidence

**A timing measurement, in release profile.** RFC 123 D4's own table is the
comparison and the format:

| Branch | p10 | median | max |
|---|---|---|---|
| wrong secret (hashes) | 34.01 ms | 34.24 ms | 40.62 ms |
| unknown client id | 81 µs | 99 µs | 165 µs |
| disabled client | 36 µs | 62 µs | 108 µs |

Produce the same three rows for `authorize`'s path, before and after.
**`--release`, and say so.** A debug-profile Argon2 runs around 20× that
and a number near 690 ms means the profile, not the code — this engagement
has already had one such figure reported as fact.

**Response equality, asserted not eyeballed.** One test that drives all
three pre-authentication failures through `/oauth2/token` and asserts the
status, the `error` code and the `error_description` are byte-identical
across them — for **both** grants. An equality across captured responses,
the shape stage 3 used, not three separate assertions that happen to agree.

**Mutation evidence, two.** (a) Give one branch a different description and
show the equality test failing. (b) Remove one dummy and show the timing
separation reappear in your measurement. One each; the second is the only
way to show the timing evidence is not decorative.

## 6 — what to return

The usual package, plus: the before/after timing table with the profile
named; what `dummy_reject` hashes on the `NotFound` branch and why; both
mutations; the full matrix, local; and `hunk-hashes.py` against the tip.

**`CHANGELOG`: `### Security`.** This closes a timing-and-response oracle
on the token endpoint. **Leave severity and the affected-version range as a
marked placeholder** — `.github/SECURITY.md` reserves both to `@nabbisen`,
and the architect will put the wording to him, as at `0.83.0`.

## 7 — what this does not do

Not `oauth_token::authenticate_client`'s missing logging (§G item 4). No
rate-limiter change of any kind — Decision B is *do not build it*, not
*build it later*. No change to stage 4b's post-authentication grant and
scope errors, which are correct as they stand.

Leave the working tree uncommitted. One open dispatch at a time; this is it.
