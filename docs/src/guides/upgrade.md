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
