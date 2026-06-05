# Operator's guide

> **Scope.** sui-id is a single-realm, first-party IdP. One flat
> namespace of users, one of clients, one global admin role. For
> multi-tenant requirements see RFC 025. For the project's design
> philosophy see the Scope section in [README.md](../README.md).


This is a *reference* for the operational surface of sui-id —
configuration fields, the master key, the audit log, GC, and routine
tasks.

If you are setting sui-id up for the first time, read
[deployment.md](deployment.md) first; that guide walks the
chronological "what do I run, in what order" path from a fresh
server to a working production install.

If you are looking for how to point an application at sui-id, see
[integrators.md](integrators.md).

## User–client relationship and the single-realm model

sui-id is a **single-realm IdP**: all users live in one namespace and all
clients share that namespace. There is no per-client user allowlist. When a
user authenticates with any client, they authenticate as themselves in the
one shared realm.

### What `allowed_scopes` controls

`allowed_scopes` is a space-separated list of OAuth scopes a client is
permitted to *request*. It does **not** restrict which users can authenticate
with a given client (all users can authenticate with any client by design).

| Scope | Claims returned | Typical use |
|---|---|---|
| `openid` | `sub`, `iss`, `aud`, `exp`, `iat` | Required for OIDC |
| `profile` | `name`, `preferred_username`, `locale` | User display name |
| `email` | `email`, `email_verified` | Email address |
| `offline_access` | (enables refresh tokens) | Long-lived access |

**New clients** default to `openid profile email`. Clients upgraded from
before v0.29.12 with an empty `allowed_scopes` retain "permit any" legacy
behaviour — review and restrict those clients.

### Common error: "scope X is not permitted for this client"

Go to **Admin → Clients**, select the client, and add the missing scope to
the *Allowed scopes* field. The error description includes the client name
and its current allowed list to make this easy to find.

For multi-tenant isolation (separate user namespaces per customer) see
RFC 025, which is the planned expansion path.

---

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

### Rotating the master key

`sui-id admin rotate-key` re-seals every encrypted column under a new
32-byte XChaCha20-Poly1305 master key, archives the old key file beside
the new one, and exits. Use it when:

- the existing key may have been exposed (laptop stolen, file leaked,
  contractor offboarded, etc);
- a periodic rotation policy says it's time;
- you've successfully restored from a backup whose key file you no
  longer trust.

**The CLI runs offline.** Stop the server before invoking it. Hot
rotation isn't supported — see the v0.26.0 CHANGELOG entry for the
rationale.

A typical run looks like:

```sh
# 1. Stop the running server.
systemctl stop sui-id

# 2. Take a fresh backup. Rotation is atomic, but having a backup
#    that pairs the OLD DB with the OLD key is the simplest fallback
#    if anything in the workflow goes wrong.
sui-id backup --to backup-pre-rotation.tar --encrypt

# 3. Run the rotation. By default the CLI mints a fresh key.
sui-id admin rotate-key

# 4. Confirm at the prompt: type `yes`. Skip the prompt with --yes
#    in scripts. The CLI prints a per-table count of re-sealed rows
#    on success.

# 5. Restart the server. It picks up the new key from the configured
#    key_file path automatically.
systemctl start sui-id
```

If you'd rather provide the new key yourself (e.g. derived from an
HSM, a key-management server, or a reproducible derivation):

```sh
# Prepare a base64-encoded 32-byte key in a file:
sui-id admin rotate-key --new-key /secure/new-master.key
```

After rotation:

- The previous key file has been renamed to
  `<original>.bak.<timestamp>` in the same directory. It is **not**
  auto-deleted — keep it as long as you might need to read a backup
  taken before the rotation, then remove it.
- An audit row `admin.master_key.rotated` is appended with a count
  of re-sealed rows.
- The new key file is written with `0600` permissions on Unix.

**Sealed columns covered:** signing keys, refresh tokens, TOTP
secrets, TOTP recovery codes, WebAuthn passkeys, and the SMTP
configuration password. The `password_reset_tokens` table holds
SHA-256 hashes (not encrypted) and is unaffected. Encrypted backup
tarballs are not re-keyed by this command — they have their own
passphrase and are the operator's to refresh as needed.

If the re-seal step fails for any reason (disk full, schema drift,
etc), the SQLite transaction rolls back, the old key file is **not**
yet renamed, and you can retry with the same arguments. There is no
half-rotated state to recover from.

## Dev mode for local testing

If you're building a relying party (RP) and just want a working
OIDC provider for local development, sui-id has a `--dev` flag
that skips the setup wizard entirely:

```sh
sui-id --dev
```

This:

- opens an in-memory SQLite database under a freshly-generated
  master key (data evaporates on shutdown);
- seeds an admin user, two test users, and one OIDC test
  client;
- prints all credentials to stderr in plaintext;
- listens on `127.0.0.1:8801`.

Point your RP at
`http://127.0.0.1:8801/.well-known/openid-configuration`. The
seed summary tells you the `client_id` (UUID) and
`client_secret` to use.

**Dev mode is not a production starting point.** It relaxes
operational knobs — `cookie_secure` is off, HIBP is off,
account lockout is disabled, the database is ephemeral. Every
*cryptographic* invariant (PKCE S256-only, AAD-bound column
encryption, Argon2id, `redirect_uri` exact match,
12-character password minimum) holds the same as in
production; the relaxations are about convenience, not about
weakening the OIDC implementation.

### Customising the seed

Three sources, in priority order (highest first):

1. **TOML file** via `--dev-seed PATH`. Full schema in
   `examples/dev-seed.toml`. Sections that aren't in the file
   fall back to defaults.
2. **CLI flag overrides**: `--dev-admin-password STR`,
   `--dev-client-secret STR`.
3. **Hardcoded defaults**: `admin / admin-admin-admin`,
   `alice / alice-alice-alice`, `bob / bob-bob-bob-bob`, plus
   one confidential test client with redirect URIs at
   `:3000`, `:5173`, `:8000` on localhost.

Example: SPA developer who wants a public (PKCE-only) client
on port 5173:

```sh
cat > /tmp/spa-seed.toml <<'TOML'
[admin]
username = "admin"
password = "admin-admin-admin"

[[client]]
name = "My SPA"
redirect_uris = ["http://localhost:5173/callback"]
public = true
allowed_scopes = "openid profile email"
TOML

sui-id --dev --dev-seed /tmp/spa-seed.toml
```

### Persisting dev state

`--dev-db PATH` pins the database to a file. The file is
truncated on each restart (a fresh master key is generated, so
re-using the old SQLite file would just produce ciphertext
nobody can decrypt). If you want persistence across restarts,
you want a regular sui-id installation, not dev mode.

### Binding outside loopback

The default bind is `127.0.0.1:8801`. You can change it with
`--dev-bind`:

```sh
sui-id --dev --dev-bind 0.0.0.0:8801
```

Any non-loopback bind requires explicit `yes` typed at the
prompt before sui-id will listen. This is a deliberate
guardrail: dev mode prints plaintext credentials at startup,
and accidentally binding to `0.0.0.0` from a Docker container
or shell-history search could expose them to a LAN. The
prompt is the operator's chance to confirm that's what they
meant.

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

2. Open the `/setup` URL in a browser. The setup wizard runs in
   five steps (RFC 012):

   - **Step 1 — welcome**: a brief description and a "begin" button.
   - **Step 2 — admin form**: paste the setup token, choose a
     username, optionally an email and display name, and pick a
     password (12 characters or more) entered twice.
   - **Step 3 — display language**: choose Japanese or English for
     the admin panel and login screens (default: Japanese). You can
     change this after setup in the admin settings.
   - **Step 4 — password security policy**: choose an HIBP mode
     (`off` / `warn` / `block`; default: `warn`). You can change
     this after setup in the admin settings.
   - **Step 5 — done**: confirmation that the admin account exists
     and the first signing key has been generated. The wizard logs
     you in automatically; the "管理画面へ進む" button lands you
     on `/admin`.

   The setup token only exists for the lifetime of this process —
   if you restart before finishing setup, you'll see a fresh token.

### Why there is no "encryption" step in the wizard

You may have seen IdP wizards that ask you to "set the master key
on first run". sui-id does not, on purpose. The master key is
**resolved before the HTTP server starts**:

- If `SUI_ID_MASTER_KEY` is set in the environment, that value
  is used.
- Otherwise sui-id reads `storage.key_file` from `sui-id.toml`.
  If the file does not exist, sui-id generates a new 32-byte key
  and writes it with permissions `0600` on first start; on
  subsequent starts the existing file is read.

By the time you reach `/setup` in a browser, the database is
already encrypted and the key is already loaded. There's nothing
left for a UI to do — surfacing one would invite operations that
the architecture doesn't support (mid-process key rotation
without restart) or actively undermine the security model
(advertising a key-manipulation interface from a process that
holds the key in memory). See [Threat model](./threat-model.md)
for the reasoning.

If you need to rotate the master key, that's a planned future
operation, performed offline against the database file with the
`sui-id admin` CLI.

## Email features

Since v0.22.0, sui-id can send a small set of transactional
emails:

- a **password-reset link** when a user submits the
  `/forgot-password` form;
- a **password-change notification** when any user (including
  the admin) changes their password — both via the self-service
  `/me/security/password` page and via a successful
  `/reset-password` flow.

Email is **opt-in**. Until it's configured, the four endpoints
that depend on it (`/forgot-password`, `/reset-password`,
`/admin/settings/email/test`, and the inbound email-related
admin pages) behave as if the feature doesn't exist (404 or no
notification mail). Existing functionality is unaffected.

### Configuring SMTP

Configuration lives in the database, not in `sui-id.toml`. Sign
in as an admin and visit **Settings → メール**
(`/admin/settings/email`). The form asks for:

| Field          | Notes                                                                                  |
|----------------|----------------------------------------------------------------------------------------|
| `host`         | SMTP relay hostname.                                                                   |
| `port`         | 465 for implicit-TLS, 587 for STARTTLS. The two are the only modes we support.         |
| `tls_mode`     | `Implicit` (TLS from the start, port 465) or `STARTTLS` (plain greeting then upgrade). |
| `username`     | Optional; leave empty when the relay does not require authentication.                  |
| `password`     | Optional; sealed with the master key before storing. Empty value keeps the existing.   |
| `from_address` | The envelope and visible `From:` address.                                              |
| `from_name`    | Optional display name (e.g. "Acme Corp Identity").                                     |
| `base_url`     | Public origin sui-id is reachable at. Used to build the reset-link URL.                |

`base_url` is **separate from the OIDC issuer URL** because the
two are sometimes different (the issuer can be a back-channel
URL while users browse from a different origin). Always use an
`https://` URL in production.

Why DB-stored, not TOML? We picked the database so:

- Config changes apply without a restart.
- A `Test Connection` button can run a real EHLO/STARTTLS/AUTH
  dance against the relay and surface the result inline.
- Credentials live alongside the rest of the encrypted columns
  (sealed with the master key).
- Setting changes feed the audit chain (`auth.smtp_config.changed`).

The `Test Connection` button is your friend: SMTP delivery
problems are notoriously hard to debug post-hoc, and getting an
immediate `550 5.7.1 relay denied` or `auth failed` in the UI
saves a lot of time vs poring over logs.

### Operational model

Sends are **inline**. When a user submits `/forgot-password`,
the handler awaits the SMTP exchange, logs the outcome, and
returns the same neutral 200 response regardless of success or
failure. Reasons:

- Volume is tiny (one mail per forgot-password / password-change
  event).
- A persistent outbox + retry worker would be more code to
  maintain than the savings justify at this scale.
- The user-enumeration neutralisation works equally well with
  inline as with queued sends, since the outer response is
  always the same.

If you need at-least-once delivery semantics — e.g. you're
deploying to a flaky network where SMTP attempts routinely
transient-fail — that's the future "persistent email outbox"
work in the ROADMAP. We don't believe it's needed at v1.0.

### Token model

Reset tokens are 32 bytes from `OsRng`, base64url-encoded for
the URL. Only the SHA-256 hash hits the database; the plaintext
exists in the user's email and on their machine, never on the
server after issue.

- Tokens expire 30 minutes after issue.
- They're single-use: redemption sets `consumed_at` and replays
  collapse to `InvalidCredentials` (same response shape as a
  bogus token, so a probe can't tell them apart).
- A user can have at most 3 outstanding (unconsumed,
  unexpired) tokens at a time. Above the ceiling we silently
  stop issuing new ones — the response remains 200 so the
  endpoint still doesn't reveal account state.

### Disabling the feature

Set `enabled = 0` (or remove the row) in the
`/admin/settings/email` page. The four feature-gated endpoints
return 404 immediately; in-flight sessions are unaffected.

## Backup and restore

A working backup contains three things: a SQLite snapshot of the
database, a copy of the master key, and a small `MANIFEST.json`
that records what version produced it. sui-id bundles all three
into a single tarball:

```bash
sui-id backup --to /var/backups/sui-id-$(date +%F).tar --config /etc/sui-id/sui-id.toml
```

The output file is created with mode `0600`. The SQLite snapshot
is produced via `VACUUM INTO`, which is safe to run while sui-id
is serving traffic — no need to stop the daemon for backups.

The manifest is human-readable JSON and contains:

- `format_version` — backup-file format version (currently 1).
- `sui_id_version` — the binary that produced the backup.
- `schema_version` — the database schema version at backup time.
- `created_at`, `hostname`, `issuer` — provenance.

`sui-id restore` reads this manifest before doing anything destructive.
A backup whose `format_version` or `schema_version` is **newer** than
the current binary supports is refused outright — the operator must
upgrade the binary first.

### Encrypted backups for off-host transport

The plain tarball above contains the master key inline. That's fine
for backups that never leave the host's trust boundary (a local
disk, a same-VPC backup volume). For backups that will travel —
cloud object storage, off-site media, email — wrap the tarball in
sui-id's encrypted envelope:

```bash
sui-id backup --to /tmp/backup.tar.enc --encrypt \
              --config /etc/sui-id/sui-id.toml
# Encryption passphrase: ******
# Encryption passphrase (again): ******
```

Or non-interactively for cron / scripts:

```bash
SUI_ID_BACKUP_PASSPHRASE='hunter2-correct-horse' \
  sui-id backup --to /tmp/backup.tar.enc --encrypt \
                --config /etc/sui-id/sui-id.toml
```

The envelope is XChaCha20-Poly1305 keyed by an Argon2id derivation
(64 MiB / 3 iterations / 1 thread) of the passphrase. The salt and
nonce are generated fresh per backup and stored in the file.

**Store the passphrase separately from the file.** A backup tarball
plus its passphrase, kept together, has the same security profile
as a plain backup. The whole point is that the two travel through
different channels.

### Verifying a backup

Before committing to a real restore — especially when migrating to
a new host — check the file is what you think it is:

```bash
sui-id verify-backup --from /tmp/backup.tar.enc --decrypt
# Format version: 1
# sui-id version: 0.12.0
# Schema version: 5
# Created at:     2026-04-28T10:31:42Z
# Hostname:       old-host.example.com
# Issuer:         https://idp.example.com
# Encrypted:      true
# Tar size:       183808 bytes
# Database size:  180224 bytes
# Master key:     present
#
# ✓ SQLite integrity check passed
# ✓ Decrypted with provided passphrase
```

`verify-backup` is read-only — it never touches the destination
storage paths and never produces any output file. Use it whenever
a backup has come in from outside; whenever you want to confirm
versions line up before an upgrade-and-migrate; or as a daily smoke
test from cron against the latest backup.

### Restoring

To restore, point `sui-id restore` at the tarball:

```bash
# plain backup
sui-id restore --from /var/backups/sui-id-2026-04-25.tar \
               --config /etc/sui-id/sui-id.toml

# encrypted backup
sui-id restore --from /tmp/backup.tar.enc --decrypt \
               --config /etc/sui-id/sui-id.toml
```

By default `restore` refuses to overwrite an existing database or
key file at the destination paths. Pass `--force` if you really mean
it (typically only when recovering onto a fresh host).

Backups produced by sui-id v0.12.x and earlier (no `MANIFEST.json`)
are still accepted by v0.13+ restore — the manifest checks are
skipped in that case. Upgrade-time recommendation: take a fresh
backup with the new binary, which will write the manifest.

### Migrating to a new host

The recommended migration sequence is:

1. **On the old host**: `sui-id backup --to /tmp/migration.tar.enc --encrypt`
2. **Transfer the file** to the new host through whatever channel is
   most convenient. Send the passphrase through a different channel.
3. **On the new host**: install the same (or newer) sui-id version,
   prepare `/etc/sui-id/sui-id.toml`, then run
   `sui-id verify-backup --from /tmp/migration.tar.enc --decrypt`
   to confirm the file is intact and the schema version matches what
   the new binary expects.
4. **Restore** with `sui-id restore --from /tmp/migration.tar.enc --decrypt`.
5. **Start the new sui-id**. Migrations run automatically on first
   boot if the new binary is a newer version than the old one.
6. **Cut DNS / load balancer** to the new host. The old refresh
   tokens, sessions, and signing keys all carry over, so end users
   don't notice the move.
7. **Stop the old host** and securely destroy the migration archive
   on both sides.

> **Be careful where the tarball ends up.** A plain backup contains
> the master key. An encrypted backup contains nothing useful
> without the passphrase, but anyone who has both has the master key.
> Treat backups the same way you treat the key file itself.

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

## Security headers

sui-id sets a fixed set of security-relevant response headers on
every response. These are not configurable — they are part of the
program's defended posture, not policy you should tune.

| Header | Value | What it does |
|---|---|---|
| `Content-Security-Policy` | `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'` | Restricts what the admin UI can load; `frame-ancestors 'none'` denies framing of every endpoint, including `/oauth2/authorize`. |
| `X-Frame-Options` | `DENY` | Belt-and-braces alongside `frame-ancestors` for older browsers. |
| `X-Content-Type-Options` | `nosniff` | Stops MIME-sniffing. |
| `Referrer-Policy` | `strict-origin-when-cross-origin` | Outbound navigation leaks only the origin, never the path/query. |
| `Permissions-Policy` | camera/geolocation/microphone/payment/USB all denied | Disables browser feature APIs sui-id has no use for. |
| `Strict-Transport-Security` | `max-age=63072000; includeSubDomains` | **Only when `cookie_secure = true`**. Two years, no `preload` (operators opt in to preload separately if they want it). |

If your reverse proxy adds these headers too, sui-id's middleware
will *not* overwrite anything the inner handler already set, but a
proxy that overwrites the response from sui-id wins. That's fine
provided the proxy's policy is at least as strict.

### CSP and the WebAuthn JS bundle

The CSP allows `script-src 'self'` so the bundled `/static/webauthn.js`
loads. There are no inline scripts and no remote scripts. If you
deploy a custom admin theme that injects external scripts, you'll
need to relax the policy in your proxy — sui-id itself does not
support this.

## CORS

sui-id emits CORS headers on the routes that legitimately want
cross-origin browser access:

| Route | Policy |
|---|---|
| `/.well-known/openid-configuration` | `Access-Control-Allow-Origin: *` |
| `/.well-known/jwks.json` | `Access-Control-Allow-Origin: *` |
| `/oauth2/userinfo` | `Access-Control-Allow-Origin: *` |
| `/oauth2/token` | Origin allowlist computed at request time from registered `redirect_uris` |
| `/oauth2/introspect`, `/oauth2/revoke` | none — server-to-server |
| `/oauth2/authorize`, `/oauth2/logout` | none — top-level navigation |
| `/admin/*` | none — same-origin |

The token-endpoint allowlist matters for SPA relying parties: a
browser-resident OIDC client running on `https://app.example.com`
exchanges its authorization code via `fetch` from that origin. The
fetch is allowed if and only if the origin matches the scheme +
host + port of some registered `redirect_uri` on some active
client. Two consequences:

- A new SPA RP that hasn't yet been registered will get a CORS
  failure on the token exchange. **Register the redirect_uri for
  the SPA before you deploy it.**
- A registered desktop/mobile client whose redirect_uri uses a
  custom scheme (e.g. `app://callback`) won't appear in the
  allowlist; that's correct, because non-browser clients don't
  send `Origin` and don't need CORS to begin with.

If your proxy already handles CORS for these routes, sui-id's
middleware adds no headers when there is no `Origin` request
header. Your proxy's policy then wins.

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
| `auth.login.locked` | A failed sign-in that just triggered or extended an account lockout. **Alert on bursts of this.** |
| `auth.refresh.theft_detected` | A revoked refresh token was replayed at the token endpoint. The whole rotation family was revoked. **Alert on this.** |
| `auth.sessions.bulk_revoke_self` | A user used "Sign out everywhere else" on `/me/security`. Note records how many sessions were swept. |
| `auth.password.changed_self` | A user changed their own password via `/me/security/password`. Note records how many sessions and refresh tokens were swept (zero if the user unchecked the box). |
| `mfa.admin_reset` | An administrator forcibly removed every MFA factor for a user. **Alert on this.** |
| `admin.user.unlock` | An admin cleared an account lockout via `sui-id admin unlock-user`. |
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

## Audit log integrity

Every audit row carries a `prev_hash` and a `hash`, where
`hash = SHA-256(prev_hash || canonical_bytes(row))` and
`prev_hash` is the previous row's `hash`. To rewrite or delete
row N you'd have to recompute every later row's hash — something
an attacker with raw SQL access can do, but never without leaving
a detectable mismatch at the boundary, because the legitimate
code path's hash computation is the only thing that produces
matching chains.

This is local tamper-evidence: it catches DB-only attackers (SQL
injection, misconfigured backups, file-system access) but not
attackers who control the binary itself. For most self-hosted IdP
deployments that's the relevant attack model; full external
timestamping is a follow-up topic.

### What sui-id does on startup

On every startup sui-id walks the most recent 5,000 audit rows
newest-first and recomputes each row's hash. The result lands in
the structured log:

```json
{"level": "INFO", "fields": {"event": "audit-log hash chain verified", "checked": 5000, "legacy_unhashed": 0}}
```

If a row's stored hash disagrees with recomputation, the log
entry is at `ERROR` level with a `broken_at_seq` field:

```json
{"level": "ERROR", "fields": {"event": "audit-log hash-chain verification FAILED — tampering or DB corruption suspected", "broken_at_seq": 423}}
```

sui-id **does not refuse to start** on detection. Refusing would
turn a one-row corruption into a denial-of-service for the entire
IdP. The error is loud enough that an operator's monitoring
catches it; the action is for the operator to investigate.

### Setting up an alert

A SIEM or log shipper rule on the literal phrase "hash-chain
verification FAILED" (or the structured field
`broken_at_seq`) is the right place to fire. From the JSON log:

```bash
jq -c 'select(.fields.broken_at_seq) | {at: .timestamp, broken_at_seq: .fields.broken_at_seq}' \
   < sui-id.log
```

In a Loki / Grafana setup:

```logql
{job="sui-id"} | json | fields_broken_at_seq != ""
```

### Rows from before v0.17.0

Rows that pre-date this feature have empty `hash` and `prev_hash`
columns. The verifier counts them as `legacy_unhashed` and does
**not** flag them as tampering — they were never hashed in the
first place. Once you upgrade and the next audit event lands,
the chain begins from there.

If you want a fully hashed audit log going forward and you don't
need the historical entries, you can re-bootstrap by truncating
`audit_log` and restarting; new rows will form a clean chain
from row 1. Be aware this destroys investigation evidence — only
do it on a fresh deployment.

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

## Account lockout

After enough consecutive failed password attempts on an account,
sui-id locks the account temporarily — refusing further sign-in
attempts even with the correct password. The defence is per-user
and orthogonal to the per-IP rate limit on `/admin/login`: between
the two, an attacker can neither hammer one account from one host
nor spread an attack across many accounts from one host.

### The backoff curve

The first two failures cost an account nothing — every operator
fat-fingers a password sometimes. From the third onward the lock
window grows quickly:

| Consecutive failures | Lock window |
| -------------------- | ----------- |
| 1, 2                 | none        |
| 3                    | 30 seconds  |
| 4                    | 1 minute    |
| 5                    | 5 minutes   |
| 6                    | 30 minutes  |
| 7                    | 2 hours     |
| 8                    | 6 hours     |
| 9                    | 12 hours    |
| 10+                  | 24 hours    |

Each value is then capped by the `[security] max_lockout` setting.

A successful password verification at any point clears the counter
and lifts any active lock.

### Configuring the cap

```toml
[security]
max_lockout = "24h"
```

Allowed values, picked from a fixed set so a typo can't put the
cap somewhere wild: `"15min"`, `"1h"`, `"4h"`, `"12h"`, `"24h"`,
`"48h"`. Default is `"24h"`. The 48-hour ceiling is deliberate —
locking past two days is more likely to lock out a real user
(post-vacation, post-weekend) than to deter an attacker, who has
long given up by then.

A lower cap (`"15min"` or `"1h"`) is reasonable for installs where
you'd rather risk a determined attacker getting more grinding
attempts per day than risk a real user being locked out overnight.
A higher cap (`"24h"` or `"48h"`) is the right choice for tighter
deployments where a real user reaching out to an admin is an
acceptable cost.

### Recovering a locked-out user

If a real user has been locked out — perhaps a typo storm, perhaps
they're back from vacation and don't remember the password — clear
the lock from the host:

```bash
sui-id admin unlock-user --username alice --config /etc/sui-id/sui-id.toml
```

This resets `failed_login_count` to 0 and removes any active lock.
The account is immediately ready for sign-in. The action is
recorded in the audit log as `admin.user.unlock`.

### What this looks like in the audit log

The two relevant events:

| Event                  | Meaning                                           |
| ---------------------- | ------------------------------------------------- |
| `auth.login.failure`   | Wrong password (or unknown user, or disabled). The audit row's `note` says which. |
| `auth.login.locked`    | A failed attempt that *just* triggered or extended a lock. Includes the consecutive-failure count and the new window length in the note. |
| `admin.user.unlock`    | An admin cleared the lock via the CLI.            |

A SIEM rule on `auth.login.locked` is a useful signal that
something is hammering an account. From the JSON log:

```bash
jq -c 'select(.fields.event == "auth.login.locked") |
       {at: .timestamp, target: .fields.target, note: .fields.note}' \
   < sui-id.log
```

### Timing-equivalence behaviour

To avoid leaking which accounts are locked, all of these branches
produce the same HTTP response (`401 Unauthorized`) and very close
to the same wall-clock time:

- The user does not exist.
- The user is disabled or deleted.
- The user is locked.
- The user exists, isn't locked, but the password is wrong.

The Argon2id hash verification (~80 ms) runs on every branch — on
the "user doesn't exist" and "user is locked" branches it's run
against a fixed dummy PHC string that never matches anything,
purely to keep timing equal to the real-verify path. A remote
observer cannot tell the cases apart.

## Self-service security (`/me/security`)

Every signed-in user — admin or not — has access to a self-service
security overview at `/me/security`. The page is intentionally
narrow in scope: it doesn't expose anything an admin couldn't
also see in `/admin/audit` or `/admin/users`, but it gives a user
the tools to notice unusual activity on their own account
without an operator having to be involved.

The page shows three sections:

- **Two-factor authentication summary.** Whether TOTP is on, how
  many passkeys are registered. Has a "Manage authenticators"
  button that goes to `/admin/profile`, where the actual
  enrollment / removal lives. (`/admin/profile` doesn't require
  admin privilege; a non-admin user reaches the same page from
  here.)
- **Where you're signed in.** Every active session for this user,
  newest first, with the session that issued the request marked
  as "current". Every other row gets a "Revoke" button. Below
  the table, "Sign out everywhere else" sweeps every session
  except the current one.
- **Recent activity.** Up to 30 most recent audit events that
  either name this user as the actor (e.g. `auth.login.success`)
  or as the target (e.g. `mfa.admin_reset`, `auth.login.locked`,
  `auth.refresh.theft_detected`). The user is told plainly: if
  you see something here you didn't do, change your password
  and sign out other sessions immediately.

### What ownership means here

The handler enforces ownership server-side: a user can only see
and revoke their *own* sessions. Attempting to revoke another
user's session — by guessing or scraping a session id — produces
the same redirect as revoking an unknown id, so there's no
oracle for distinguishing "session exists" from "session exists
but belongs to someone else". The e2e suite includes a regression
test (`me_security_cannot_revoke_someone_elses_session`) that
pins this behaviour.

### Self-service password change

Since v0.19.0, `/me/security/password` lets a signed-in user
change their own password without an admin reset. The form asks
for the current password, the new password, and a confirmation,
and offers a checkbox (default ON) to "sign out my other browsers
and apps" once the change is committed.

The flow is:

1. CSRF check.
2. Rate limit on the same `Login` bucket the sign-in form uses,
   keyed by client IP. Even though the caller already has a
   valid session, this prevents someone with a stolen cookie
   from grinding the `current_password` field at unbounded rate.
3. Confirmation match check.
4. Verify the supplied current password against the stored hash.
   On mismatch the change is refused with `InvalidCredentials` —
   the same error the regular login path raises.
5. Apply the new-password policy (length, etc.).
6. Hash and persist.
7. If the "sign out other sessions" box is checked, revoke every
   other session for this user and revoke every active refresh
   token. The current session stays alive — otherwise the user
   would be bounced back to the login page the moment they save
   the form, which feels broken.
8. Append a `auth.password.changed_self` audit event recording
   how many sessions and refresh tokens were swept.

The wrong-current-password branch deliberately **does not**
trigger the account lockout that the public sign-in form does.
Brute-forcing the current-password field requires a valid cookie
to begin with; raising lockouts here would let a user lock
themselves out by typo. The IP-keyed rate limit still applies.

The `must_change` flag (set by an admin reset) is cleared on
self-change — the user has demonstrated agency, so the prompt
to rotate goes away.

### Audit trail

The bulk-revoke action emits a `auth.sessions.bulk_revoke_self`
audit event recording how many sessions were swept. Single-row
revokes do not emit a dedicated event today; the `revoked_at`
column on the row is itself the durable record. If you need
finer-grained accounting, ship the `sessions` table to your SIEM
periodically alongside the audit log.

### What `/me/security` deliberately does **not** do

- HIBP breach check on password reuse.
- Long-form session metadata (browser fingerprint, IP, user
  agent). The session table does not record IP or User-Agent
  today; we'd add it deliberately rather than as a side effect
  of building this page.
- Cross-account view. Admins do *not* get extra rows here — they
  see only their own sessions, like everyone else. The `/admin/`
  pages are the place for cross-account work.

### Self-service password change (`/me/security/password`)

The "Change password" button on `/me/security` opens a form
asking for the current password, the new one (twice), and a
checkbox — checked by default — that says "sign out my other
browsers and apps after changing the password." On submit:

1. The CSRF token is verified.
2. The request is rate-limited against the same IP-keyed bucket
   the login form uses. A user already holding a valid session
   shouldn't be able to grind the current-password field at
   unbounded rate even with a stolen cookie.
3. The new password and the confirmation field must match.
4. The current password is verified against the stored Argon2id
   hash. A wrong current password is reported as
   `InvalidCredentials` — same error variant as a failed login,
   so client error mapping stays simple. **No account lockout
   is applied on this path**: the user is already authenticated
   by their session, and locking yourself out by mistyping a
   confirmation field would be unhelpful.
5. The new password is checked against the policy (minimum 12
   characters, maximum 256). The order — verify-current then
   policy-check-new — is deliberate: it stops the endpoint from
   becoming an oracle for "is X actually a password?" via
   differentiated error messages.
6. The credential row is upserted with the new hash, and the
   `must_change` flag is cleared if it was set.
7. If "sign out everywhere else" was checked, every session
   *except* the current one is revoked, and **every** active
   refresh token belonging to the user is revoked. The current
   session stays alive so the user isn't booted out of the form
   they just submitted.
8. An `auth.password.changed_self` audit event is appended,
   noting in its `note` field how many sessions and refresh
   tokens were swept.

### Things `/me/security/password` deliberately does **not** do

- Send a confirmation email. We don't have email integration
  today; that lands in a later release. When it does, this
  flow gains a "we emailed your previous address that this
  happened" notification.
- Require re-MFA. The session is already MFA-elevated if the
  user's account requires MFA at sign-in time; re-prompting on
  every sensitive action is a separate (good!) feature we'll
  add as part of a step-up-auth pass.
- Block re-using the same password. The check would be cheap to
  add (verify-against-old-hash before upsert), but reuse policy
  more broadly belongs in a dedicated v0.20+ pass alongside
  HIBP and password-history.

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

### v0.29.10 — pre-flight required for boolean CHECK constraints

Migration 0022 adds `CHECK (col IN (0, 1))` constraints to five tables
(`users`, `credentials`, `clients`, `signing_keys`, `user_totp`) and the
`clients.confidential ↔ secret_hash` consistency CHECK.

Unlike migration 0021 (which was safe with no pre-flight requirement),
**migration 0022 will abort if any existing row has a boolean value outside
{0, 1}**. This is deliberate: rather than silently converting a bad value
(which could re-enable a disabled user), the migration fails so the operator
can review and correct the data manually.

Run the pre-flight script before upgrading:

```bash
sqlite3 /path/to/sui-id.db < docs/operators/preflight-0022.sql
```

All queries must return zero or empty results before upgrading.

**Safe evacuation approach.** Migration 0022 uses a new `FK_DISABLE_REQUIRED`
marker recognised by the migration runner. The runner sets
`PRAGMA foreign_keys = OFF` *before* starting the transaction, so the
`DROP TABLE` steps do not trigger `ON DELETE CASCADE` on child tables. After
`COMMIT`, the runner re-enables FK enforcement and runs
`PRAGMA foreign_key_check` to confirm integrity. This is the fix for the
v0.29.7 data-loss scenario.

**Additional improvements in migration 0022:**

- `users.user_uuid CHECK (length(user_uuid) = 36)` — prevents empty-string
  `user_uuid` at the DB layer. Migration 0022 backfills any remaining empty
  values automatically before the table rebuild.
- Non-partial `UNIQUE INDEX idx_users_user_uuid` — since all values are now
  guaranteed to be 36 chars, the `WHERE user_uuid <> ''` partial condition
  is no longer needed.
- `signing_keys.is_active CHECK (is_active IN (0, 1))` — completes the
  enforcement that the partial-unique index from migration 0021 began.
- `user_totp` rebuilt with STRICT mode preserved from migration 0003.

---

### v0.29.8 — bugfix for v0.29.7 data-loss on upgrade

**If you deployed v0.29.7, do not run its migration 0021 against a production
database.** Migration 0021 in that release deletes all credentials, sessions,
refresh tokens, and MFA registrations for existing users (see root-cause analysis
in CHANGELOG). Instead:

1. Roll back to v0.29.6 (restore from backup if you already applied 0021)
2. Upgrade directly to v0.29.8

v0.29.8 ships a corrected migration 0021. It is safe to apply to any database
at schema version 0020 (v0.29.6). No pre-flight checks are required beyond the
standard backup recommendation.

The migration adds:
- `idx_signing_keys_single_active` partial unique index (at most one active signing key)
- `consents` table redesign with proper FK constraints
- `idx_sessions_user_active_alive` index on sessions

Boolean CHECK constraints (`is_admin IN (0,1)`, etc.) and the `clients` confidential/
secret_hash consistency CHECK are **deferred** to a future release. They were removed
from this migration to fix the data-loss bug. The pre-flight SQL in
`docs/operators/preflight-0021.sql` remains useful for diagnostics but is no longer
a required step before upgrading.

---

### v0.29.7 — RETRACTED (data-loss bug)

> **Do not upgrade to v0.29.7.** See v0.29.8 for the fix.

Migration 0021 in v0.29.7 used `PRAGMA foreign_keys = OFF` inside a transaction.
SQLite treats this PRAGMA as a no-op in transaction context, so `DROP TABLE users`
triggered `ON DELETE CASCADE` on all child tables. The following tables were wiped:

- `credentials` (users could no longer log in)
- `sessions`
- `refresh_tokens`
- `user_totp` (MFA registrations lost)
- potentially `user_webauthn_credentials`, `password_reset_tokens`, and others

The pre-flight section below (for v0.29.7) is retained for reference but is no
longer relevant if you upgrade directly to v0.29.8.

---

### ~~v0.29.7~~ — pre-flight checks required for schema hardening (retracted)

Migration `0021` rebuilds five tables (`users`, `credentials`, `clients`,
`signing_keys`, `user_totp`) and adds CHECK constraints. If any existing
row violates a constraint (unusual boolean values, inconsistent
`confidential` / `secret_hash`, or multiple active signing keys), the
migration will **fail** and sui-id will refuse to start until the data is
repaired.

Run the pre-flight script before upgrading:

```bash
sqlite3 /path/to/sui-id.db < docs/operators/preflight-0021.sql
```

All queries must return empty or zero results. See the script at
`docs/operators/preflight-0021.sql` for repair instructions.

The `consents` table is dropped and rebuilt (no production data should
exist there yet; it has no consumers until RFC 008 lands).

The signing key rotation order is also reversed in this release: the
existing active key is retired *before* the new one is inserted (both
in one transaction). This is invisible to callers but means any manual
direct-SQL rotation done against a pre-0.29.7 instance may need to be
revised if you bypassed the admin UI.

---

### v0.29.6 — pre-flight check required if duplicate emails exist

Migration `0020` adds a `UNIQUE INDEX` on `email_normalized`
(the lower-cased, trimmed form of each user's email). If any two
users share the same normalised email address (e.g. one registered as
`alice@example.com` and another as `Alice@EXAMPLE.COM`), the migration
will **fail with a `UNIQUE constraint failed` error** and sui-id will
refuse to start.

Run the following SQL against your database **before** upgrading to
check for duplicates:

```sql
-- Returns one row per normalised-email collision.
-- An empty result means you are safe to upgrade.
SELECT lower(trim(email)) AS email_normalized,
       count(*)            AS n,
       group_concat(id)    AS user_ids
FROM   users
WHERE  email IS NOT NULL
  AND  email <> ''
GROUP  BY lower(trim(email))
HAVING count(*) > 1;
```

If the query returns rows, resolve the conflicts before upgrading. The
typical resolution is to delete or merge the duplicate accounts through
the admin panel while on the current version.

All other changes in 0.29.6 (migration `0019`: `auth_codes` FK
constraints and `refresh_tokens.token_hash`) are safe to apply without
any pre-flight check. The `auth_codes` table is rebuilt by the
migration; any rows outstanding at upgrade time are at most 60 seconds
old and can safely be discarded (users will be asked to log in again
for in-flight OIDC flows).

## Further reading

- [`docs/deployment.md`](deployment.md) is a chronological install
  walkthrough — useful when setting sui-id up for the first time on
  a new host.
- [`docs/threat-model.md`](threat-model.md) describes what sui-id defends
  against, what it does not, and what assumptions an operator must
  uphold for the design to work.
- [`docs/integrators.md`](integrators.md) is the corresponding guide for
  developers integrating an application against a sui-id instance.
