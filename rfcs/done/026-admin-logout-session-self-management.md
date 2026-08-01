# RFC 026 — Admin panel: logout and session self-management

**Status.** Implemented (v0.29.13)
**Priority.** High. The absence of a logout button is a security gap: an
operator sitting at a shared machine has no in-UI way to end their session.
**Tracks.** Operator UX observation — v0.29.x.
**Touches.** `crates/sui-id-web/` (header component), `crates/sui-id/src/handlers/admin.rs` (logout route), `docs/`.

## Problem

The admin panel currently has no logout button. Once an operator signs in,
they can only end their session by:

- Closing the browser (relies on cookie expiry, not server-side revocation).
- Using the Sessions list in `/me/security` (not obvious, and requires knowing
  that page exists).
- Waiting for the session to expire.

This is both a security gap (shared terminals) and a discoverability failure
(new operators expect a logout action to exist).

## Requirements

After this RFC ships:

1. A **Logout** action is reachable from every admin panel page in at most
   two clicks / keyboard steps.
2. Logging out revokes the server-side session row (`sessions.revoked_at`
   stamped) in addition to clearing the session cookie.
3. The action is not hidden behind settings; it is visible to any logged-in
   admin without step-up or confirmation (logout is a safe, easily undone
   action — the admin can simply sign back in).
4. After logout, the operator is redirected to `/admin/login` with a
   "Signed out." flash message.
5. The CSRF token is validated on the logout POST to prevent logout-CSRF
   (an attacker forcing a victim out of their session).

## Design

### Placement

A **user menu** or persistent **header action** in the admin panel shell,
consistent with the header/nav established by RFC 017 and RFC 023. Options:

**Option A — Header username + dropdown.**
The logged-in admin's username appears in the top-right corner of the
admin shell header. Clicking it opens a small dropdown containing:
- "Your profile" → `/me/security`
- "Sign out" → POST `/admin/logout`

**Option B — Persistent "Sign out" link in the sidebar nav.**
Below the nav items (Dashboard / Users / Clients / Settings / Audit) a
"Sign out" text link is always visible at the bottom of the sidebar.

Recommendation: **Option A** — aligns with the UI/UX design document's
admin panel navigation pattern and matches user expectations from other
admin tools. The dropdown provides a natural home for future self-service
actions (language picker, etc.) without sidebar bloat.

### Route

```
POST /admin/logout
```

- Requires a valid session cookie (otherwise redirect to login).
- Validates CSRF token from the request body.
- Calls `session::revoke(&db, session_id)`.
- Clears the session cookie.
- Redirects to `/admin/login` with flash "Signed out."

This complements the existing per-session revocation in `/me/security` and
the admin-forced revocation in `/admin/users/{id}/sessions`.

### Session context display (optional, same RFC)

The dropdown (Option A) could also surface the current session start time
("Signed in 2 h ago") as a reassurance that the right account is active.
This is low-cost given the dropdown exists.

## Tests

- POST `/admin/logout` with valid CSRF → session revoked, cookie cleared, redirected.
- POST `/admin/logout` without CSRF → rejected 403.
- After logout, GET `/admin/` → redirected to login (session gone).
- Logout is accessible from keyboard (tab navigation reaches the button).

## Accessibility

The logout trigger must be a `<button>` (or a form submit), not a bare
`<a>` link, so that its action (stateful POST) is clear to screen readers.
`aria-label="Sign out"` on icon-only variants.

## Security notes

Server-side revocation (stamping `revoked_at`) is essential: clearing the
cookie alone is not sufficient because the cookie could be captured before
logout. The `session::resolve` path already checks `revoked_at`, so
revocation takes effect immediately on the next request from any device
holding that session ID.
