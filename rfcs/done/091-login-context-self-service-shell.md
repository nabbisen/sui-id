# RFC 091 — LoginContext Rendering and SelfServiceShell Navigation

**Status.** Implemented (v0.73.0)
**Tracks.** UI/UX handoff v2.3 §4 (Login), §5 (Navigation, SelfServiceShell)
— unit 5. Category B.
**Touches.** `sui-id-core/src/authorize.rs` or `session.rs` (LoginContext
derivation), `sui-id/src/handlers/admin/auth.rs` or login handler
(context-aware copy), `sui-id-web/src/components/chrome.rs` (SelfServiceShell
layout), `sui-id-web/src/pages/auth/login.rs` (context-aware rendering), i18n.

## Summary

Two UI improvements that complete the v2.3 contract for user-facing identity:

1. **LoginContext rendering.** The login page derives context from the `next`
   parameter in the handler and renders context-appropriate copy:
   `AdminPanel`, `OidcAuthorize { client_name }`, or `SelfService`. The
   `client_name` always comes from the validated client record, never from
   user-supplied query parameters.

2. **SelfServiceShell.** The `/me/security/*` routes render in a dedicated
   shell (`SelfServiceShell`) with its own horizontal navigation (Security,
   MFA, Sessions, Passkeys, Language) and a user-menu that shows "Back to
   admin panel" for admin users. Currently these routes render in the same
   shell as the OIDC auth pages.

## Motivation

### LoginContext

The current login page always renders "Sign in" without any reference to
why the user is logging in. When a user is redirected from an OIDC authorize
request, they receive no visible confirmation that the sign-in is for a
specific application — a requirement for transparency and anti-phishing
posture.

From v2.3 §4:
> "The login surface is one handler, but its copy is driven by a
> `LoginContext`. **[NEW CONTRACT — copy/rendering not yet implemented]**"

Trusted-name invariant: the rendered `client_name` is always loaded from
the **registered client record** after validating `client_id` and
`redirect_uri` through the same path the authorize endpoint uses.

### SelfServiceShell

The `/me/security/*` self-service pages currently use the same shell
structure as the OIDC auth pages. v2.3 §5 (P1-2) introduces a dedicated
shell with contextual navigation. Without a dedicated shell, the
self-service navigation is absent and users cannot tab between Security /
MFA / Sessions / Passkeys / Language without using the browser's back button.

## Target code areas

### LoginContext derivation

In the login GET handler:
1. Read `?next=` parameter.
2. If `next` starts with `/oauth2/authorize`:
   - Extract `client_id` and `redirect_uri` from the pending authorize
     request (same validation path as `validate_client_and_redirect_uri`).
   - On success: `LoginContext::OidcAuthorize { client_name: client.name }`.
   - On failure: `LoginContext::AdminPanel` (neutral fallback — never echo
     untrusted input).
3. If `next` starts with `/me/`:
   - `LoginContext::SelfService`.
4. Otherwise: `LoginContext::AdminPanel`.

```rust
pub enum LoginContext {
    AdminPanel,
    OidcAuthorize { client_name: String },
    SelfService,
}
```

The enum is added to `sui-id-web` (render data) and derived in the handler.

### Login page rendering

The render function receives `LoginContext` and emits:

| Context | Title i18n key | Body copy i18n key |
|---|---|---|
| AdminPanel | `login_title_admin` | `login_body_admin` |
| OidcAuthorize | `login_title_oidc` | `login_body_oidc` (shows `{client_name}`) |
| SelfService | `login_title_self_service` | `login_body_self_service` |

5 new i18n keys across all locales.

### SelfServiceShell

New shell component in `crates/sui-id-web/src/components/chrome.rs` (or
a new file `components/self_service_shell.rs` if the file exceeds 500 ELOC
after the addition).

Layout:
- Horizontal nav: Security · MFA · Sessions · Passkeys · Language.
  Active tab is path-matched (same mechanism as admin settings tabs).
- User menu (top right): username, "Sign out". For `role == Admin`, also
  "Back to admin panel" link.
- No admin navigation labels visible to non-admin users.
- Matches the authenticated shell's `role="navigation"` + skip link pattern.

The `/me/security/*` handlers that currently call `render_*` with
`AuthShell` or the fallback shell switch to `SelfServiceShell`.

## Security properties / invariants

- **P1 (trusted client name).** `LoginContext::OidcAuthorize` is only
  constructed after successful Phase-1 client validation. A user-supplied
  `client_id` in a query parameter that fails validation never reaches the
  rendered page.
- **P2 (no admin nav in SelfService).** The `SelfServiceShell` never
  renders admin navigation links. A non-admin user visiting
  `/me/security/*` cannot discover admin routes by inspecting the page.
- **P3 (role-aware "Back to admin panel").** The "Back to admin panel"
  link in the user menu is only rendered when `role == Admin || role ==
  Auditor`. Regular users do not see it.

## Non-goals

- No change to the login authentication logic — only rendering.
- No new OIDC flows.
- The user menu dropdown (if any) is not changed; only the "Back to admin
  panel" link is added/conditional.

## Proposed design

### i18n keys (5 new)

```
login_title_admin          → "Sign in to manage sui-id"
login_body_admin           → "Use an administrator or auditor account."
login_title_oidc           → "Sign in to continue to {0}"  (interpolation slot)
login_body_oidc            → "sui-id will verify your identity for this application."
login_title_self_service   → "Sign in to manage your security"
login_body_self_service    → "Manage MFA, passkeys, sessions, and password."
```

Note: `login_title_oidc` requires a single interpolation slot for the
client name. Pattern: `("{0}", &[client_name])` using the existing
`fmt_*` closure pattern already in the codebase.

### LoginContext enum placement

`LoginContext` lives in `sui-id-web` (as a render-data enum passed to the
render function), not in `sui-id-core`. Derivation logic (which context to
choose) lives in the handler. This matches the "renderers are pure; handlers
decide context" principle (§11 of the handoff).

### SelfServiceShell navigation active item

Uses the same path-prefix matching used by the admin settings tabs
(`path.starts_with("/me/security/mfa")` etc.).

## Data model impact

None.

## API impact

None (routes unchanged).

## Testing strategy

- Unit: `LoginContext` derivation — OIDC `next` → `OidcAuthorize`, `/me/`
  `next` → `SelfService`, other → `AdminPanel`.
- Unit: invalid client_id in OIDC `next` → `AdminPanel` fallback (not
  `OidcAuthorize`).
- Text-leak gate continues to pass (5 new i18n keys replace hardcoded copy).
- Visual: `/me/security/sessions` renders within `SelfServiceShell` with
  nav tabs visible.

## Migration strategy

None. The shell change is purely additive rendering logic.

## Rollout plan

Ships as v0.73.0.

## Risks and mitigations

- *Risk:* the `client_name` interpolation slot `{0}` is not handled
  correctly in all locales. Mitigation: the interpolation pattern already
  exists in the codebase for similar use cases; test each locale.
- *Risk:* SelfServiceShell ELOC pushes `chrome.rs` over 500 ELOC.
  Mitigation: split into `chrome/self_service_shell.rs` sub-module if needed.

## Acceptance criteria

- Login page title and body copy change depending on `next` context.
- When `next` refers to an OIDC authorize request, `client_name` is the
  registered app name, not any user-supplied value.
- `/me/security/*` renders within `SelfServiceShell` with horizontal nav.
- Admin/auditor users see "Back to admin panel"; regular users do not.
- 5 new i18n keys in all locale files; CI text-leaks gate passes.
- 0 warnings; baseline test suite green.

## Open questions

- Should the `LoginContext` derivation read the pending authorize request
  from the DB (requires the `state` parameter to be stored) or from the
  query params? Recommendation: read from the DB-backed pending authorize
  state if it exists; the `state` parameter is already stored for PKCE
  flows. If not found, fall back to `AdminPanel`. This avoids trusting
  any URL parameter.
