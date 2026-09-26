# Upgrade guide

## General procedure

1. **Back up** before upgrading:
   ```bash
   sui-id backup --config sui-id.toml --to sui-id-pre-upgrade.tar
   ```
   Do not use `cp`: the database runs in WAL mode, so a copy of the
   `.sqlite` file can miss committed data.

2. **Stop** the running instance (SIGTERM; sui-id finishes in-flight requests
   before exiting).

3. **Replace** the binary with the new version.

4. **Start** the new binary. Migrations run automatically on startup.
   Check stderr for any migration errors.

5. **Verify** — open the admin panel and confirm the version shown in
   Settings → Advanced.

## Migration behaviour

sui-id runs database migrations forward-only on startup. There is no
`down` migration. The schema version is recorded in the database, and each
migration runs once.

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

- `metrics_enabled` / `metrics_listen_addr` under `[server]` — enable Prometheus metrics endpoint
- `[[user_sources]]` — LDAP external user source
- `[[federation_providers]]` — upstream OIDC provider

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
  The outbox worker starts automatically; no configuration change is needed.

## Rollback

Rollback is not supported. If a bad release is deployed:

1. Stop the new binary.
2. Restore the pre-upgrade backup:
   `sui-id restore --config sui-id.toml --from sui-id-pre-upgrade.tar --force`.
3. Start the previous binary.

An older binary **refuses to start** against a database that a newer binary has
migrated. It reads the recorded schema version before it touches the file, and
stops if that version is newer than it understands; **nothing is written to the
database** when it does. The recovery is to run the newer binary again, or to
restore the pre-upgrade backup, as above.

What the refusal looks like: one line on stderr, first and alone (`sui-id:
refusing to run: the database at … is at schema version 44, but this sui-id
0.78.0 understands up to 43 …`), naming the path, both versions and, when the
database recorded it, the release that migrated it, and exit code **65**.
Nothing is written to the database. There is no override flag. A schema version
that cannot be read, or that is missing from a database that has tables, is
refused the same way. The full text, the exit codes and the systemd setting
(`RestartPreventExitStatus=65`) are in the
[deployment guide](deployment.md#11-upgrades).

**One binary version per database at a time**: the check is made when a binary
opens the database, so a newer binary migrating while an older one is already
running is not detected by the older one. Stop the old instance first.
