# Configuration reference

sui-id is configured via a single [TOML](https://toml.io/) file — by
convention `sui-id.toml` but configurable with `--config <path>`.

Run the following to print a valid minimal configuration and exit:

```sh
sui-id config sample
```

The output expresses every default so it can be edited in place.

> **Two settings live outside the TOML file** because they are secrets that
> must not appear in plaintext config:
>
> | Variable | Purpose |
> |---|---|
> | `SUI_ID_MASTER_KEY` | Base64-encoded 32-byte master encryption key. Overrides `[storage].key_file`. On first start, if neither is present, a key is generated and written to `key_file`. |
> | `SUI_ID_SETUP_TOKEN` | Override the one-time setup token printed to stderr on first start. Optional; useful for scripted provisioning. |
> | `SUI_ID_BACKUP_PASSPHRASE` | Passphrase for `sui-id backup create` and `sui-id backup restore`. When set, the CLI reads it instead of prompting interactively. |

---

## `[server]`

Controls the listening address and the public OIDC identity.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `listen_addr` | string | **yes** | — | `host:port` for the HTTP listener. sui-id does not terminate TLS — deploy behind a TLS-terminating reverse proxy in production. Example: `"127.0.0.1:8801"`. |
| `issuer` | string | **yes** | — | External URL used as the OIDC `issuer` claim and JWKS base URL. Must be an absolute `http://` or `https://` URL and must match the URL relying parties discover at `/.well-known/openid-configuration`. Example: `"https://id.example.com"`. |
| `cookie_secure` | bool | no | `false` | Set the `Secure` attribute on session cookies. Must be `true` in production behind HTTPS. When `false` the dashboard shows a "cookie insecure" warning. |
| `trusted_proxies` | array of strings | no | `[]` | CIDR ranges of reverse proxies whose `X-Forwarded-For` header is trusted for rate-limiting. Empty = always use the socket peer IP. An over-broad value lets clients spoof their IP and bypass rate limits. Example: `["10.0.0.0/8", "172.16.0.0/12"]`. |
| `metrics_enabled` | bool | no | `false` | Expose the Prometheus endpoint at `GET /metrics` (RFC 006). When `false` the route is not registered and returns 404, so the endpoint's existence is not revealed. See [Prometheus metrics](#prometheus-metrics-rfc-006). |
| `metrics_listen_addr` | string | no | `""` | Separate `host:port` for `/metrics`. Empty mounts it on the main listener. A private address such as `"127.0.0.1:9091"` is strongly recommended in production. |

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
> The built-in `sui-id backup create` command creates an encrypted archive of
> both. See the [operators reference](../guides/operators.md).

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
| `access_log` | bool | no | `false` | — | When `true`, emit one `INFO` line per request: method, path, status, and request ID. Also enabled by `sui-id dev`. |
| `file` | path or absent | no | absent (stderr only) | — | When set, write logs to daily-rotated files in this directory **in addition to** stderr. Files are named `sui-id.YYYY-MM-DD.log`. The directory must exist and be writable. |

---

## `[security]`

Security-policy knobs. Future settings (rate-limit thresholds,
password-complexity parameters) will be added here.

| Field | Type | Required | Default | Valid values | Description |
|---|---|---|---|---|---|
| `max_lockout` | string | no | `"24h"` | `"15min"`, `"1h"`, `"4h"`, `"12h"`, `"24h"`, `"48h"` | Cap on the automatic account-lockout duration after repeated failed sign-ins. Uses a progressive-backoff curve; `max_lockout` is the ceiling. Admins can unlock manually at any time with `sui-id admin user unlock`. The restricted value set prevents locking real users out over weekends or holidays. The maximum is `"48h"`. |

> **Session policy** (idle timeout, concurrent session cap) is configured
> in the admin UI under **Settings → Authentication**, not in this file.
> The TOML governs sign-in policy; the admin UI governs session policy.

---

## Environment variables

| Variable | Required | Description |
|---|---|---|
| `SUI_ID_MASTER_KEY` | No (but recommended for containers) | Base64-encoded 32-byte master key. Overrides `[storage].key_file` when set. Preferred for container deployments where secrets are injected via environment. |
| `SUI_ID_SETUP_TOKEN` | No | Override the one-time setup token printed to stderr on first start. Set it before starting sui-id to use a known value in scripted provisioning. |
| `SUI_ID_BACKUP_PASSPHRASE` | No | Passphrase for `sui-id backup create` and `sui-id backup restore`. When set, skips the interactive passphrase prompt. |

---

## The command line

`--config <path>` (default `./sui-id.toml`) and `--no-backup` (RFC 142 stage
5: skip the automatic pre-migration snapshot) are global — valid before or
after any command. Everything else — every command, its own options, the
environment variables it reads, and a worked example — is the
[CLI reference](cli.md), generated from the binary's own command registry so
it cannot drift from what the binary actually accepts. Run `sui-id help` for
the one-screen map, or `sui-id help COMMAND` for one command's full detail.

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

## `[[user_sources]]` (RFC 005)

Zero or more `[[user_sources]]` blocks configure external user sources for the
authentication cascade. Currently only LDAP is supported.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `slug` | string | **yes** | — | Internal identifier used in logs and audit events. |
| `kind` | string | **yes** | — | Source type. Only `"ldap"` is currently supported. |
| `url` | string | **yes** | — | LDAP server URL. Must start with `ldaps://` (TLS required). |
| `bind_dn` | string | **yes** | — | DN of the service account used to search. Empty bind DN is rejected (no anonymous bind). |
| `bind_password_env` | string | **yes** | — | Name of the environment variable holding the service-account password. Never put the password inline. |
| `user_search_base` | string | **yes** | — | Base DN for user searches. |
| `user_search_filter` | string | **yes** | — | LDAP search filter with a single `{username}` placeholder, substituted with the login username (RFC 4515-escaped). Example: `"(uid={username})"`. |
| `stable_id_attribute` | string | **yes** | — | Attribute holding the user's stable unique identity, such as `entryUUID` or `objectGUID`. |
| `display_name_attribute` | string | no | none | Attribute to use as the display name, such as `cn` or `displayName`. |
| `email_attribute` | string | no | none | Attribute to use as the email address, such as `mail` or `userPrincipalName`. |
| `connect_timeout_secs` | integer | no | `5` | TCP connect timeout. |
| `search_timeout_secs` | integer | no | `10` | LDAP search/bind timeout. |

```toml
[[user_sources]]
slug                = "corporate-ldap"
kind                = "ldap"
url                 = "ldaps://ldap.corp.example.com:636"
bind_dn             = "cn=svc-sui-id,ou=service-accounts,dc=corp,dc=example,dc=com"
bind_password_env   = "LDAP_BIND_PASSWORD"
user_search_base    = "ou=people,dc=corp,dc=example,dc=com"
user_search_filter  = "(uid={username})"
stable_id_attribute = "entryUUID"
email_attribute     = "mail"
```

---

## `[[federation_providers]]` (RFC 004)

Zero or more `[[federation_providers]]` blocks configure upstream OIDC identity
providers for federated sign-in (the "Sign in with X" flow).

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `slug` | string | **yes** | — | URL-safe identifier used in routes: `/auth/federated/{slug}/start`. Must be 1–64 characters of lowercase letters, digits and hyphens, and unique across blocks; startup refuses anything else. |
| `display_name` | string | **yes** | — | Human-readable label for the "Sign in with X" button. |
| `issuer` | string | **yes** | — | Upstream OIDC issuer URL. Used to fetch `/.well-known/openid-configuration`. The host may not be an IP address, `localhost`, or a name ending `.localhost`, `.local`, `.internal`, or `.home.arpa`, nor one of those zones itself (`local`, `internal`, `home.arpa`) — an identity provider reachable only on a private network is not supported in this release, and there is no configuration escape hatch. Renaming it does not help: sui-id also refuses a name that resolves to a private, loopback or link-local address when it connects. Startup refuses any other value. |
| `client_id` | string | **yes** | — | OAuth 2.0 client ID registered at the upstream. |
| `client_secret_env` | string | no | `""` | Name of the environment variable holding the client secret. Omit or set empty for public clients (no secret). Never put the secret inline. |
| `scopes` | string | no | `"openid email"` | Space-separated scopes to request from the upstream. |
| `provision_mode` | string | no | `"link_only"` | `"link_only"` — an upstream identity with no existing link is refused, shown the login page's generic "account link required" message; **this release has no self-service linking**, so an administrator must create the `federation_link` row directly before that user's first sign-in can succeed (a later design may restore self-service, with fresh local authentication, local MFA, CSRF, a single-use durable intent, and explicit approval — no date is set). `"provision_on_first_login"` — a password-less local account is created, **enabled**, on first sign-in, for anyone the upstream vouches for with a verified email (`email_verified = true`). **With a public provider such as Google, that is open sign-up: anyone with an account there gets an account here**, and sui-id cannot limit it to one email domain. Choose it when that is what you want, or when the upstream's accounts are all yours (for example a single-tenant Microsoft Entra issuer) — see the [operator guide](../guides/operators.md#federated-sign-in-upstream-oidc). |
| `enabled` | bool | no | `false` | **No longer has any effect** (RFC 096-B2). Enabling a provider is an administrator's own action, at **Federation providers** in the admin UI (`/admin/federation-providers`); the key is still accepted, and a block that sets it true logs one warning per boot saying so. |
| `allowed_origins` | array of strings | no | `[]` | Canonical HTTPS origins (scheme, host, and port) this provider's discovery document may name for `authorization_endpoint`, `token_endpoint`, `userinfo_endpoint` and `jwks_uri`. Empty means the issuer's own origin alone. If you set this, list every origin you want allowed, and **include the issuer's own origin**. Write each origin exactly in canonical form: lowercase host, no path or trailing `/`, and no `:443` (`https://accounts.google.com`, not `https://Accounts.google.com/`). At most 8 entries, no duplicates. Startup refuses any other value. |
| `id_token_algs` | array of strings | no | `["RS256"]` | The ID-token signature algorithms this provider is trusted to use: one to four of `RS256`, `PS256`, `ES256` and `EdDSA`. `none` and the `HS*` family are refused at startup. Recorded when the provider is first created; see below. |

> **Security note.** Every provider is created disabled, and stays
> disabled, regardless of what this file's `enabled` key says — RFC
> 096-B2 makes enabling a provider an administrator's own action, at
> **Federation providers** in the admin UI (`/admin/federation-providers`).
> **A user created by `provision_on_first_login` is enabled at once**: anyone
> the upstream vouches for is signed in on their first visit. (Until
> 0.90.0 this note said such a user was created disabled. That was never
> true.)

> **`email` and `email_verified` must be in the ID token.** sui-id never
> calls the upstream's userinfo endpoint (RFC 096) — only claims present
> in the verified ID token are read. Some providers send these claims
> from userinfo by default and omit them from the ID token; Duende
> IdentityServer is one, unless the client sets
> [`AlwaysIncludeUserClaimsInIdToken`](https://docs.duendesoftware.com/identityserver/fundamentals/claims-lifecycle/)
> (default `false`) to `true`. Without a verified email in the ID
> token, every first sign-in through `provision_on_first_login` is
> refused.

> **Which fields take effect after the provider already exists.**
> `display_name` is reconciled from this file on every boot, and applies
> without disabling the provider — it is not part of its trust policy.
> Every other field — `issuer`, `client_id`, `client_secret_env`,
> `scopes`, `provision_mode`, `allowed_origins` and `id_token_algs` — is
> trust policy, and a change to any one of them also applies on the next
> boot, but **disables the provider** until an administrator reviews and
> re-enables it; the server logs a warning naming the provider and which
> fields changed. Values are compared by meaning: listing the same scopes,
> algorithms or origins in another order is not a change. (The secret is compared by its plaintext, in constant
> time, after decrypting the stored value — never logged either way.)
> Check and re-enable it the same way as after an upgrade: **Federation
> providers** in the admin UI (`/admin/federation-providers`), then
> **Check and enable**.
>
> Removing a provider's `[[federation_providers]]` block entirely also
> disables it (keeping its existing federation links) rather than
> deleting anything. Restoring the block keeps it disabled until it is
> checked and re-enabled.
>
> `enabled` is never read into the stored provider at all, at creation or
> on any later boot — RFC 096-B2 made enabling a provider an
> administrator's own action, at **Federation providers** in the admin
> UI (`/admin/federation-providers`). A block that still sets it logs one
> warning per boot saying the key no longer enables anything.

```toml
[[federation_providers]]
slug               = "google"
display_name       = "Google"
issuer             = "https://accounts.google.com"
client_id          = "1234567890-abc.apps.googleusercontent.com"
client_secret_env  = "GOOGLE_CLIENT_SECRET"
scopes             = "openid email profile"
# With Google, provision_on_first_login admits anyone with a Google account.
provision_mode     = "provision_on_first_login"
# Google's discovery document names endpoints on four origins.
allowed_origins    = [
  "https://accounts.google.com",
  "https://oauth2.googleapis.com",
  "https://openidconnect.googleapis.com",
  "https://www.googleapis.com",
]

[[federation_providers]]
slug               = "entra"
display_name       = "Microsoft"
issuer             = "https://login.microsoftonline.com/{tenant}/v2.0"
client_id          = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
client_secret_env  = "ENTRA_CLIENT_SECRET"
provision_mode     = "link_only"
# Microsoft's userinfo endpoint is on Microsoft Graph, not the issuer's origin.
allowed_origins    = [
  "https://login.microsoftonline.com",
  "https://graph.microsoft.com",
]
```

The `allowed_origins` above are what each provider's discovery document
named on 2026-11-12. A provider can move an endpoint. If **Check and
enable** is refused with *"the upstream's discovery document was refused"*,
the server log names the endpoint and its origin. Add that origin, if you
trust it, and restart.

---

## Prometheus metrics (RFC 006)

The metrics endpoint is configured by two keys in [`[server]`](#server),
`metrics_enabled` and `metrics_listen_addr`. There is no `[metrics]` section:
the configuration loader rejects unknown sections, so a `[metrics]` table
stops sui-id from starting.

The endpoint is protected by a bearer token stored (hashed) in the database.
Issue or rotate the token with:

```sh
sui-id admin metrics-token rotate --config sui-id.toml
```

Present the token as `Authorization: Bearer <token>` or as a session cookie
(for browser access via the admin panel).

```toml
[server]
listen_addr         = "127.0.0.1:8801"
issuer              = "https://id.example.com"
metrics_enabled     = true
metrics_listen_addr = "127.0.0.1:9091"   # Only reachable from localhost / monitoring agent.
```

## See also

- [Deployment guide](../guides/deployment.md) — reverse-proxy configuration,
  systemd unit, `trusted_proxies` guidance.
- [Operators reference](../guides/operators.md) — backup procedures, session
  policy, HIBP setup, key rotation, role management.
- [Upgrade guide](../guides/upgrade.md) — version-specific migration notes.
