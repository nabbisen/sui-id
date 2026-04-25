# Operator's guide

This guide is for someone running sui-id on a server they control. If you
are looking for how to point an application at sui-id, see
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

## Upgrading

For now, sui-id is pre-release: take a backup, replace the binary, and
restart. If a release ever requires a destructive migration, the
`CHANGELOG.md` entry will say so explicitly.

## Further reading

- [`docs/threat-model.md`](threat-model.md) describes what sui-id defends
  against, what it does not, and what assumptions an operator must
  uphold for the design to work.
- [`docs/integrators.md`](integrators.md) is the corresponding guide for
  developers integrating an application against a sui-id instance.
