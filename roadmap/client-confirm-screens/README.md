# Confirm screens for client disable and secret rotation; stop putting the secret in a URL

**Authorized by.** [`ROADMAP.md`](../../ROADMAP.md) §Non-RFC work packages. Owner
authorization, 2026-09-16. This package implements a contract the shipped
RFCs 030 and 059 already set out: every dangerous operation has a confirm
screen.
**Implementer.** Mid-capability model. **Baseline.** `9f6acdb` or later.

## The defects

Measured by reading at `9f6acdb`.

1. **No confirm screen.** Eight dangerous operations exist, and six have a
   `…-confirm` GET route and a `render_confirm_*` page. Client disable
   (`POST /admin/clients/{id}/disabled`) and client secret rotation
   (`POST /admin/clients/{id}/rotate-secret`) require `_confirmed=1` and a fresh
   step-up, but no confirm screen exists. The form posts directly.
2. **The rotated secret travels in a redirect URL.**
   `clients_rotate_secret_post` (`crates/sui-id/src/http/handlers/admin/clients.rs`)
   redirects to `/admin/clients/{id}/edit?rotated_secret=<secret>`. The secret
   then lands in browser history, in any proxy or server access log that records
   URLs, and in `Referer` if the edit page loads any resource.

## Required

- **Confirm screens.** Add `GET /admin/clients/{id}/disable-confirm` and
  `GET /admin/clients/{id}/rotate-secret-confirm`, built on `ConfirmScreenData`
  like the existing six:
  - identity-of-target line, blast radius, reversibility, optional reason, cancel;
  - the same step-up placement as `clients_delete_confirm_get`;
  - every existing entry point to the two POSTs now goes through them;
  - i18n in en, ja and zh_hans.
- **No secret in a URL.** The POST response renders the new secret directly,
  shown once, with `Cache-Control: no-store` and `Referrer-Policy: no-referrer`.
  Remove the `rotated_secret` query parameter from the edit handler entirely, so
  no URL can display a secret. Apply the same rule to the client-*creation* page
  if it carries its secret in a URL; report either way.
- `docs/src/guides/dangerous-operations.md`: all eight operations have a confirm
  screen. Revert dispatch 15's interim wording.

## Evidence

- Rendered HTML for both confirm screens, in en and ja.
- A test that the rotate-secret POST response contains the secret, has no
  `Location` header, and carries both headers.
- A test that `GET /admin/clients/{id}/edit?rotated_secret=x` does not render
  `x`.
- A route-table test: every `POST` in the dangerous-operation set has a matching
  `…-confirm` GET.
- G12 (UI invariants, text leaks). fmt, both clippy scopes, `cargo test
  --workspace` count before and after, MSRV 1.95.
