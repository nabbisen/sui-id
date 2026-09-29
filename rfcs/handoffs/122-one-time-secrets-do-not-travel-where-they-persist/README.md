# A one-time secret does not travel where it persists

**RFC.** [RFC 122](../../proposed/122-one-time-secrets-do-not-travel-where-they-persist.md), **Proposed**.
**Author.** High-capability model, requirements-architect role.
**Baseline.** `4ebf0f7` or later.

## Measured at `e1a251d` — re-measure before changing anything

- `admin/clients.rs:401-431` (`clients_rotate_secret_post`) redirects to
  `/admin/clients/{id}/edit?rotated_secret=…`; the edit page reads it back at
  `:295` and renders it at `:323`.
- `Cache-Control: no-store` is present on error responses (`errors.rs:216`),
  `/reset-password` (`router.rs:99`), the recovery-link page
  (`admin/users.rs:711`), and the token/JWKS/userinfo responses. **Absent** on
  `clients_create` (`admin/clients.rs:131`), the client edit page, TOTP
  enrollment (`me_security/mfa.rs:99`), and the recovery-code pages (`:147`,
  `:219`).

State the full list you find; the numbers above are a starting point, not the
answer.

## What to build

RFC 122 D1–D4. Two design questions are yours to answer with reasoning:

1. **How a rotated secret survives one navigation without a URL.** Candidates:
   render it directly from the POST response instead of redirecting; a
   short-lived server-side value keyed to the session; or accept that it does
   not survive and the operator copies it from the POST response. **The third is
   the simplest and may be the right answer** — say what it costs the operator
   before dismissing it. Whatever you choose, D2 means the page must not trust a
   value handed to it.
2. **Where D3's headers are applied** so they cannot be forgotten. A response
   builder, a typed "carries a secret" wrapper, a router layer on named routes —
   name the one you chose and why it cannot be bypassed by the next surface.

## Evidence

- A test per surface asserting no secret appears in any URL the flow produces
  (including the `Location` header) and that the response carries the headers.
- A test that the edit page does not render a secret supplied to it by the
  caller.
- Mutations: reintroduce the query parameter; drop the headers from one surface.
- fmt, both clippy scopes, the test count before and after, MSRV, every doc gate.

## What to return

The usual package, with your two design answers and the full enumeration of
one-time-secret surfaces you found.
