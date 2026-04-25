# Roadmap

This is a sketch of where sui-id is heading. Items are loose; nothing here is
a promise.

## Near term

(None outstanding — all near-term items shipped in v0.3.0. Next batch
draws from medium-term.)

## Medium term

- **Per-client scope policy.** Today every active client may request any
  scope. Allow clients to declare a permitted scope set.
- **Per-client `post_logout_redirect_uris`.** Today logout reuses the
  authorization `redirect_uris` set; a real deployment may want a separate
  list. Adding it is a schema migration.
- **MFA.** TOTP first; WebAuthn second. Both are big enough to be their own
  releases.

## Longer term, less certain

- **Federation.** Acting as an OIDC client to an upstream IdP, mapping the
  result onto a sui-id user.
- **Pluggable user backends.** A read-only LDAP shim, perhaps. The current
  storage layer assumes sui-id owns the user table.
- **Metrics.** A Prometheus endpoint behind admin auth.

## Done

- Per-IP rate limiting on `/admin/login`, `/oauth2/token`, `/setup`.
- Background GC of expired authorization codes, sessions, and refresh tokens.
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

## Explicitly **not** on the roadmap

- SAML.
- Implicit or hybrid OIDC flows.
- Dynamic client registration (RFC 7591) over the public internet.
- A built-in clustering / multi-master mode.

The "not" list is not a list of bad ideas. It is a list of things that pull
sui-id in a direction it isn't trying to go.
