# Configuration reference

sui-id is configured via a single [TOML](https://toml.io/) file — by
convention `sui-id.toml` but configurable with `--config <path>`.

Run the following to print a valid minimal configuration and exit:

```sh
sui-id --print-sample-config
```

The output expresses every default so it can be edited in place.

> **Two settings live outside the TOML file** because they are secrets that
> must not appear in plaintext config:
>
> | Variable | Purpose |
> |---|---|
> | `SUI_ID_MASTER_KEY` | Base64-encoded 32-byte master encryption key. Overrides `[storage].key_file`. On first start, if neither is present, a key is generated and written to `key_file`. |
> | `SUI_ID_SETUP_TOKEN` | Override the one-time setup token printed to stderr on first start. Optional; useful for scripted provisioning. |
> | `SUI_ID_BACKUP_PASSPHRASE` | Passphrase for `sui-id backup` and `sui-id restore`. When set, the CLI reads it instead of prompting interactively. |

---

## `[server]`

Controls the listening address and the public OIDC identity.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `listen_addr` | string | **yes** | — | `host:port` for the HTTP listener. sui-id does not terminate TLS — deploy behind a TLS-terminating reverse proxy in production. Example: `"127.0.0.1:8801"`. |
| `issuer` | string | **yes** | — | External URL used as the OIDC `issuer` claim and JWKS base URL. Must be an absolute `http://` or `https://` URL and must match the URL relying parties discover at `/.well-known/openid-configuration`. Example: `"https://id.example.com"`. |
| `cookie_secure` | bool | no | `false` | Set the `Secure` attribute on session cookies. Must be `true` in production behind HTTPS. When `false` the dashboard shows a "cookie insecure" warning. |
| `trusted_proxies` | array of strings | no | `[]` | CIDR ranges of reverse proxies whose `X-Forwarded-For` header is trusted for rate-limiting. Empty = always use the socket peer IP. An over-broad value lets clients spoof their IP and bypass rate limits. Example: `["10.0.0.0/8", "172.16.0.0/12"]`. |

**Startup validation.** `issuer` must be an absolute `http://` or `https://` URL.
Each entry in `trusted_proxies` must be a valid CIDR block.
Startup fails with a clear error if either constraint is violated.

---

## `[storage]`

File paths for the database and the master encryption key.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `db_path` | path | **yes** | — | Path to the SQLite database file. Created on first start if it does not exist. Relative paths are resolved from the working directory. |
| `key_file` | path | **yes** | — | Path to a file holding the base64-encoded 32-byte master key. On first start, if the file does not exist and `SUI_ID_MASTER_KEY` is unset, a key is generated and written here with permissions `0600`. Back this file up separately from the database — without it all encrypted columns are permanently unreadable. |

> **Backup.** A complete backup is two files: `db_path` + `key_file`.
> The built-in `sui-id backup` command creates an encrypted archive of both.
> See the [operators reference](../guides/operators.md).

---

## `[tokens]`

Lifetime settings for tokens issued at the OIDC token endpoint. All values
are in seconds.

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `access_lifetime_secs` | integer | no | `900` (15 min) | Lifetime of access tokens. Short by design — access tokens are bearer tokens with no server-side revocation; a shorter window limits blast radius. Must be > 0. |
| `id_token_lifetime_secs` | integer | no | `900` (15 min) | Lifetime of ID tokens included in the token response. Should be close to `access_lifetime_secs`. Must be > 0. |
| `refresh_lifetime_secs` | integer | no | `1209600` (14 days) | Lifetime of refresh tokens. Rotated on every use: the old token is immediately revoked when a new one is issued. Must exceed `access_lifetime_secs`. |

**Startup validation.** `access_lifetime_secs` must be > 0.
`refresh_lifetime_secs` must be strictly greater than `access_lifetime_secs`.

---

## `[log]`

Logging configuration via the [`tracing`](https://docs.rs/tracing) crate.

| Field | Type | Required | Default | Valid values | Description |
|---|---|---|---|---|---|
| `format` | string | no | `"fmt"` | `"fmt"`, `"json"` | `"fmt"` — human-readable lines. `"json"` — one JSON object per line, for ELK, Loki, Datadog, etc. |
| `filter` | string | no | `"info,sui_id=info,sui_id_core=info,sui_id_store=info"` | [`tracing-subscriber` env-filter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) expression | Verbosity per module. For debug output: `"debug,h2=warn,hyper=warn,reqwest=warn"`. |
| `access_log` | bool | no | `false` | — | When `true`, emit one `INFO` line per request: method, path, status, and request ID. Also enabled by `--dev`. |
| `file` | path or absent | no | absent (stderr only) | — | When set, write logs to daily-rotated files in this directory **in addition to** stderr. Files are named `sui-id.YYYY-MM-DD.log`. The directory must exist and be writable. |

---

## `[security]`

Security-policy knobs. Future settings (rate-limit thresholds,
password-complexity parameters) will be added here.

| Field | Type | Required | Default | Valid values | Description |
|---|---|---|---|---|---|
| `max_lockout` | string | no | `"24h"` | `"15min"`, `"1h"`, `"4h"`, `"12h"`, `"24h"`, `"48h"` | Cap on the automatic account-lockout duration after repeated failed sign-ins. Uses a progressive-backoff curve; `max_lockout` is the ceiling. Admins can unlock manually at any time with `sui-id admin unlock-user`. The restricted value set prevents locking real users out over weekends or holidays. The maximum is `"48h"`. |

> **Session policy** (idle timeout, concurrent session cap) is configured
> in the admin UI under **Settings → Authentication**, not in this file.
> The TOML governs sign-in policy; the admin UI governs session policy.

---

## Environment variables

| Variable | Required | Description |
|---|---|---|
| `SUI_ID_MASTER_KEY` | No (but recommended for containers) | Base64-encoded 32-byte master key. Overrides `[storage].key_file` when set. Preferred for container deployments where secrets are injected via environment. |
| `SUI_ID_SETUP_TOKEN` | No | Override the one-time setup token printed to stderr on first start. Set it before starting sui-id to use a known value in scripted provisioning. |
| `SUI_ID_BACKUP_PASSPHRASE` | No | Passphrase for `sui-id backup` and `sui-id restore`. When set, skips the interactive passphrase prompt. |

---

## Runtime flags

CLI arguments — apply only to the current process invocation, not persisted.

| Flag | Description |
|---|---|
| `--config <path>` | Path to the configuration file. Default: `./sui-id.toml`. |
| `--dev` | Development mode. Seeds a test admin (`admin` / `changeme`) and an OIDC test client. Sets `cookie_secure = false`, disables HIBP, disables lockout, enables access logging. **Never use in production.** |
| `--print-sample-config` | Print a minimal valid configuration to stdout and exit. Pipe to `> sui-id.toml` to bootstrap. |
| `--help` | Print full usage and subcommand reference. |

---

## Subcommands

| Subcommand | Description |
|---|---|
| `sui-id setup --config <c> --admin-username <name>` | **Headless initialization (RFC 077).** Creates the first administrator and bootstraps a signing key without the GUI wizard. Accepts `--admin-email` and `--admin-display-name`. Password comes from `SUI_ID_ADMIN_PASSWORD` env var if set; otherwise a random 24-char password is generated and printed once to stdout with a change advisory. Fails if the instance is already initialized. |
| `sui-id backup --config <c> --dest <path>` | Create an encrypted archive of the database and key file. |
| `sui-id restore --config <c> --src <path>` | Restore from an archive. Prompts for confirmation before overwriting. |
| `sui-id verify-backup --src <path>` | Verify archive integrity and print a compatibility report without writing files. |
| `sui-id admin unlock-user --config <c> <username>` | Clear an automatically-locked account immediately. |
| `sui-id admin rotate-key --config <c>` | Create and activate a new Ed25519 signing key; retire the old one. |
| `sui-id admin rotate-metrics-token --config <c>` | Generate a new Prometheus metrics bearer token and print it once. |
| `sui-id admin issue-registration-token --config <c> [--max-uses N] [--note TEXT]` | Generate an RFC 7591 initial-access token for dynamic client registration. Printed once — save it immediately. |

Run `sui-id --help` for the full flag listings.

---

## Minimal configuration

```toml
[server]
listen_addr = "127.0.0.1:8801"
issuer      = "http://127.0.0.1:8801"

[storage]
db_path  = "./sui-id.sqlite"
key_file = "./sui-id.key"
```

All other sections use defaults.

---

## Production-ready annotated configuration

```toml
[server]
# Listen on loopback; the reverse proxy handles public TLS.
listen_addr     = "127.0.0.1:8801"
issuer          = "https://id.example.com"
cookie_secure   = true                     # Required behind HTTPS.
trusted_proxies = ["10.0.0.0/8"]          # Adjust to your proxy subnet.

[storage]
db_path  = "/var/lib/sui-id/sui-id.sqlite"
key_file = "/etc/sui-id/sui-id.key"        # Back up separately from the DB.
# Alternatively: export SUI_ID_MASTER_KEY=<base64> and omit key_file.

[tokens]
access_lifetime_secs  = 900    # 15 min — default; limits stolen-token blast radius.
refresh_lifetime_secs = 86400  # 24 h — tighter than the 14-day default.
# id_token_lifetime_secs defaults to 900.

[log]
format     = "json"            # Structured log aggregation.
filter     = "info,h2=warn,hyper=warn,reqwest=warn"
access_log = true              # One INFO line per request.
file       = "/var/log/sui-id" # Daily-rotated files in addition to journald.

[security]
max_lockout = "24h"            # Default; suits most deployments.
```

---


---

## `[[user_source]]` (RFC 005)

Zero or more `[[user_source]]` blocks configure external user sources for the
authentication cascade. Currently only LDAP is supported.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `slug` | string | **yes** | — | Internal identifier used in logs and audit events. |
| `kind` | string | **yes** | — | Source type. Only `"ldap"` is currently supported. |
| `url` | string | **yes** | — | LDAP server URL. Must start with `ldaps://` (TLS required). |
| `bind_dn` | string | **yes** | — | DN of the service account used to search. Empty bind DN is rejected (no anonymous bind). |
| `bind_password_env` | string | **yes** | — | Name of the environment variable holding the service-account password. Never put the password inline. |
| `base_dn` | string | **yes** | — | Base DN for user searches. |
| `user_filter` | string | no | `"(uid={username})"` | LDAP search filter. `{username}` is substituted with the login username (RFC 4515-escaped). |
| `email_attribute` | string | no | `"mail"` | LDAP attribute name for the user's email address. |
| `connect_timeout_secs` | integer | no | `5` | TCP connect timeout. |
| `search_timeout_secs` | integer | no | `10` | LDAP search/bind timeout. |

```toml
[[user_source]]
slug               = "corporate-ldap"
kind               = "ldap"
url                = "ldaps://ldap.corp.example.com:636"
bind_dn            = "cn=svc-sui-id,ou=service-accounts,dc=corp,dc=example,dc=com"
bind_password_env  = "LDAP_BIND_PASSWORD"
base_dn            = "ou=people,dc=corp,dc=example,dc=com"
user_filter        = "(uid={username})"
email_attribute    = "mail"
```

---

## `[[federation_provider]]` (RFC 004)

Zero or more `[[federation_provider]]` blocks configure upstream OIDC identity
providers for federated sign-in (the "Sign in with X" flow).

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `slug` | string | **yes** | — | URL-safe identifier used in routes: `/auth/federated/{slug}/start`. Must be lowercase alphanumeric with optional hyphens. |
| `display_name` | string | **yes** | — | Human-readable label for the "Sign in with X" button. |
| `issuer` | string | **yes** | — | Upstream OIDC issuer URL. Used to fetch `/.well-known/openid-configuration`. |
| `client_id` | string | **yes** | — | OAuth 2.0 client ID registered at the upstream. |
| `client_secret_env` | string | no | `""` | Name of the environment variable holding the client secret. Omit or set empty for public clients (no secret). Never put the secret inline. |
| `scopes` | string | no | `"openid email"` | Space-separated scopes to request from the upstream. |
| `provision_mode` | string | no | `"link_only"` | `"link_only"` — user must authenticate locally to link; `"provision_on_first_login"` — a password-less local account is created on first sign-in (requires `email_verified = true` from the upstream). |
| `enabled` | bool | no | `false` | Whether the "Sign in with X" button is shown and callbacks are accepted. |

> **Security note.** New providers start with `enabled = false` and with
> `is_disabled = true` on the first locally-provisioned user. An administrator
> must explicitly enable the provider after verifying the configuration.

```toml
[[federation_provider]]
slug               = "google"
display_name       = "Google"
issuer             = "https://accounts.google.com"
client_id          = "1234567890-abc.apps.googleusercontent.com"
client_secret_env  = "GOOGLE_CLIENT_SECRET"
scopes             = "openid email profile"
provision_mode     = "provision_on_first_login"
enabled            = true

[[federation_provider]]
slug               = "entra"
display_name       = "Microsoft"
issuer             = "https://login.microsoftonline.com/{tenant}/v2.0"
client_id          = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
client_secret_env  = "ENTRA_CLIENT_SECRET"
provision_mode     = "link_only"
enabled            = false
```

---

## `[metrics]` (RFC 006)

Optional Prometheus metrics endpoint.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `enabled` | bool | no | `false` | Expose the metrics endpoint at `GET /metrics`. |
| `listen_addr` | string | no | same as `[server].listen_addr` | Override the address for the metrics endpoint. Useful to restrict metrics to an internal network interface. |

The metrics endpoint is protected by a bearer token stored (hashed) in the
database. Issue or rotate the token with:

```sh
sui-id admin rotate-metrics-token --config sui-id.toml
```

Present the token as `Authorization: Bearer <token>` or as a session cookie
(for browser access via the admin panel).

```toml
[metrics]
enabled     = true
listen_addr = "127.0.0.1:9091"   # Only reachable from localhost / monitoring agent.
```

## See also

- [Deployment guide](../guides/deployment.md) — reverse-proxy configuration,
  systemd unit, `trusted_proxies` guidance.
- [Operators reference](../guides/operators.md) — backup procedures, session
  policy, HIBP setup, key rotation, role management.
- [Upgrade guide](../guides/upgrade.md) — version-specific migration notes.
