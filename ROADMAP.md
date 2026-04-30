# Roadmap

This is a sketch of where sui-id is heading. Items are loose; nothing here is
a promise.

## Near term

(None outstanding — all near-term items shipped in v0.3.0. Next batch
draws from medium-term.)

## Medium term

The big-ticket auth features have all shipped (TOTP MFA, WebAuthn
passkeys, scope policy, post-logout URIs, signing key rotation, CSRF
tokens, edit page for clients, admin-initiated MFA reset). The
authentication and authorization surface is, for v0.x, broadly
complete; the natural next steps are operability and quality work
rather than new auth primitives.

The Medium term list is now empty — the items previously here
(`cargo audit` integration, the deployment guide) shipped in
v0.10.1 / v0.10.2.

## Longer term, less certain

- **Federation.** Acting as an OIDC client to an upstream IdP, mapping
  the result onto a sui-id user.
- **Pluggable user backends.** A read-only LDAP shim, perhaps. The
  current storage layer assumes sui-id owns the user table.
- **Metrics.** A Prometheus endpoint behind admin auth.

## Done

- Per-IP rate limiting on `/admin/login`, `/oauth2/token`, `/setup`.
- Background GC of expired authorization codes, sessions, refresh
  tokens, pending-MFA rows, and WebAuthn ceremonies.
- Audit logging of authentication outcomes (success/failure).
- `/healthz` endpoint suitable for liveness/readiness probes.
- crates.io publication metadata; binary distributable via
  `cargo install sui-id`.
- OpenID Connect RP-Initiated Logout 1.0 (`/oauth2/logout`).
- `server.trusted_proxies` opt-in for `X-Forwarded-For`-derived client IP.
- Annotated `sui-id.example.toml` configuration template.
- `sui-id backup` / `sui-id restore` subcommands with hot SQLite snapshot.
- `docs/threat-model.md` and a documentation index in the README.
- Signing key rotation UI with a JWKS grace window.
- CSRF tokens on every admin form (synchronizer-token + double-submit cookie).
- Per-client scope policy enforced at `/oauth2/authorize`.
- Per-client `post_logout_redirect_uris` (separate from `redirect_uris`).
- TOTP MFA (RFC 6238) with single-use recovery codes.
- Edit page for existing clients (name / redirect URIs / allowed scopes /
  post-logout redirect URIs).
- WebAuthn / passkey support (registration, authentication, multiple
  credentials per user, list / delete UI).
- Admin-initiated MFA reset (lifts every second factor for a user
  whose authenticator app, recovery codes, and passkeys have all
  been lost).
- `docs/deployment.md` — chronological install walkthrough from a
  fresh Linux server to a hardened production deployment.
- `cargo audit` CI integration — `.github/workflows/audit.yml` runs
  on push, on PR, and weekly. The companion `ci.yml` covers build,
  test, fmt, and clippy on every change.
- Token introspection (RFC 7662) and token revocation (RFC 7009).
  Confidential clients can introspect and revoke their own tokens
  via the standards-blessed endpoints; both are advertised through
  the OIDC discovery document.

## Explicitly **not** on the roadmap

- SAML.
- Implicit or hybrid OIDC flows.
- Dynamic client registration (RFC 7591) over the public internet.
- A built-in clustering / multi-master mode.

The "not" list is not a list of bad ideas. It is a list of things that pull
sui-id in a direction it isn't trying to go.
