# Upgrade guide

## General procedure

1. **Back up** before upgrading:
   ```bash
   cp sui-id.sqlite sui-id.sqlite.bak
   cp sui-id.key    sui-id.key.bak
   ```

2. **Stop** the running instance (SIGTERM; sui-id finishes in-flight requests
   before exiting).

3. **Replace** the binary with the new version.

4. **Start** the new binary. Migrations run automatically on startup.
   Check stderr for any migration errors.

5. **Verify** — open the admin panel and confirm the version shown in
   Settings → Advanced.

## Migration behaviour

sui-id runs database migrations forward-only on startup. There is no
`down` migration. Each migration is idempotent: running the same migration
twice is safe.

Migrations that add columns use `ADD COLUMN` with a default value or
`NULL`, so they run without locking the entire table.

## Version-specific notes

### v0.76.x

**New migrations (0033–0038).** All run automatically and are backwards-compatible
(`ADD COLUMN` with defaults, new tables):

| Migration | Table | What changed |
|---|---|---|
| 0033 | `server_settings` | `metrics_token_hash` column for Prometheus auth |
| 0034 | `users` | `source`, `external_stable_id` columns for LDAP shadow rows |
| 0035 | `clients` | `registered_via`, `logo_uri`, `homepage_uri`, `privacy_policy_uri`, `tos_uri` |
| 0036 | new: `scope_definition`, `client_registration_token` | Scope catalog (seeded) and RFC 7591 registration tokens |
| 0037 | new: `federation_provider` | Upstream OIDC IdP configurations (encrypted secrets) |
| 0038 | new: `federation_link` | Per-user upstream identity links |

**New routes.** If you run sui-id behind a strict allowlist firewall or WAF,
add these paths:

- `GET /metrics` — Prometheus metrics (only if `metrics_enabled = true`)
- `POST /oauth2/register` — RFC 7591 dynamic client registration
- `GET /auth/federated/{slug}/start` — Federation sign-in initiation
- `GET /auth/federated/callback` — Federation sign-in callback
- `GET /auth/federated/link` — Federation link-only flow

**New configuration sections.** Optional; existing deployments that do not
add them continue to work without change:

- `[metrics]` — enable Prometheus metrics endpoint
- `[[user_source]]` — LDAP external user source
- `[[federation_provider]]` — upstream OIDC provider

**New CLI subcommands:**

- `sui-id admin rotate-metrics-token` — generate/rotate a Prometheus bearer token
- `sui-id admin issue-registration-token` — issue an RFC 7591 initial-access token

### v0.36.x

- Dangerous operations (user disable/delete, MFA reset, client delete,
  signing key delete) now require navigating to a confirmation screen
  rather than accepting a browser `confirm()` dialog.
- The admin panel now shows operator action prompts when SMTP is
  unconfigured, HIBP is off, or `cookie_secure` is false.
- A `--dev` banner is displayed on every page when running in dev mode.

### v0.34.x

- Migration 0024 adds a `locale` column to `email_outbox`. This is a
  backwards-compatible schema change.
- Chinese Simplified (`zh`) locale is now supported.

### v0.33.x

- Migration 0023 adds the `email_outbox` table for async mail delivery.
  The outbox worker starts automatically; no configuration change is needed
  unless you want to change retry parameters.

## Rollback

Rollback is not supported. If a bad release is deployed:

1. Stop the new binary.
2. Restore the backup SQLite file.
3. Start the previous binary.

If the new binary ran migrations, the old binary may refuse to start
because the schema version is higher than it understands. In that case,
restoring from the SQLite backup is the only recovery path.
