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

A working backup contains two files:

- The SQLite database (whatever you set as `db_path`).
- The master key file (whatever you set as `key_file`).

To take a consistent backup while sui-id is running, use SQLite's `.backup`
command on the database, then copy the key file separately:

```bash
sqlite3 /var/lib/sui-id/sui-id.sqlite ".backup '/tmp/backup.sqlite'"
cp /var/lib/sui-id/sui-id.key /tmp/backup.key
chmod 0600 /tmp/backup.sqlite /tmp/backup.key
```

Restore by stopping sui-id, putting both files back at their configured
paths, and starting sui-id again.

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

## Upgrading

For now, sui-id is pre-release: take a backup, replace the binary, and
restart. If a release ever requires a destructive migration, the
`CHANGELOG.md` entry will say so explicitly.
