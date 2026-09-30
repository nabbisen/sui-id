# RFC 123 — An endpoint that authenticates a client costs the caller something

**Status.** Accepted
**Accepted on.** 2026-09-30
**Approved by.** `@nabbisen`, 2026-09-30: "Accepted." On the **amended** text.
Its implementation was built while this RFC was still Proposed, because the
architect's own end-of-turn file list named the handoff as a dispatch; that work
was **held unconmitted** until this acceptance rather than landed, and the
implementer's package recorded the Proposed status correctly throughout.
**Security review.** Required
**Independent design review.** [Design review 2026-09-30](../handoffs/123-authenticating-a-client-costs-the-caller/design-review-2026-09-30.md) by the implementation role, which had itself narrowed this finding from four endpoints to two in the RFC 120 triage. Verdict **accept with the changes named**. It **measured** what this RFC asserted, answered D3 rather than leaving it open, and found a larger problem this RFC does not fix. Amended on it the same day.
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** No unauthenticated caller can make sui-id spend password-hashing work without a limit that stops them, and no endpoint that authenticates a client is reachable without one. Whether a client id can be distinguished from a non-existent one by timing is measured and either closed or recorded as accepted with its reasoning.
**Tracks.** Availability. Found by the RFC 119 design review, 2026-09-26; confirmed and narrowed by the implementation role in the RFC 120 triage.
**Touches.** `crates/sui-id/src/http/handlers/oauth_token.rs`, `crates/sui-id-core/src/oidc/oauth_token.rs`, `crates/sui-id/src/http/handlers/oidc.rs`, and the tests.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/123-authenticating-a-client-costs-the-caller/README.md`](../handoffs/123-authenticating-a-client-costs-the-caller/README.md)

## Summary

`/oauth2/token` takes a per-IP rate limit. `/oauth2/introspect` and
`/oauth2/revoke` take none, and both authenticate the client — which for a
confidential client means an Argon2 verification, configured for **64 MiB** of
memory, on every call. A caller who knows one client id can therefore make the
service spend that, repeatedly, from one connection.

The secret is not the target. Secrets are random and guessing them is not the
risk. **The memory and CPU are the target**, and the present design hands them
out on request.

## What the triage corrected, and why it is still worth an RFC

The finding as first written also named `webauthn_auth_start` and dynamic
registration. Neither is a defect: the first needs a pending-sign-in cookie
issued only after a correct password, and the second needs an initial-access
token. **Two endpoints, not four**, and this RFC says so rather than inheriting
the larger claim.

## What this RFC does not close

**A rate limit bounds requests per window; it does not bound requests in
flight.** `verify_password` and `hash_password` run **synchronously on the Tokio
worker thread handling the request** — and the runtime defaults to one worker per
core. A short burst of concurrent authenticating calls, well inside any
per-window budget, can occupy every worker for the ~34 ms each takes, stalling
every other request the service is serving — not only these two endpoints. This
already applies to `/oauth2/token`, which has a limiter.

**That is a separate, larger piece of work and is not folded in here.** It is
[RFC 126](../proposed/126-password-hashing-must-not-block-the-runtime.md). This RFC closes the
volume-over-time half; a reader must not take "RFC 123 landed" to mean the
exposure class is closed.

## Decision

**D1 — Every endpoint that authenticates a client takes a limit, and it is two
buckets, not one.** The two endpoints that take none, do.

- **Per-IP**, a new named bucket, sized generously enough that a resource server
  behind a shared NAT does not trip it under normal load. The exact number is an
  operational decision for `@nabbisen` and the implementer together, not
  something this RFC fixes by guessing.
- **Per-claimed-`client_id`**, sized higher, because one legitimate integration
  bursts far more than one signed-in human.

**Neither requires authenticating first**, which is what made this look like a
conflict with D2. `client_id` arrives unauthenticated from `client_credentials()`
before `authenticate_client` runs — it is a **claim**, exactly as the existing
limiters' IP is a claim, and it is used as a rate-limiting key, not as an
identity assertion. So the tension D2 seemed to create dissolves rather than
being traded away: both buckets are checked immediately after credential
extraction, before the expensive step.

Per-IP alone is sized wrong for a resource server behind one egress address;
per-client alone is defeated by an attacker rotating source addresses against one
fixed id. They defend different shapes of the same attack.

**D2 — The expensive step comes after the cheap ones, and inside
`authenticate_client` it already does.** Client-id parse, one indexed row read
and three field checks all precede the single `verify_password` call. **This RFC
is not asking for a reorder of anything that exists**; it asks only that the new
limiter check sit where `/oauth2/token`'s already does — the handler's first
statement, before credential extraction resolves anything.

**And it introduces no protocol-conformance change**, which was checked rather
than assumed: introspect and revoke already build every error through the same
RFC 6749 §5.2 shape `/oauth2/token` uses, so a limited request returns the 429
that endpoint already returns. Neither RFC 7662 nor RFC 7009 specifies a shape
for "too many requests". No existing error path changes.

**D3 — No per-client failure counter, and the reason is sharper than the hazard
first described.** A counter keyed on failures against an unauthenticated
`client_id` claim can be driven by *failing on purpose*: anyone who knows a real
client id can lock out the legitimate integration that owns it. A `client_id` is
not confidential — it travels in `/oauth2/authorize` URLs a browser carries — so
this is a targeted denial of a named integration, available for free. **That is a
sharper problem than the one it would be built to solve.**

The volume buckets in D1 already bound failed guesses against one id, because
they bound *all* calls against it. That is enough.

**D4 — Measured, and closed.** The measurement is in the design review's
evidence: 300 samples per branch, `authenticate_client` called directly.

| Branch | p10 | median | max |
|---|---|---|---|
| known, confidential, enabled — wrong secret (hashes) | 34.01 ms | 34.24 ms | 40.62 ms |
| unknown client id | 81 µs | 99 µs | 165 µs |
| disabled client | 36 µs | 62 µs | 108 µs |
| public client | 17 µs | 42 µs | 90 µs |

**Not a subtle side channel:** the hashing branch's entire range sits 200–2000×
above every other branch's maximum, so it survives ordinary network jitter
without careful statistics. `Argon2` is configured `m_cost = 64 MiB`,
`t_cost = 2`, `p_cost = 1` (`crates/sui-id-core/src/authn/password.rs:11-13`) —
and `t_cost = 2` is what explains the 34 ms, not the memory alone.

**Closed with a dummy verification on every branch that returns before hashing**,
copying `authn/session.rs`'s existing `DUMMY_PHC` calls (`:154`, `:162`, `:175`)
rather than inventing an idiom this project already has.
