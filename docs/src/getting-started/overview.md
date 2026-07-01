# Overview

## What sui-id does

sui-id is an **OpenID Connect (OIDC) Identity Provider**. It lets your
applications authenticate users without building a login system themselves.

A user flow with sui-id looks like this:

1. Your application redirects the user to sui-id's `/oauth2/authorize`.
2. sui-id handles login (password + optional MFA / passkey).
3. sui-id returns an authorization code to your application.
4. Your application exchanges the code for an ID token and access token.
5. The tokens are signed by sui-id's Ed25519 key, verifiable via JWKS.

## What makes it different

| Property | sui-id | Typical self-hosted IdP |
|---|---|---|
| Binary count | 1 | 3–10 (app + DB + cache + …) |
| Runtime deps | none | JVM / Node / container runtime |
| Database | SQLite (bundled) | PostgreSQL / MariaDB |
| Backup | `cp sui-id.sqlite sui-id.key` | dump + restore procedure |
| Sensitive-column encryption | XChaCha20-Poly1305 | depends on DB driver |
| JWT signing | Ed25519 only | RS256 default |

## Scope

### Single realm

sui-id is a **single-realm IdP**. All users share one flat namespace. There
is no `tenant_id`, no organisation table, no group table, and no per-tenant
scoping in the schema.

This is a deliberate design choice: it keeps the schema and the admin UI
minimal. If you need per-tenant user isolation, see RFC 025 in the
[roadmap](https://github.com/nabbisen/sui-id/blob/main/ROADMAP.md).

### Human roles

Three human roles exist in sui-id:

- **Admin** — full read/write access to all admin surfaces. Created at
  setup; additional admins can be added and role-changed by any existing admin.
- **Auditor** — read-only access to all admin surfaces. Cannot mutate any
  state; all POST/DELETE routes return 403 for this role. Useful for
  compliance reviewers, on-call SREs, or incident-response staff who need
  visibility without mutation capability.
- **User** — end-user self-service only (`/me/*`). This is the default role
  for all accounts created through the OIDC consent flow or admin user creation.

All three roles have access to `/me/security/*` (password, MFA, passkeys,
sessions) and `/me/apps` (review and revoke OAuth consent grants).

### Supported flows

- Authorization Code + PKCE (S256) — the only supported grant type for interactive login.
- Refresh Token grant (token rotation on every use, theft-detection family revocation).
- Client Credentials — not supported (no machine-to-machine grant).
- Implicit / Hybrid / Device Flow — not supported.

### Supported (as of v0.76.x)

- LDAP / Active Directory external user source (RFC 005) — read-only bind; shadow
  rows for sessions and MFA; local-first cascade with fail-soft.
- Dynamic client registration — RFC 7591 (RFC 008) — initial-access-token gated;
  clients start disabled pending admin approval.
- Federation as upstream OIDC relying party (RFC 004) — "Sign in with X" via any
  OIDC-compliant provider; `link_only` or `provision_on_first_login` modes; local
  MFA enforced post-callback.
- Prometheus metrics endpoint (RFC 006) — disabled by default; session-cookie or
  bearer-token auth.

### Not in scope

- SAML IdP or SP
- Multi-tenant namespacing (design in RFC 025; no schedule)

## Design principles

1. **Accessible by Default** — readable labels, keyboard-first, focus-visible ring,
   status conveyed by text not colour alone.
2. **Minimal by Responsibility** — one screen, one task; dangerous operations
   isolated to a step-up + confirmation flow.
3. **Safe by Workflow** — failure modes fall to the safe side; audit log records
   outcomes; secrets never appear in logs.
