# FAQ

## Is sui-id production-ready?

sui-id is under active development and has not reached v1.0. No interface
carries a compatibility guarantee yet — not the HTTP API, the admin UI, the
CLI or the configuration file. The OIDC endpoints implement the standards
listed in the [OIDC API reference](../reference/oidc-api.md). Run it in
non-critical environments first and review the CHANGELOG before upgrading.

## How do I back up sui-id?

Use `sui-id backup`:

```bash
sui-id backup --config /etc/sui-id/sui-id.toml --to /var/backups/sui-id/sui-id.tar
```

It writes one tar file holding a consistent snapshot of the database, a copy
of the master key and a manifest, and it can run while sui-id is serving.
Do not back up with `cp`: the database runs in WAL mode, so a copy of the
`.sqlite` file can miss committed data.

The tar file contains the master key. For a backup that leaves the host's
trust boundary, add `--encrypt`. See the
[deployment guide](../guides/deployment.md#9-backups).

## A user lost their MFA device. What do I do?

1. Go to **Admin panel → Users**.
2. Click the user's username to open the user detail page.
3. Click **Reset MFA**. This removes TOTP and all passkeys.
4. The user can re-enrol at their next sign-in.

The reset is logged in the audit log.

## How do I rotate the signing key?

Go to **Admin panel → Signing keys → Rotate signing key**.

Rotation issues a new key and retires the current one. Retired keys remain
published in JWKS so tokens issued under them continue to verify until they
expire. After all old tokens have expired you can safely delete the retired key.

## What does the master key protect?

The master key (at `key_file` or `SUI_ID_MASTER_KEY` env var) seals the
sensitive database columns. What it covers, and what it does not, is stated in
the [threat model](https://github.com/nabbisen/sui-id/blob/main/docs/threat-model.md).

If the master key is lost and no backup exists, the sealed columns cannot be
recovered.

## Can I use sui-id with PostgreSQL?

Not yet. SQLite is the only supported backend. RFC 009 in the roadmap
sketches alternative SQL backend support.

## Can I run multiple instances?

No. sui-id uses SQLite in WAL mode and assumes a single writer. SQLite is the
only supported backend, and work on alternative backends is frozen under the
current [roadmap](https://github.com/nabbisen/sui-id/blob/main/ROADMAP.md) programme.

## How do I enable HTTPS?

sui-id does not terminate TLS itself. Put a reverse proxy (nginx, Caddy,
Traefik) in front and set `cookie_secure = true` in `[server]`.

Example nginx location block:

```nginx
location / {
    proxy_pass         http://127.0.0.1:8801;
    proxy_set_header   X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header   Host $host;
}
```

Also set `trusted_proxies = ["127.0.0.1/32"]` in `[server]` so
sui-id trusts the `X-Forwarded-For` header from the proxy.

## What if I forget the admin password?

There is no out-of-band recovery path by design — that would be a
security vulnerability. Options:

1. If SMTP is configured, use the forgot-password flow.
2. Otherwise, restore from a backup taken before the password was lost.

No admin can reset another user's password today: neither the admin panel
nor the CLI has that action.
