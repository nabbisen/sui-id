# RFC 123 — An endpoint that authenticates a client costs the caller something

**Status.** Proposed
**Security review.** Required
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

## Decision

**D1 — Every endpoint that authenticates a client takes a limit.** The two that
do not, do. The limit's identity, window and bucket are stated rather than
copied, because introspection by a legitimate resource server is a different
traffic shape from a token request by a user's browser, and a limit that breaks
it is worse than none.

**D2 — The expensive step comes after the cheap ones.** Password hashing is
work the caller asked for; it runs once the request has passed everything that
can reject it more cheaply. This is ordering, not a new mechanism.

**D3 — A per-client failure counter, or a stated reason there is none.** A
per-IP limit is defeated by many addresses. The RFC does not assume a counter is
right — it is state, and state on an unauthenticated path is its own hazard —
but it does not leave the question unasked.

**D4 — The client-id timing difference is measured before it is judged.**
`authenticate_client` returns before any hashing for an unknown, public or
disabled client, and after hashing for a known confidential one. Whether that
is distinguishable over a network is an empirical question. **Measure it, then
decide**: close it with a dummy verification, or record it as accepted with the
measurement that justifies the decision. Do not close it by reflex, and do not
dismiss it by reflex.
