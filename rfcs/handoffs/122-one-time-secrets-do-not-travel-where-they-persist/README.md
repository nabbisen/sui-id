# A one-time secret does not travel where it persists

**RFC.** [RFC 122](../../done/122-one-time-secrets-do-not-travel-where-they-persist.md), **Proposed**.
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

## Dispatched for implementation 2026-09-30, on the amended RFC

The [design review](design-review-2026-09-30.md) returned **accept with the
changes named**, and all three corrected what the RFC *claimed* rather than what
it asks to be built. **Build against the amended RFC**; this section's earlier
text is superseded where they differ, and the two design questions it posed are
now answered — by the review, not by you.

**What changed, so you do not re-derive it:**

1. **Six surfaces, not five.** `POST /oauth2/register` returns a `client_secret`
   in its JSON body with no `no-store`, and it is the **only one not behind an
   administrator session** — bearer registration token only, under the token
   routes. Treat it as in scope everywhere the others are.
2. **D1 is not a design problem.** Five of the six already render their secret
   straight from the POST response; only the rotated client secret redirects.
   Making `clients_rotate_secret_post` do what `clients_create` already does — in
   the same file — is the whole transport change. **And it is not a durability
   improvement:** a directly-rendered response is not re-derivable by a reload
   either, so do not describe it as one.
3. **D3's placement is decided: a router layer on the named routes**, the pattern
   `/reset-password` already uses at `router.rs:105-112`. Not a response builder,
   not a typed wrapper — both still need a handler to remember. The layer
   relocates the remembering into one file.
4. **A referrer policy is redundant, not required**, once D1 holds. Do not add
   one to these surfaces; the global default already covers a page whose own URL
   never held a secret.
5. **D4's test asserts both directions**: every route on the list carries the
   layer, and no route off the list carries it. State in the package that the
   list's own completeness is unchecked — that is the known residual and the RFC
   says so.

**Two things to get right that the review flagged and this dispatch will not
restate:** the edit page must not render a secret handed to it by the caller
(D2), and the severity of that is **medium** — a display forgery, not XSS. Do not
write it up as more than it is.

**One unrelated line while you are in the file:** the doc comment above
`render_qr_svg` (`handlers/admin.rs:65-68`) names a call site that does not
exist. Fix it; say you did.

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
