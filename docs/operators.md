# Operator's guide

This is a *reference* for the operational surface of sui-id —
configuration fields, the master key, the audit log, GC, and routine
tasks.

If you are setting sui-id up for the first time, read
[deployment.md](deployment.md) first; that guide walks the
chronological "what do I run, in what order" path from a fresh
server to a working production install.

If you are looking for how to point an application at sui-id, see
[integrators.md](integrators.md).

## Installing

sui-id is one binary. Build it from source:

```bash
git clone <repo-url> sui-id && cd sui-id
cargo build --release
install -m 0755 target/release/sui-id /usr/local/bin/sui-id
```

Or copy the binary from a release archive when one is published.

## Configuring

sui-id reads a single TOML file. Generate a starter:

```bash
sui-id --print-sample-config > /etc/sui-id/sui-id.toml
```

The fields:

```toml
[server]
listen_addr = "127.0.0.1:8801"   # bind address
issuer = "https://idp.example.com" # external URL clients see (no trailing slash)
cookie_secure = true             # set true behind HTTPS

[storage]
db_path = "/var/lib/sui-id/sui-id.sqlite"
key_file = "/var/lib/sui-id/sui-id.key"

[tokens]
access_lifetime_secs = 900        # 15 minutes
id_token_lifetime_secs = 900
refresh_lifetime_secs = 1209600   # 14 days

[log]
format = "json"                   # "fmt" for human-readable
filter = "info,sui_id_bin=info"   # tracing-subscriber filter expression
```

## The master key

The master key is a 32-byte secret used to encrypt sensitive columns. sui-id
resolves it in this order:

1. `SUI_ID_MASTER_KEY` environment variable, base64-encoded.
2. The `key_file` path from config.
3. If neither is present, sui-id generates a fresh key, writes it to the
   `key_file` path with permissions `0600`, and logs a warning.

**The key is not stored in the SQLite file.** This is the whole point of the
arrangement: an attacker who gets only the database file cannot decrypt
refresh tokens or signing-key material.

**Back the key up.** Without it, the encrypted columns are unrecoverable.
Keep at least one offline copy in your normal secrets store. Treat it the
same way you would treat a TLS private key.

## First run

1. Start sui-id. It will print a setup token to stderr that looks like:

   ```
   =====================================================
     sui-id has not been initialized yet.
     Open  https://idp.example.com/setup
     Setup token (one-time, stays only in this process):
       <random base64 string>
   =====================================================
   ```

2. Open the `/setup` URL in a browser. Paste the setup token and create the
   first administrator. The token only exists for the lifetime of this
   process; if you restart before finishing setup, you'll see a fresh one.

3. The wizard logs you in automatically and redirects to the dashboard.

## Backup and restore

A working backup contains two files: the SQLite database and the master
key. sui-id ships a subcommand that bundles both into a single tarball
with restrictive permissions:

```bash
sui-id backup --to /var/backups/sui-id-$(date +%F).tar --config /etc/sui-id/sui-id.toml
```

The output file is created with mode `0600`. The SQLite snapshot is
produced via `VACUUM INTO`, which is safe to run while sui-id is serving
traffic — no need to stop the daemon for backups.

To restore, point `sui-id restore` at the tarball:

```bash
sui-id restore --from /var/backups/sui-id-2026-04-25.tar --config /etc/sui-id/sui-id.toml
```

By default `restore` refuses to overwrite an existing database or key
file at the destination paths. Pass `--force` if you really mean it
(typically only when recovering onto a fresh host).

> **Be careful where the tarball ends up.** It contains the master key.
> Anyone who can read the archive can decrypt the SQLite columns inside
> it. Treat the backup the same way you treat the key file itself.

## Reverse proxying

sui-id binds plain HTTP and is intended to sit behind a reverse proxy that
terminates TLS. A minimal nginx fragment:

```nginx
server {
    listen 443 ssl http2;
    server_name idp.example.com;

    ssl_certificate     /etc/letsencrypt/live/idp.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/idp.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8801;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-For $remote_addr;
    }
}
```

When proxying, set `cookie_secure = true` in `[server]` so the session
cookie is only ever sent over HTTPS.

## Logging

sui-id uses [`tracing`](https://docs.rs/tracing). Pick `format = "json"` if
you ship logs to a log aggregator that does its own structured-field
indexing; pick `format = "fmt"` for the development experience.

The log filter is a [`tracing-subscriber` env-filter expression](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html).
The defaults emit `info` for the project crates and stay quiet about
upstream chatter.

The setup token is **not** written through the tracing pipeline. It only
goes to stderr so it cannot accidentally land in your log aggregator.

### Request correlation

Every HTTP request gets an `X-Request-Id` header. If the caller supplies
one (and it's reasonable — alphanumeric, dash, dot, underscore, up to 64
chars) it is kept and echoed back on the response; otherwise sui-id
generates a fresh UUIDv4. The id is attached to the `tracing` span that
wraps handler execution, so every log line emitted while handling that
request — including ones from inside use cases, repos, and middleware —
carries it automatically.

In a JSON log line the request id appears under `spans[].request_id`,
alongside the `method` and `path` fields:

```json
{
  "timestamp": "2026-04-28T10:31:42.123456Z",
  "level": "INFO",
  "fields": { "message": "request completed", "status": 200, "latency_ms": 4 },
  "target": "sui_id::request_id",
  "spans": [
    { "method": "POST", "path": "/oauth2/token",
      "request_id": "0c58b960-f963-4427-86f0-d4e16938d8aa",
      "name": "request" }
  ]
}
```

Every request produces at least two of these: a `request received`
line on entry and a `request completed` line on exit (with `status`
and `latency_ms`). Anything that happens in between — security
events, internal warnings — picks up the same request_id from the
ambient span.

To correlate across a reverse proxy, configure the proxy to
generate or forward an `X-Request-Id`. Caddy:

```
header_up X-Request-Id {http.request.uuid}
```

nginx:

```
proxy_set_header X-Request-Id $request_id;
```

### Security events

A small number of events are *security-relevant* and get a structured
log line in addition to the audit-log row in the database. The two
records carry the same fields. Operators monitoring live should
filter on `event = ...`; operators reconstructing what happened
yesterday should query the `audit_log` table. Both have the same
underlying truth.

Canonical event names (these are the strings you query on; do not
expect them to be renamed without a deprecation cycle):

| Event | Meaning |
|---|---|
| `auth.login.success` | Password (and possibly MFA) check succeeded; session issued. |
| `auth.login.failure` | Password did not match, or account is disabled. |
| `auth.login.password_ok_mfa_required` | Password accepted; user redirected to MFA challenge. |
| `auth.mfa.success` | TOTP / recovery code / WebAuthn assertion accepted. |
| `auth.mfa.failure` | TOTP code or recovery code rejected. |
| `auth.session.revoked` | Session torn down by logout, admin disable, or expiry. |
| `auth.logout` | RP-initiated logout completed. |
| `mfa.admin_reset` | An administrator forcibly removed every MFA factor for a user. **Alert on this.** |
| `oauth.authorize.issued` | `/oauth2/authorize` issued an authorization code. |
| `oauth.authorize.rejected` | `/oauth2/authorize` refused (bad redirect_uri, scope outside policy, etc). |
| `oauth.token.issued` | `/oauth2/token` minted an access + ID token. |
| `oauth.token.refreshed` | A refresh token was rotated for a new access token. |
| `oauth.token.introspected` | A confidential client called `/oauth2/introspect`. |
| `oauth.token.revoked` | A confidential client called `/oauth2/revoke`. |
| `webauthn.credential.register` | A user enrolled a passkey. |
| `webauthn.credential.delete` | A user deleted one of their passkeys. |

Common queries (jq against a JSON-line log file):

```bash
# Recent failed logins.
jq -c 'select(.fields.event == "auth.login.failure")' < sui-id.log | tail

# MFA failures grouped by user.
jq -r 'select(.fields.event == "auth.mfa.failure") | .fields.target' < sui-id.log | sort | uniq -c

# Every admin-initiated MFA reset, with who reset whom.
jq -c 'select(.fields.event == "mfa.admin_reset") | {at: .timestamp, actor: .fields.actor, target: .fields.target, note: .fields.note}' < sui-id.log

# Correlate everything that happened during a given request_id.
jq -c 'select(.spans[]?.request_id == "0c58b960-f963-4427-86f0-d4e16938d8aa")' < sui-id.log
```

The audit-log table carries the same data and is the right query
target when investigating something more than a few days old (the
log file may already have been rotated):

```sql
-- Same events, from the database.
SELECT at, actor, action, target, result, note
FROM audit_log
WHERE action = 'mfa.admin_reset'
ORDER BY seq DESC;
```

## Systemd unit

```ini
[Unit]
Description=sui-id
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/local/bin/sui-id --config /etc/sui-id/sui-id.toml
Restart=on-failure
User=sui-id
Group=sui-id
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/sui-id
PrivateTmp=yes

[Install]
WantedBy=multi-user.target
```

## Signing key rotation

sui-id signs JWTs with one active Ed25519 key. The bootstrap key is
created during setup; you can rotate it later from the admin UI at
`/admin/signing-keys`.

A rotation does three things:

1. Generates a fresh Ed25519 key pair, sealed under the master key.
2. Marks the new key as the active signer. All tokens issued from this
   moment onwards use it.
3. Demotes the previous key to retired status. The retired key stays in
   `/.well-known/jwks.json` so that tokens already issued with it can
   still be verified by relying parties during their remaining
   lifetime — the JWKS "grace window".

Once the longest-lived previously-issued tokens have expired (the
default is `tokens.access_lifetime_secs = 900` seconds for access tokens
and 14 days for refresh tokens), you can delete the retired key from
the same page. Refresh tokens do not need to grace-window the *signing*
key because they are opaque, but if your `refresh_lifetime_secs` is
long, leaving the retired key in JWKS for the same window is harmless.

A reasonable cadence is "rotate every quarter, delete keys older than
the longest token lifetime." There is no built-in scheduler today;
rotations are operator-initiated.

### When to rotate immediately

- The master key file may have been exposed.
- A backup tarball is missing or unaccounted for.
- An administrator account was compromised.

In any of those cases, rotate the signing key, then take additional
steps as appropriate (e.g. rotate the master key, force-re-issue
client secrets).

## Multi-factor authentication

sui-id supports two second factors: TOTP (RFC 6238 authenticator
app) and WebAuthn (passkeys / hardware authenticators). Either, both,
or neither can be active per user. They are user-driven from the
Profile page, not operator-driven from the user list.

- **TOTP** is set up by scanning a QR code or pasting a Base32
  secret. On confirmation the user gets eight single-use recovery
  codes; they are shown once.
- **Passkeys** are registered via the browser's
  `navigator.credentials.create()` API. The user can have many — a
  YubiKey, a phone-based platform authenticator, and a backup
  device, for instance. Each is named by the user.

Once the first factor is enabled, password-only login is no longer
sufficient: the user must also pass the second factor. If both are
enabled, either suffices at the challenge page.

The audit log records every relevant event: `mfa.enable`,
`mfa.disable`, `mfa.recovery_codes_regenerate`,
`webauthn.credential.register`, `webauthn.credential.delete`,
`auth.mfa.success`, `auth.mfa.failure`,
`auth.login.password_ok_mfa_required`.

### Admin-initiated MFA reset

When a user has lost every second factor at once — authenticator
gone, recovery codes lost, every passkey broken — they cannot sign
in. Self-service recovery is impossible at that point.

From `/admin/users`, find the row, and use the **Reset MFA** button.
This deletes the user's TOTP enrolment (if any) and every registered
passkey. The user can then sign in with password alone and is free
to re-enrol whichever factors they want.

The reset is recorded as `mfa.admin_reset` in the audit log, with
the actor (the admin who performed it), the target (the user reset),
and a note describing exactly what was removed. **Review this
column periodically.** A reset is a privileged operation that
weakens the account; legitimate uses are recoverable user lockouts,
not routine maintenance.

The reset does **not** revoke the user's existing sessions or
refresh tokens. If you also want to force a re-login, follow up
with the Disable / Enable cycle from the same page — that revokes
sessions.

## WebAuthn / passkey requirements

WebAuthn imposes two requirements on the deployment:

- The browser must reach sui-id over **HTTPS**, except for
  `localhost` (which the spec excludes from the HTTPS rule for
  development convenience).
- The `server.issuer` URL's host part is the WebAuthn relying-party
  id. It must not change after users have registered passkeys; if
  it does, every passkey breaks. Treat the issuer URL as
  immutable from the moment the first user registers a passkey.

If you are deploying behind a reverse proxy, make sure the proxy
preserves the `Host` header sui-id sees as the issuer host. The
sample Caddy and nginx configs in
[`deployment.md`](deployment.md) get this right.

## Per-client scope policy

Every client registered on or after v0.6.0 declares an
`allowed_scopes` policy. Requests at `/oauth2/authorize` for scopes
outside the policy are rejected with `invalid_scope`. The default
for new clients is `openid profile`. Empty means "permit any" for
backwards compatibility with clients registered before the feature
existed.

The policy is editable from `/admin/clients/{id}/edit`. Tightening
takes effect immediately; loosening too. There is no token-issuance
window for a previously-issued code to slip through with a now-
forbidden scope.

## Auditing dependencies for known vulnerabilities

sui-id pins its dependency tree via `Cargo.lock`. New advisories
against pinned versions of those dependencies happen all the time,
and the only way to find out about them quickly is to scan the lock
file against the [RustSec advisory database](https://rustsec.org/).

The tool for this is [`cargo-audit`](https://crates.io/crates/cargo-audit):

```bash
cargo install --locked cargo-audit
cd /path/to/sui-id-source
cargo audit
```

The output flags two categories:

- **Vulnerabilities** — a published advisory whose `patched` versions
  do not include the version locked in `Cargo.lock`. These need
  attention. The fix is usually `cargo update -p <crate>` to pick up
  the patched version, then rebuild and redeploy. If the patched
  version requires a major-version bump that breaks downstream
  callers, consult the CHANGELOG before upgrading.
- **Warnings** — informational advisories: most commonly a
  dependency that has been **unmaintained**. Not directly
  exploitable; prompt to consider migrating off the crate over time.
  These do not need to block a deployment.

The upstream project runs the same scan in CI on every push and on a
weekly schedule, so contributors see results immediately. Operators
who build their own binary should run `cargo audit` as part of their
pre-deploy checklist — at minimum, before each upgrade.

If a freshly-disclosed CVE affects a dependency you're already
running and you cannot upgrade right away, the next-best step is to
isolate the deployment (firewall, IP allow-list) until the patched
build ships.

## Upgrading

For now, sui-id is pre-release: take a backup, replace the binary, and
restart. If a release ever requires a destructive migration, the
`CHANGELOG.md` entry will say so explicitly.

## Further reading

- [`docs/deployment.md`](deployment.md) is a chronological install
  walkthrough — useful when setting sui-id up for the first time on
  a new host.
- [`docs/threat-model.md`](threat-model.md) describes what sui-id defends
  against, what it does not, and what assumptions an operator must
  uphold for the design to work.
- [`docs/integrators.md`](integrators.md) is the corresponding guide for
  developers integrating an application against a sui-id instance.
