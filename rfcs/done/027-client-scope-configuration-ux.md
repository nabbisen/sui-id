# RFC 027 — OAuth client: scope configuration UX and operator guidance

**Status.** Implemented (v0.29.13)
**Priority.** High. The current state leaves operators blocked at first
integration: a new client has no permitted scopes, so every authorization
request fails with a confusing protocol error before the operator knows
what to fix.
**Tracks.** Operator UX observation — v0.29.x.
**Touches.** `crates/sui-id-web/` (client create/edit forms), `crates/sui-id/src/handlers/admin.rs` (client handlers), `docs/`.

## Problem

Two related issues were observed during first-use:

### Issue A — `scope "email" is not permitted for this client`

When an operator registers a client and then attempts an OIDC authorization
flow from an external application requesting the `email` scope, they receive:

```
Protocol error
scope "email" is not permitted for this client
```

**Root cause.** `clients.allowed_scopes` defaults to an empty string `""`.
The application correctly enforces the policy, but the operator has no
in-UI signal that:

1. `allowed_scopes` exists and is enforced.
2. Its default value prevents all scopes from working.
3. They must explicitly configure it.

The client creation form accepts an `allowed_scopes` field but does not
provide meaningful defaults, help text, or a list of known scopes.

### Issue B — "how do I bind a client to specific users?"

The same operator expected a per-client user allowlist: "which users are
permitted to authenticate with this client." sui-id intentionally does not
have this mechanism — it is a **single-realm IdP** where all users are
shared across all clients. This is a correct design choice for the target
audience (small self-hosted deployments), but it is not documented in a
place operators encounter before they get blocked.

## Requirements

After this RFC ships:

### A — `allowed_scopes` UX

1. **Default on client creation.** New clients are created with a sensible
   default `allowed_scopes` value. The recommended default is `openid profile email`
   (the three scopes most commonly needed for basic OIDC integration). The
   field must remain editable so operators can restrict it.
2. **Scope picker or help text.** The client create/edit form displays the
   set of scopes sui-id recognises (`openid`, `profile`, `email`,
   `offline_access`) with a short description of each. A checkbox list or
   multi-select is preferred over a raw text field.
3. **Inline error context.** When the authorization endpoint rejects a scope,
   the error description (visible in the OAuth redirect or the direct
   `/oauth2/authorize` error page) names the client and its current
   `allowed_scopes` value so the operator knows exactly where to fix it:
   > `scope "email" is not permitted for client "my-app" (allowed: "openid profile")`

### B — Single-realm model documentation

1. **Admin UI copy.** The client creation page includes a one-line note:
   > "All sui-id users can authenticate with any client. There is no
   > per-client user allowlist — this is by design."
2. **Operator guide.** `docs/operators.md` gains a "User–client relationship"
   section explaining the single-realm model and pointing to RFC 025 as the
   multi-tenant expansion path.
3. **Scope vs. user restriction.** The same section explains that
   `allowed_scopes` controls *what information a client may request*, not
   *which users may use the client*.

## Design notes

### Known scope list

The following scopes are defined in sui-id today:

| Scope | Claims included | Notes |
|---|---|---|
| `openid` | `sub`, `iss`, `aud`, `exp`, `iat` | Required for OIDC |
| `profile` | `name`, `preferred_username`, `locale` | Basic identity |
| `email` | `email`, `email_verified` | Requires email set on user |
| `offline_access` | (enables refresh tokens) | RFC 6749 §1.5 |

The UI should present exactly these four. New scopes added in future
releases should be added to this table and reflected in the form.

### Default `allowed_scopes` for existing clients

Migration: existing clients that have `allowed_scopes = ""` (the empty
default) should remain unchanged — silently expanding their scope would be
a security change. The operator guide should call out this situation and
recommend reviewing existing clients after upgrading.

## Tests

- Client creation with no `allowed_scopes` input → row has `"openid profile email"`.
- Authorization request for `email` scope → succeeds for a client that has `email` in `allowed_scopes`.
- Authorization request for `email` scope → fails with an improved error message for a client whose `allowed_scopes` does not include `email`.
- Error message includes the client name and current allowed list.
