# RFC 038 — OIDC consent screen

**Status.** Implemented (v0.39.0)
**Priority.** Medium. The UI/UX design document (P.10 / overview screen 15)
explicitly shows a consent step in the OIDC flow. The current authorize
handler has a comment: "sui-id deliberately does not show a separate
consent screen in the first-party model." This RFC implements it as a
per-client opt-in, completing the OIDC UX.  
**Scoped from.** RFC 008 (third-party-posture bundle). This RFC covers
only the consent screen and stored consent; dynamic client registration
and per-user application management remain in RFC 008 proper.  
**Touches.** New migration (0025), `crates/sui-id-store` (new repo),
`crates/sui-id-core/src/oidc.rs` (consent gate), `crates/sui-id-web`
(new screen), `crates/sui-id/src/router.rs` and
`crates/sui-id/src/handlers/oidc.rs` (new routes).

## Consent policy

Each OIDC client gets a `consent_policy` column (TEXT, default `"none"`):

| Policy | Behaviour |
|---|---|
| `none` | No consent screen — existing first-party behaviour. |
| `first_time` | Show on first authorization; skip if user has approved at least the requested scopes. |
| `always` | Always prompt, even if a prior approval exists. |

Default for new clients: `none` (backwards-compatible).
Operators set policy in the client edit form.

## Schema

### Migration 0025

```sql
-- Per-client consent policy
ALTER TABLE clients ADD COLUMN consent_policy TEXT NOT NULL DEFAULT 'none';

-- Stored user approvals
CREATE TABLE user_consent (
    user_id        TEXT NOT NULL,
    client_id      TEXT NOT NULL,
    granted_scopes TEXT NOT NULL,  -- space-separated
    granted_at     TIMESTAMP NOT NULL,
    PRIMARY KEY (user_id, client_id),
    FOREIGN KEY (user_id)   REFERENCES users   (id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES clients (id) ON DELETE CASCADE
);
```

## Authorize flow change

```
GET /oauth2/authorize?...
  → validate params (unchanged)
  → require authenticated session (unchanged)
  → check consent_policy:
      none         → issue code immediately (unchanged)
      first_time   → lookup user_consent row
                     if granted_scopes ⊇ requested_scopes → issue code
                     else → render consent screen
      always       → always render consent screen
```

The consent screen receives a short-lived `consent_session` cookie carrying
the serialized authorization parameters, so the `GET /oauth2/authorize` query
string does not need to be threaded through the consent POST.

## Consent screen

```
[App name]                          [client name from DB]
[Scope request]                     e.g. "openid profile email"

This application wants access to:
  • Your profile (name, preferred language)
  • Your email address

[Approve]  [Deny]
```

Scopes are displayed as human-readable labels (defined in i18n).

## New routes

```
GET  /oauth2/consent   → render_consent (from consent_session cookie)
POST /oauth2/consent   → consent_post (approve/deny)
```

## Security

- Consent screen does not re-authenticate — the user is already authenticated.
- `consent_session` cookie is `HttpOnly; SameSite=Lax; Max-Age=300` (5 min).
- CSRF token required on POST.
- Deny → redirect to `redirect_uri?error=access_denied`.
- Approve → store in `user_consent`, issue code.

## Tests

- E2e: client with `consent_policy=none` issues code without consent screen.
- E2e: client with `consent_policy=first_time` shows screen on first auth,
  skips on second auth with same scopes.
- E2e: client with `consent_policy=always` shows screen every time.
- E2e: Deny redirects to `redirect_uri?error=access_denied`.

## Version

Minor bump (new migration, new routes, new screen).
