# RFC 122 — A one-time secret does not travel where it persists

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** No secret sui-id shows once is carried in a URL, and no response that carries one is storable: every surface that displays a client secret, a rotated secret, a TOTP secret or QR, or recovery codes is reached by a method that does not put it in a URL, and carries the headers that keep it out of a shared or back-forward cache. Each has a test.
**Tracks.** Secret handling. Found by the RFC 119 design review, 2026-09-26; confirmed by the implementation role in the RFC 120 triage.
**Touches.** `crates/sui-id/src/http/handlers/admin/clients.rs`, `crates/sui-id/src/http/handlers/me_security/mfa.rs`, `crates/sui-id/src/http/router.rs`, `crates/sui-id-web/`, and the tests.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/122-one-time-secrets-do-not-travel-where-they-persist/README.md`](../handoffs/122-one-time-secrets-do-not-travel-where-they-persist/README.md)

## Summary

sui-id shows some secrets exactly once: a client secret at creation, a rotated
client secret, a TOTP secret and its QR code, and recovery codes. Two transports
are wrong for that.

**A rotated client secret is carried in a redirect query string** — so it
reaches the `Location` header, the browser's history, the referrer of anything
the page loads, and any log or proxy that records a URL. The comment above it
says the history entry "is replaced by the subsequent navigation", which is not
what a redirect does. It also means the edit page displays whatever that
parameter contains as "the new secret", so the page can be made to show a value
the server never issued.

**Several of these responses carry no `Cache-Control: no-store`**, so a
back-forward cache keeps them.

## Decision

**D1 — A secret is never a URL component.** Not in a path, not in a query
string, not in a fragment. Where a value must survive one navigation, it travels
by a method that does not put it in a URL, or it does not survive the
navigation.

**D2 — A page renders a secret it obtained, never one it was handed.** The edit
page currently trusts a query parameter. After this RFC, a surface that shows a
secret got it from the operation that produced it.

**D3 — Every response that carries one of these secrets is unstorable.**
`Cache-Control: no-store` and a referrer policy that does not leak the URL. This
is a property of the response, so it is applied where responses are built, not
remembered at each call site — the present state, where `no-store` is on four
surfaces and absent from four others, is what remembering produces.

**D4 — The list is closed and checked.** The secrets sui-id shows once are
enumerated, and a test asserts that each of their surfaces satisfies D1 and D3.
A new one-time secret then either joins the list or fails the test.

## What this RFC does not claim

The exposure is modest and this RFC does not pretend otherwise: these are POST
responses or short-lived query-carrying GETs, on an administrative interface, on
a service with no known deployment. It is worth fixing because the cost is small
and because a secret in a URL is the kind of thing that is cheap now and
expensive after a log aggregator is introduced.
