# Deployment guide

This guide walks an operator from a fresh Linux server to a hardened
production deployment of sui-id. It opinionates where it can — pick
the alternative if you have a reason to.

If you are looking for the *reference* for individual configuration
fields and operational tasks, that's [`operators.md`](operators.md).
This document is the chronological "what do I run, in what order"
companion.

The example uses Debian 12 / Ubuntu 24.04 syntax; it ports cleanly to
RHEL-family distributions with the obvious substitutions
(`apt` → `dnf`, `systemctl` shape unchanged, paths under `/etc` and
`/var/lib` identical).

## What you need before you start

- A Linux host with a public IPv4 or IPv6 address you control.
- A DNS name pointed at it. The rest of this guide assumes
  `idp.example.com`. Substitute your own throughout.
- Root or sudo access on the host.
- Inbound TCP/443 reachable from the internet (or your private
  network, if this is for an intranet).
- An email address for the certificate authority's expiry warnings.

## 1. System packages

```bash
apt update
apt install -y \
  ca-certificates \
  curl \
  caddy \
  cron \
  libssl3 \
  sqlite3
```

Notes:

- `caddy` gives you HTTPS with automatic certificate renewal in one
  step. If you already run nginx and prefer to keep it that way,
  skip this and follow the nginx variant in §5 below.
- `libssl3` is a runtime dependency of the WebAuthn path
  (webauthn-rs links openssl). The `-dev` package is needed only at
  build time, on a different machine.
- `sqlite3` is optional — for inspecting the database during
  troubleshooting. sui-id itself bundles its SQLite library.

## 2. A dedicated user

sui-id should not run as `root` and should not own anything outside
its own state directory.

```bash
useradd --system --create-home --home-dir /var/lib/sui-id \
        --shell /usr/sbin/nologin \
        --comment "sui-id OIDC provider" \
        sui-id
install -d -m 0750 -o sui-id -g sui-id /etc/sui-id
install -d -m 0750 -o sui-id -g sui-id /var/lib/sui-id
install -d -m 0750 -o sui-id -g sui-id /var/log/sui-id
install -d -m 0750 -o sui-id -g sui-id /var/backups/sui-id
```

The `0750` permissions matter: the master key file ends up in
`/var/lib/sui-id`, and a stolen master key plus a stolen
`sui-id.sqlite` reads every encrypted column. Don't make either
world-readable.

## 3. Install the binary

If you build from source on the deployment host:

```bash
apt install -y curl build-essential pkg-config libssl-dev
curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
. "$HOME/.cargo/env"
git clone https://github.com/nabbisen/sui-id.git
cd sui-id
cargo build --release
install -m 0755 target/release/sui-id /usr/local/bin/sui-id
```

If you build elsewhere and copy:

```bash
# On the build host
cargo build --release
ldd target/release/sui-id   # confirm it pulls libssl.so.3, glibc, etc.
scp target/release/sui-id deploy@idp.example.com:/tmp/sui-id

# On the deployment host
install -m 0755 -o root -g root /tmp/sui-id /usr/local/bin/sui-id
sui-id --version
```

The runtime needs `libssl.so.3` from the system. A statically-linked
build is not provided.

## 4. Configuration

```bash
sui-id --print-sample-config > /etc/sui-id/sui-id.toml
chown root:sui-id /etc/sui-id/sui-id.toml
chmod 0640 /etc/sui-id/sui-id.toml
```

Edit `/etc/sui-id/sui-id.toml` to match production. The crucial
fields are below; everything else can be left at the defaults until
you have a reason to change them.

```toml
[server]
listen_addr = "127.0.0.1:8801"
# This is the URL your relying parties will hit, ending up in `iss`
# in every issued token. WebAuthn derives `rp_id` from the host
# part. Once you put real users on the system, changing this
# invalidates every previously-issued token and every registered
# passkey — choose carefully and do not change it.
issuer = "https://idp.example.com"
cookie_secure = true
# Trust X-Forwarded-For only from the IP your reverse proxy uses
# to reach sui-id. 127.0.0.1/32 is the right answer for the same-
# host setup in this guide. Don't add ranges you don't trust;
# anything in this list can spoof a client IP and bypass rate
# limiting per address.
trusted_proxies = ["127.0.0.1/32", "::1/128"]

[storage]
db_path = "/var/lib/sui-id/sui-id.sqlite"
key_file = "/var/lib/sui-id/sui-id.key"

[tokens]
access_lifetime_secs = 900
id_token_lifetime_secs = 900
refresh_lifetime_secs = 1209600

[log]
format = "json"
filter = "info,sui_id=info,sui_id_core=info,sui_id_store=info"
```

Verify the file parses without starting the daemon:

```bash
sudo -u sui-id sui-id --config /etc/sui-id/sui-id.toml --version
```

## 5. HTTPS termination

sui-id listens on plain HTTP. A reverse proxy puts HTTPS in front of
it, takes care of certificates, and forwards to the local socket.

### Caddy (recommended)

`/etc/caddy/Caddyfile`:

```
idp.example.com {
    encode zstd gzip
    reverse_proxy 127.0.0.1:8801 {
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
        header_up X-Forwarded-Host {host}
    }
    # Be conservative: sui-id pages should never be framed.
    header {
        Strict-Transport-Security "max-age=31536000; includeSubDomains"
        X-Content-Type-Options "nosniff"
        Referrer-Policy "same-origin"
    }
}
```

```bash
systemctl reload caddy
```

Caddy's automatic TLS picks the cert up from Let's Encrypt; the only
prerequisites are that the DNS record for `idp.example.com` already
points at this host and that ports 80 + 443 are open in the
firewall.

### nginx (alternative)

If you would rather use nginx with `certbot`:

```bash
apt install -y nginx certbot python3-certbot-nginx
certbot --nginx -d idp.example.com -m you@example.com --agree-tos
```

Then edit `/etc/nginx/sites-available/idp.example.com.conf` to add
the reverse-proxy stanza. The minimal block:

```nginx
location / {
    proxy_pass         http://127.0.0.1:8801;
    proxy_set_header   Host              $host;
    proxy_set_header   X-Forwarded-For   $remote_addr;
    proxy_set_header   X-Forwarded-Proto $scheme;
    proxy_set_header   X-Forwarded-Host  $host;

    add_header         Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header         X-Content-Type-Options "nosniff" always;
    add_header         Referrer-Policy "same-origin" always;
}
```

```bash
systemctl reload nginx
```

## 6. systemd unit

`/etc/systemd/system/sui-id.service`:

```ini
[Unit]
Description=sui-id OpenID Connect provider
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=sui-id
Group=sui-id
ExecStart=/usr/local/bin/sui-id --config /etc/sui-id/sui-id.toml
Restart=on-failure
RestartSec=2s

# Read-only filesystem except the explicit state directory.
ProtectSystem=strict
ReadWritePaths=/var/lib/sui-id /var/log/sui-id /var/backups/sui-id
ProtectHome=yes
PrivateTmp=yes
NoNewPrivileges=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
RestrictRealtime=yes
SystemCallFilter=@system-service
SystemCallErrorNumber=EPERM
CapabilityBoundingSet=
AmbientCapabilities=

[Install]
WantedBy=multi-user.target
```

```bash
systemctl daemon-reload
systemctl enable --now sui-id
systemctl status sui-id
```

The hardening directives here are the standard `systemd-analyze
security` recommendations for a long-running unprivileged HTTP
daemon. None of them is required for sui-id to work, but together
they materially raise the cost of a successful exploit. Run
`systemd-analyze security sui-id` after enabling to see the
resulting score.

## 7. Bootstrap the admin

The first start logs a one-time setup token to stderr. Capture it
from the journal:

```bash
journalctl -u sui-id --since "1 minute ago" | grep -A2 "Setup token"
```

Open `https://idp.example.com/setup` in a browser, paste the token,
and create your administrator account. The token is good only until
the first successful setup; subsequent visits to `/setup` redirect
to login.

If you missed the token in the journal, restart the service — a new
token is issued on every startup until setup succeeds:

```bash
systemctl restart sui-id
journalctl -u sui-id --since "1 minute ago" | grep -A2 "Setup token"
```

## 8. Enable MFA on the admin account

You just created the only account that can administrate this
installation. Lock it down:

1. Sign in.
2. Go to `Profile`.
3. Either set up TOTP (with an authenticator app) **and** save the
   recovery codes, or register a passkey, or both.
4. Sign out and sign back in to confirm the second factor works.

If a single device holds your only second factor and the device is
lost, the recovery codes (or another passkey on a separate device)
are how you get back in. Plan the recovery story before you need it.

## 9. Backups

`sui-id backup` produces an atomic, hot snapshot of the database and
master key in a single tar file. Schedule it.

For backups that stay on the same trust boundary as the host (e.g.
a local backup volume on the same machine, or a same-VPC backup
host), the plain form is fine:

`/etc/cron.d/sui-id-backup` (local-only retention):

```
SHELL=/bin/sh
PATH=/usr/local/bin:/usr/bin:/bin

# Daily at 03:17. Keep 30 days.
17 3 * * * sui-id /usr/local/bin/sui-id backup \
    --config /etc/sui-id/sui-id.toml \
    --to /var/backups/sui-id/sui-id-$(date +\%Y-\%m-\%d).tar
27 3 * * * sui-id find /var/backups/sui-id -name 'sui-id-*.tar' -mtime +30 -delete
```

The plain backup tar file contains the master key. Treat it with the
same care as any other secret: restrict who can read it, encrypt the
filesystem under it, restrict the backup host as if it were the
sui-id host itself.

For backups that will leave the trust boundary — cloud object
storage, off-site media, anywhere you wouldn't put the unencrypted
key file — use `--encrypt`:

```
# Daily at 03:17. Encrypted with a passphrase from a sealed file.
17 3 * * * sui-id SUI_ID_BACKUP_PASSPHRASE="$(cat /etc/sui-id/backup.pass)" \
    /usr/local/bin/sui-id backup \
    --config /etc/sui-id/sui-id.toml \
    --to /var/backups/sui-id/sui-id-$(date +\%Y-\%m-\%d).tar.enc \
    --encrypt
```

The passphrase file (`/etc/sui-id/backup.pass`, mode `0600`, owned
by the `sui-id` user) holds a 32+ character random passphrase.
Generate it once with `head -c 32 /dev/urandom | base64`. Store a
copy in your password manager — losing the passphrase makes the
encrypted backups unrecoverable.

A trivial off-host shipper for the encrypted form:

```
37 3 * * * sui-id rsync -a --delete \
    /var/backups/sui-id/ backup-host:/srv/backups/sui-id/
```

Before relying on a backup, verify it. A daily smoke test:

```
47 3 * * * sui-id SUI_ID_BACKUP_PASSPHRASE="$(cat /etc/sui-id/backup.pass)" \
    /usr/local/bin/sui-id verify-backup \
    --from /var/backups/sui-id/sui-id-$(date +\%Y-\%m-\%d).tar.enc \
    --decrypt > /var/log/sui-id-backup-verify.log 2>&1
```

`verify-backup` reads the file, runs a SQLite integrity check on
the inner snapshot, and prints the manifest. It never writes
anything. If this command fails, alert.

To restore, copy a tar to the new host and:

```bash
systemctl stop sui-id

# Plain
sui-id restore --config /etc/sui-id/sui-id.toml \
              --from /tmp/sui-id-2026-04-15.tar

# Encrypted
SUI_ID_BACKUP_PASSPHRASE="$(cat /etc/sui-id/backup.pass)" \
  sui-id restore --config /etc/sui-id/sui-id.toml \
                 --from /tmp/sui-id-2026-04-15.tar.enc --decrypt

systemctl start sui-id
```

## 10. Health checks and monitoring

sui-id exposes `/healthz` over HTTP. It returns `200 OK` when the
process can read the database and `503` otherwise. Wire it into
whatever you use:

- A systemd watchdog: extend the unit with `WatchdogSec=` and have
  sui-id restart on stalls. (sui-id does not currently emit
  watchdog notifications, so this only catches process death, not
  hangs — same as `Restart=on-failure`.)
- A Caddy / nginx upstream health probe.
- A pull-based monitor (`curl -fsS https://idp.example.com/healthz`)
  triggered by Prometheus blackbox exporter, monit, etc.

The audit log lives in the database (`audit_log` table). Useful queries:

```sql
-- Recent failed sign-ins.
SELECT at, action, target, note
FROM audit_log
WHERE action = 'auth.login.failure'
ORDER BY seq DESC LIMIT 50;

-- MFA resets — you want this clean. Every row here is an admin
-- forcibly removing a user's second factor.
SELECT at, actor, target, note
FROM audit_log
WHERE action = 'mfa.admin_reset'
ORDER BY seq DESC;
```

If you ship logs centrally, point your collector at the journal and
filter by `_SYSTEMD_UNIT=sui-id.service`. The `[log]` section's
`format = "json"` produces structured records the collector can
parse directly.

## 11. Upgrades

Test the upgrade on a copy first if you can. The general flow:

```bash
# Pre-flight: scan the build's dependency tree for known
# vulnerabilities. This catches advisories published since the
# upstream release tagged its lockfile.
cd /path/to/sui-id-source-of-the-new-build
cargo install --locked cargo-audit
cargo audit

# Make a fresh backup before doing anything.
sui-id backup --config /etc/sui-id/sui-id.toml \
              --to /var/backups/sui-id/pre-upgrade-$(date +%Y-%m-%d).tar

# Replace the binary.
install -m 0755 /tmp/new-sui-id /usr/local/bin/sui-id

# Restart. Migrations run automatically on startup.
systemctl restart sui-id
journalctl -u sui-id --since "1 minute ago" | grep -i migrat
systemctl status sui-id
```

The migration log lines tell you which schema versions ran. If
something is wrong, restore the pre-upgrade backup and downgrade:

```bash
systemctl stop sui-id
install -m 0755 /usr/local/bin/sui-id.bak /usr/local/bin/sui-id
sui-id restore --config /etc/sui-id/sui-id.toml \
              --from /var/backups/sui-id/pre-upgrade-2026-04-15.tar
systemctl start sui-id
```

There is no automatic schema downgrade. If a release introduces a
schema change you cannot live with, the only path back is the
pre-upgrade backup. Read the CHANGELOG before upgrading minor
versions for that reason.

## 12. Things to verify after deployment

A short checklist to run through with a browser and `curl` once
sui-id is live:

```bash
# Discovery document is reachable and lists the right issuer.
curl -fsS https://idp.example.com/.well-known/openid-configuration | jq .issuer

# JWKS is present and has at least one signing key.
curl -fsS https://idp.example.com/.well-known/jwks.json | jq '.keys | length'

# /healthz is 200.
curl -fsSI https://idp.example.com/healthz | head -1

# /setup is gone (it should redirect to /admin/login once setup ran).
curl -fsSI https://idp.example.com/setup | head -1
```

In the browser:

- `https://idp.example.com/admin` should redirect to login.
- After signing in, the dashboard renders, the user count is 1, and
  the client count is 0.
- Register your first relying party from `Clients`. The secret is
  shown exactly once on the resulting page; store it where the RP
  needs it before navigating away.

## What's next

- [`operators.md`](operators.md) — full reference of configuration
  fields, the master key handling, GC behaviour, the audit log
  schema, and routine tasks.
- [`integrators.md`](integrators.md) — how a relying party hooks
  itself into sui-id once you've registered it as a client.
- [`threat-model.md`](threat-model.md) — what sui-id defends against
  (and what it does not). Worth reading before you deploy somewhere
  visible to the internet.
