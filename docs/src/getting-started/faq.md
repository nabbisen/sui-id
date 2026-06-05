# FAQ

## Is sui-id production-ready?

sui-id is under active development and has not reached v1.0. The HTTP API
(OIDC endpoints) is stable. The admin UI and internal APIs may change
between minor versions. Run it in non-critical environments first and
review the CHANGELOG before upgrading.

## How do I back up sui-id?

Copy two files:

```bash
cp /var/lib/sui-id/sui-id.sqlite /backup/sui-id.sqlite
cp /etc/sui-id/sui-id.key        /backup/sui-id.key
```

Store them separately. The SQLite file alone is useless without the key;
the key alone is useless without the database. Together they are a full
point-in-time backup. For frequent backups, use SQLite's
[online backup API](https://www.sqlite.org/backup.html) or `sqlite3 .backup`.

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

The master key (at `key_file` or `SUI_ID_MASTER_KEY` env var) seals:

- All refresh token values stored in the database.
- WebAuthn credential private-key bytes.
- TOTP secrets.
- SMTP password.
- Ed25519 signing-key private bytes.

If the master key is lost and no backup exists, none of the above can be
recovered. The SQLite file must be treated as unrecoverable.

## Can I use sui-id with PostgreSQL?

Not yet. SQLite is the only supported backend. RFC 009 in the roadmap
sketches alternative SQL backend support.

## Can I run multiple instances?

Not in the current release. sui-id uses SQLite in WAL mode and assumes a
single writer. If you need horizontal scaling, wait for RFC 009.

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
2. If another admin account exists, that admin can reset the password.
3. If neither applies, you must restore from a backup taken before the
   password was lost.
