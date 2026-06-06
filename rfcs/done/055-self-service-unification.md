# RFC 055 — Consolidate self-service onto `/me/security/*`

**Status.** Implemented (v0.44.0)
**Priority.** P0 — Phase C (v0.44.0)
**Tracks.** Decision item from v0.41.0 codebase review:
"Two parallel implementations of user self-service —
`/admin/profile` and `/me/security/*`. Either consolidate or
remove one."
**Touches.** `crates/sui-id/src/router.rs`,
`crates/sui-id/src/handlers/admin.rs` (profile_* removal),
`crates/sui-id/src/handlers/me_security.rs` (mutative routes
added), `crates/sui-id-web/src/pages.rs`
(render_profile removal, mutative form action URLs updated),
`crates/sui-id-web/src/layout.rs` (Nav: "Security" entry),
`crates/sui-id-i18n/src/strings.rs` and locale files,
`crates/sui-id/tests/e2e/profile_*.rs` and `me_security_*.rs`.

## Background

v0.40.0 introduced `/me/security/*` as the modern self-service
surface: a tabbed five-page UI (Overview, MFA, Sessions,
Passkeys, Language) with proper navigation affordances. v0.41.0
left `/admin/profile` as the legacy single-page version,
linked from the admin Nav. The result at v0.43.0 is that:

- The "Profile" link in the admin Nav goes to `/admin/profile`
  (the legacy single page).
- The new tabbed UI at `/me/security/*` exists and is partially
  reachable via direct URL, but no link points to it.
- The **mutative** MFA and passkey routes still live under
  `/admin/profile/*`. So a user landing on the new tabbed
  `/me/security/mfa` sees their MFA status but the enrollment
  form POSTs to `/admin/profile/mfa/enroll/start`.

The split was never an intentional split-system; it's drift from
unfinished consolidation. Two screens, one user — pick one.

## Goal

Exactly one path leads from the admin Nav to "my security
settings." Every action on that page (change password, enrol
MFA, register passkey, see sessions, choose language) works
under the same route prefix and shows a confirmation. The
legacy `/admin/profile` code path is removed.

## Design

### Route map

| Action | Before (`/admin/profile/*`) | After (`/me/security/*`) |
|--------|---|---|
| View overview | GET `/admin/profile` | GET `/me/security/overview` (existing) |
| Change language | POST `/admin/profile/lang` | POST `/me/security/language` (existing) |
| TOTP enroll start | POST `/admin/profile/mfa/enroll/start` | POST `/me/security/mfa/enroll/start` |
| TOTP enroll confirm | POST `/admin/profile/mfa/enroll/confirm` | POST `/me/security/mfa/enroll/confirm` |
| MFA disable | POST `/admin/profile/mfa/disable` | POST `/me/security/mfa/disable` |
| Regenerate recovery codes | POST `/admin/profile/mfa/recovery-codes/regenerate` | POST `/me/security/mfa/recovery-codes/regenerate` |
| Webauthn register start | POST `/admin/profile/webauthn/register/start` | POST `/me/security/passkeys/register/start` |
| Webauthn register complete | POST `/admin/profile/webauthn/register/complete` | POST `/me/security/passkeys/register/complete` |
| Webauthn delete | POST `/admin/profile/webauthn/{id}/delete` | POST `/me/security/passkeys/{id}/delete` |

### Handler migration

The handlers themselves are mostly fine — they take `CurrentUser` and
operate on the signed-in user. They will be **moved**
from `handlers/admin.rs` to `handlers/me_security.rs` (or a sibling
sub-module `handlers/me_security/mfa.rs` and `handlers/me_security/passkeys.rs`)
and renamed for clarity (`profile_mfa_enroll_start` → `mfa_enroll_start`,
`webauthn_register_start` → `passkey_register_start`, etc.). The
inner business-logic calls into `sui-id-core` are unchanged.

### Compatibility / transition

Two-step:

1. **GET `/admin/profile`**: 301 Permanent Redirect to
   `/me/security/overview`. Honours user bookmarks.
2. **POST `/admin/profile/*`** mutative routes: **removed**.
   These are only called from forms rendered server-side by
   the legacy `render_profile`. Once `render_profile` is
   removed and the forms render under `/me/security/*` with
   the new action URLs, there is no caller left for the old
   POST endpoints. 301 redirects on POST are a footgun (RFC 7231
   §6.4.2 advises against changing method on redirect, and
   browsers historically downgrade POST→GET anyway), so this is
   correct.

### View change

- `render_profile` removed (large function, ~250 LOC).
- The five `render_me_*` views (`render_me_overview`,
  `render_me_mfa`, `render_me_passkeys`, `render_me_sessions`,
  `render_me_language`) gain whatever pieces of `render_profile`
  belong to them: MFA enrollment QR + form into `render_me_mfa`,
  passkey register button + list into `render_me_passkeys`, etc.
- The Nav gains a "Security" entry that points to
  `/me/security/overview`. The old "Profile" entry is replaced.
- All form `action=` URLs in the relevant `render_me_*` functions
  switch from `/admin/profile/*` to `/me/security/*`.

### i18n

The string table loses `profile_*` keys that become unused
(`profile_title`, `profile_lede`, `profile_mfa_section`, etc.).
Many `me_security_*` keys already exist and just get extended
where new sub-sections move in.

The Nav gains `nav_security` (replaces `nav_profile`).

### Test plan

1. **Unit**: `render_me_mfa` and `render_me_passkeys` build cleanly with
   the moved-in form bodies.
2. **E2E**: existing `profile_mfa_*` tests rewritten to hit
   `/me/security/mfa/*` URLs. The body assertions stay the same;
   only the path prefix changes.
3. **Redirect**: a new e2e asserts GET `/admin/profile` returns
   301 to `/me/security/overview`.
4. **No regressions**: full e2e suite passes.
5. **Manual**: log in, click "Security" in Nav → overview tab.
   Click each tab → renders. Trigger each mutative action
   (enrol TOTP, regenerate codes, register passkey, delete a
   passkey, revoke a session, change language) → success +
   the relevant tab re-renders with the change visible.

## Open questions

None. Decisions made:
- Path prefix: `/me/security/*` wins (modern, tabbed).
- Method for retired POST endpoints: **remove** (not 301).
  Documented in CHANGELOG as a breaking change for anyone
  scripting against `/admin/profile/*` POSTs; ~zero such users
  expected for a self-hosted IdaaS.
- Path for redirect: GET only.

## Rollout

Single release. v0.44.0 ships with the new routes and the old
`/admin/profile` GET as a 301; the legacy POST routes simply
return 404 (or whatever the router default is) since they're
removed.

The CHANGELOG flags this as a soft breaking change for any
external integrators who may have automated against the old
URLs. Self-service URLs are not part of the OIDC public API so
this is internal-only.
