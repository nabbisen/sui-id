# Quick start

## Install

```bash
cargo install sui-id
```

Or build from source:

```bash
git clone https://github.com/nabbisen/sui-id
cd sui-id
cargo build --release
```

## Configure

Generate a starter config:

```bash
sui-id --print-sample-config > sui-id.toml
```

Edit at minimum:

```toml
[server]
listen_addr = "127.0.0.1:8801"
issuer      = "https://id.example.com"   # must be reachable by clients

[storage]
db_path  = "/var/lib/sui-id/sui-id.sqlite"
key_file = "/etc/sui-id/sui-id.key"
```

## First run

```bash
sui-id --config sui-id.toml
```

On first run sui-id:

1. Creates a 32-byte master key at `key_file` with permissions `0600`.
   **Back this file up immediately.** Without it, encrypted columns cannot be read.
2. Prints a one-time **setup token** to stderr.
3. Waits at the setup wizard (`/setup`).

**Option A — headless CLI (no browser needed):**

Stop the server (Ctrl-C), then run:

```sh
sui-id setup --config ./sui-id.toml --admin-username admin
```

A random password is printed once to stdout. Then restart the server.

**Option B — browser wizard:**

Open a browser to `http://127.0.0.1:8801/setup` and complete the three-step wizard:

- **Language** — choose the admin UI language.
- **Security** — configure HIBP breach-password checking.
- **Admin account** — enter the setup token and create the first administrator.

After setup (either option), the admin panel is available at `/admin`.

## Try it locally (dev mode)

For rapid local testing, dev mode starts without a setup wizard and seeds
demo data:

```bash
sui-id --dev
```

Dev mode:
- Binds to `127.0.0.1:8801` by default.
- Creates a pre-configured admin, user, and OIDC client from hardcoded defaults
  (or a `dev-seed.toml` if present).
- Disables `cookie_secure`, HIBP checking, and account lockout.
- Shows a **yellow banner** on every admin page as a reminder.

**Never use `--dev` in production.**

## OIDC endpoints

| Endpoint | URL |
|---|---|
| Discovery | `/.well-known/openid-configuration` |
| JWKS | `/.well-known/jwks.json` |
| Authorization | `/oauth2/authorize` |
| Token | `/oauth2/token` |
| Userinfo | `/oauth2/userinfo` |
| Revocation | `/oauth2/revoke` |
| Introspection | `/oauth2/introspect` |
| End session | `/oauth2/logout` |

For full integration details see the [OIDC API reference](../reference/oidc-api.md).
