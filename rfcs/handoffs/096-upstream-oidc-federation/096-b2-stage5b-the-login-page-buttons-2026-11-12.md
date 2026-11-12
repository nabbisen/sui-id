# RFC 096-B2 stage 5b — "Sign in with …" on the login page

> ## Landed 2026-11-12, with reviewer fold-ins. Next:
> [`096-b2-stage6-c18-delete-and-c20-unlink-2026-11-12.md`](096-b2-stage6-c18-delete-and-c20-unlink-2026-11-12.md).
>
> Cycle L, 096-B2 S5b —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §3.
> **This document is the dispatch.**

**Dispatched.** 2026-11-12, by the architect.
**Decided.** `@nabbisen`, 2026-11-12: *"Both your recommendations are
accepted."*, on
[`096-b2-login-screen-buttons-decision-2026-11-12.md`](096-b2-login-screen-buttons-decision-2026-11-12.md).
§3 there recommends building the buttons in cycle L.
**Governing design.** [RFC 004](../../done/004-federation.md) `:72` and
`:155` (*"The login screen lists enabled providers as buttons"*), which was
designed and never built. Read the decision's §1 for what exists today.
**Baseline.** The tip of `main` when you start (`git log -1`; record it). S5
must be in it.

## 0 — what this stage is

**After this stage, a user can start federated sign-in from the login page.**
Until now it could be started only by typing `/auth/federated/{slug}/start`.
It also makes four documents true, and makes cycle L's second protection
true for a real reason instead of by accident: *"the login screen does not
offer a disabled provider at all"*.

**Nothing new runs on the server.** The button is a plain link to the
existing `GET /auth/federated/{slug}/start`, which already validates its
`next` (`handlers/federation.rs:472`, `same_site_return_path`), the same
check the password form's POST applies (`handlers/admin/auth.rs:103`).

## A — the page

- **`render_login`** (`crates/sui-id-web/src/pages/auth.rs:26`) takes the
  list of providers to offer, as their slug and display name. Its **four
  callers** (`handlers/admin/auth.rs:123, :457, :537, :1028`) all pass it,
  so a failed password attempt or a `fed_error` return still shows the
  buttons. **Count the callers yourself before relying on four.**
- **Which providers:** `federation_provider::list_enabled`, the function
  RFC 004 meant for this, and still uncalled. Order them by `display_name`.
  Migration 0051's CHECK guarantees an enabled row is `validated` with
  current evidence, so nothing more needs filtering. Say so in a comment.
- **The label** is a translated template, `"Sign in with {name}"` in English,
  and in Japanese the name first, as in 「{name} でサインイン」. Use the
  `{placeholder}` and `.replace` precedent (`pages/me_security/overview.rs:75`).
  Leptos escapes the result. Fill `zh_hans` too, since the completeness test
  requires it; Chinese is still not offered (`Locale::ALL`).
- **`next`.** Each link is `/auth/federated/{slug}/start?next=…`, with the
  page's own `next` **percent-encoded** as a query value. The start handler
  validates it, so this page passes it on exactly as received. When `next`
  is empty, the link has no `next`.
- **Placement.** Put the buttons in their own group, apart from the password
  form, with the divider the passkey option already uses
  (`divider-with-label`). They appear in every `LoginContext`. When no
  provider is enabled, nothing is shown: no group and no divider.
- `ui-invariants.toml`'s blocking checks stay green (no hard-coded text, no
  new inline styles beyond the bound).

## B — tests

- **An enabled provider appears, and a disabled one does not.** Seed one of
  each, with `create_enabled_for_test` and a disabled row.
- **Ordering** by `display_name`.
- **`next` survives**: render with a `next` containing `?`, `&` and `=`, and
  assert the link's query value decodes back to it exactly.
- **Escaping**: a `display_name` with `<`, `"` and `&` renders as text, not as
  markup.
- **No providers, no group**: the divider is absent.
- **Every caller**: a password failure's re-render still lists the provider.
- **End to end, the reason this stage exists.** Load the login page, take
  the button's `href` **from the rendered HTML**, follow it, and complete a
  federated sign-in against the mock upstream (`federation_fail_closed`'s
  harness). Then disable the provider through S5's page, reload the login
  page, and the button is gone.
- **Through an OIDC authorization:** start at `/authorize` for a client,
  land on the login page, follow the provider button, sign in, and land back
  on the authorization flow, as the password path does. If the existing flow
  makes this impossible to drive, stop and report.

## C — the documents

The four places the decision's §1 lists are now true. **Re-read each one**
and correct what is still not true:
- `docs/src/reference/oidc-api.md:195`;
- `docs/src/guides/operators.md:1686`;
- `docs/src/reference/configuration.md:246, :251`;
- `docs/src/getting-started/overview.md:79`.

In particular, *"sui-id adds 'Sign in with X'"* should now say **which**
providers appear: enabled ones only.

`CHANGELOG.md` `[Unreleased]`, under **`### Added`**: the login page offers
each enabled federation provider as a "Sign in with …" button, which it never
did before, though RFC 004 described it.

## D — constraints and what to return

- **No tag, no dispatch.**
- Run `bash scripts/local-matrix.sh --changed`, the complete matrix, and
  quote the scope line and any failing check names.
- **Don't edit files while the matrix reads them.**
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage5b-the-login-page-buttons-2026-11-12.md`,
with:
- the baseline;
- the `render_login` callers you found, and what each now passes;
- the label in both offered locales;
- the `next` encoding test, with its input and decoded output;
- the end-to-end test, including the `href` it followed;
- the OIDC-authorization test, or the report if it could not be driven;
- each document's changed sentence.
