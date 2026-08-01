# RFC 076 — Configuration reference documentation

**Status.** Implemented (v0.62.0)
**Priority.** P2 — recommended before v1.0.
**Tracks.** Verification-soak documentation.
**Touches.** `docs/src/reference/configuration.md` only. No code changes.

## Implementation note (v0.62.0)

Replaced the 6-line stub with a 193-line complete reference covering:

- **All 10 config fields** across all 5 TOML sections (`[server]` × 4,
  `[storage]` × 2, `[tokens]` × 3, `[log]` × 4, `[security]` × 1), each
  with type, required/optional status, default value, valid values/range,
  and description.
- **3 environment variables** (`SUI_ID_MASTER_KEY`, `SUI_ID_SETUP_TOKEN`,
  `SUI_ID_BACKUP_PASSPHRASE`) with purpose and usage notes.
- **4 runtime flags** (`--config`, `--dev`, `--print-sample-config`,
  `--help`) and a **5-row subcommand reference**.
- **Startup validation rules** for `issuer`, `trusted_proxies`,
  `access_lifetime_secs`, and `refresh_lifetime_secs`.
- **Minimal configuration** (4-line TOML, no extras needed).
- **Production-ready annotated configuration** (full `[server]`,
  `[storage]`, `[tokens]`, `[log]`, `[security]` with comments).
- **Cross-references** to `deployment.md`, `operators.md`, and
  `upgrade.md`.

No code changes. `cargo check --workspace` clean before and after.

---

---

## Background

`docs/src/reference/configuration.md` currently reads:

> # Configuration reference
>
> See [`docs/operators.md`](../../docs/src/guides/operators.md) for the full configuration reference.
>
> This page will be expanded in a future release to provide a structured
> field-by-field reference with defaults and valid value ranges.

The operators guide (`operators.md`) describes configuration _in narrative
form_ — tuning decisions, relationships, consequences. It is an excellent
guide but it is not a complete field reference; several settings and their
valid values are not mentioned there. A distinct reference page is needed.

---

## Goal

Write `docs/src/reference/configuration.md` as a complete, accurate,
self-contained field reference derived from `crates/sui-id/src/config.rs`
and `crates/sui-id/src/main.rs`. The page must:

1. Cover every field in every `[section]` of the TOML config.
2. State the Rust type, the TOML type, whether the field is required or
   optional, the default value, and valid values or ranges.
3. Include a runnable minimal configuration block and a production-ready
   annotated block.
4. Document environment variable overrides and the `--dev` flag side effects.
5. Note the `validate()` constraints that are checked at startup.
6. Cross-reference `operators.md` and `deployment.md` where deeper context
   lives.

---

## Design

Below is the complete content of the new `configuration.md`.

---

### Document content (to become `docs/src/reference/configuration.md`)

---

```markdown
# Configuration reference

sui-id is configured via a single [TOML](https://toml.io/) file — by
convention `sui-id.toml` but configurable with `--config <path>`. Run:

```sh
sui-id --print-sample-config
```

to print a valid, minimal configuration and exit. The printed config
expresses every default so it can be edited in place.

> **Note.** Two settings live _outside_ the TOML file because they are
> secrets that must not appear in plaintext config:
>
> | Variable | Purpose |
> |---|---|
> | `SUI_ID_MASTER_KEY` | Base64-encoded 32-byte master encryption key. Overrides `[storage].key_file`. On first start, if neither is present, the key is generated and written to `key_file`. |
> | `SUI_ID_SETUP_TOKEN` | Override the one-time setup token printed to stderr on first start. Optional; useful for scripted provisioning. |
> | `SUI_ID_BACKUP_PASSPHRASE` | Passphrase used by `sui-id backup` and `sui-id restore`. Read by the CLI so operators do not have to enter it interactively in scripts. |

---

## `[server]`

Controls the listening address and the public OIDC identity.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `listen_addr` | string | **yes** | — | `host:port` for the HTTP listener. Example: `"0.0.0.0:8801"`. No TLS at this layer — deploy behind a TLS-terminating reverse proxy in production. |
| `issuer` | string | **yes** | — | The external HTTPS URL used as the OIDC `issuer` claim and JWKS base URL. Must be an absolute `http://` or `https://` URL. Must match the URL your relying parties discover at `/.well-known/openid-configuration`. |
| `cookie_secure` | bool | no | `false` | Set the `Secure` flag on session cookies. Must be `true` in production behind HTTPS; leave `false` only for local development. When `false`, the admin dashboard shows a "cookie insecure" warning. |
| `trusted_proxies` | array of strings | no | `[]` | CIDR ranges of reverse proxies whose `X-Forwarded-For` header should be trusted for rate-limiting. Empty = always use the socket peer IP. See [deployment.md](../../docs/src/guides/deployment.md) for guidance on setting this correctly — an over-broad value lets clients spoof their IP and bypass rate limits. Example: `["10.0.0.0/8", "172.16.0.0/12"]`. |

**Startup validation:** `issuer` must be an absolute `http(s)://` URL.
Each entry in `trusted_proxies` must be a valid CIDR block; startup
fails on parse error.

---

## `[storage]`

File paths for the database and the master key.

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `db_path` | path | **yes** | — | Path to the SQLite database file. Created on first start if it does not exist. Relative paths are resolved from the working directory of the process. Example: `"./sui-id.sqlite"`. |
| `key_file` | path | **yes** | — | Path to a file holding the base64-encoded 32-byte master key. On first start, if the file does not exist **and** `SUI_ID_MASTER_KEY` is not set, a new key is generated and written here with permissions `0600`. Once created, back this file up — without it, the encrypted columns in the SQLite file are permanently unreadable. |

> **Backup note.** A complete backup is two files: `db_path` + `key_file`.
> The built-in `sui-id backup` command creates an encrypted archive
> containing both. See [operators.md](../../docs/src/guides/operators.md#backup-and-restore).

---

## `[tokens]`

Token lifetime settings. All values are in seconds.

| Field | Type | Required | Default | Valid range | Description |
|---|---|---|---|---|---|
| `access_lifetime_secs` | integer | no | `900` (15 min) | > 0 | Lifetime of access tokens issued at the token endpoint. Short-lived by design — OIDC access tokens are not revocable, so a shorter lifetime reduces the blast radius of a stolen token. |
| `id_token_lifetime_secs` | integer | no | `900` (15 min) | > 0 | Lifetime of ID tokens. Should match or be close to `access_lifetime_secs`. |
| `refresh_lifetime_secs` | integer | no | `1209600` (14 days) | > `access_lifetime_secs` | Lifetime of refresh tokens. A refresh token is rotated on every use (the old token is immediately revoked). 14 days is appropriate for "stay signed in on this device." Set lower for higher-security deployments. |

**Startup validation:** `access_lifetime_secs` must be positive.
`refresh_lifetime_secs` must exceed `access_lifetime_secs`.

---

## `[log]`

Logging configuration. Uses the `tracing` crate.

| Field | Type | Required | Default | Valid values | Description |
|---|---|---|---|---|---|
| `format` | string | no | `"fmt"` | `"fmt"`, `"json"` | `"fmt"` — structured, human-readable lines (good for development and most log aggregators). `"json"` — one JSON object per line (good for ELK, Loki, Datadog, etc.). |
| `filter` | string | no | `"info,sui_id=info,sui_id_core=info,sui_id_store=info"` | any `tracing-subscriber` env-filter expression | Controls log verbosity per-module. Example for debug output: `"debug,h2=warn,hyper=warn"`. |
| `access_log` | bool | no | `false` | — | When `true`, emit one `INFO` line per HTTP request: method, path, status code, and a request-id. Disabled by default — the default `filter` captures enough. Enable for traffic analysis or debugging. Also enabled automatically by `--dev`. |
| `file` | path or null | no | `null` (stderr only) | — | When set, write logs to daily-rotated files at this path **in addition to** stderr. The path is a directory; files are named `sui-id.YYYY-MM-DD.log`. Example: `"/var/log/sui-id"`. |

---

## `[security]`

Security-policy knobs. Currently exposes one setting.

| Field | Type | Required | Default | Valid values | Description |
|---|---|---|---|---|---|
| `max_lockout` | string | no | `"24h"` | `"15min"`, `"1h"`, `"4h"`, `"12h"`, `"24h"`, `"48h"` | Maximum time an account stays automatically locked after repeated failed sign-in attempts. The lockout uses a progressive backoff curve; `max_lockout` is the cap. An admin can always unlock immediately via `sui-id admin unlock-user <username>`. The restricted set of values is intentional — arbitrary integers would let operators accidentally choose a value that locks real users out over weekends or vacations. NIST SP 800-63B recommends at least one day for higher-assurance tiers; the default `24h` reflects this. |

> **Note.** Per-user and server-wide session limits (idle timeout, max
> concurrent sessions) are configured via the admin UI under
> **Settings → Security**, not in the TOML file. The TOML file governs
> login-time policy; the admin UI governs session policy.

---

## Runtime flags

These are CLI flags, not TOML settings. They apply only to the current
process invocation.

| Flag | Description |
|---|---|
| `--config <path>` | Path to the configuration file. Default: `sui-id.toml` in the current directory. |
| `--dev` | Start in development mode. Seeds the database with test data (one admin user `admin` / password `changeme`, one OIDC client), sets `cookie_secure = false`, disables HIBP, disables account lockout, enables access logging. **Never use in production.** |
| `--print-sample-config` | Print a minimal, valid configuration to stdout and exit. Pipe to `> sui-id.toml` to bootstrap a new instance. |
| `--help` | Print full usage including subcommand reference. |

---

## Subcommands

| Subcommand | Description |
|---|---|
| `sui-id backup --config <c> --dest <path>` | Create an encrypted archive of the database and key file. |
| `sui-id restore --config <c> --src <path>` | Restore from an archive. Requires confirmation. |
| `sui-id verify-backup --src <path>` | Verify archive integrity without writing any files. |
| `sui-id admin unlock-user --config <c> <username>` | Immediately clear a locked account. |
| `sui-id admin rotate-key --config <c>` | Rotate the signing key (creates a new key, retires the old). |

See `sui-id --help` for full flag listings.

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

All other sections use their defaults.

---

## Production-ready annotated configuration

```toml
[server]
# Replace with your actual domain.
listen_addr = "127.0.0.1:8801"   # Listen on loopback; nginx handles public TLS.
issuer      = "https://id.example.com"
cookie_secure   = true            # Required behind HTTPS.
trusted_proxies = ["10.0.0.0/8"] # Set to your reverse-proxy subnet.

[storage]
db_path  = "/var/lib/sui-id/sui-id.sqlite"
key_file = "/etc/sui-id/sui-id.key"         # Backed up separately from the DB.

[tokens]
access_lifetime_secs  = 900       # 15 min (default; appropriate for most apps).
refresh_lifetime_secs = 86400     # 24 h (tighter than default for this deployment).

[log]
format     = "json"              # For log aggregation.
access_log = true                # Useful in production for traffic analysis.
file       = "/var/log/sui-id"   # Daily-rotated, in addition to stderr/journald.

[security]
max_lockout = "24h"              # Default; suits most deployments.
```

---

## Cross-references

- [Deployment guide](../../docs/src/guides/deployment.md) — step-by-step server setup,
  including reverse-proxy configuration and `trusted_proxies` guidance.
- [Operators reference](../../docs/src/guides/operators.md) — backup procedures, session
  policy, key rotation, HIBP setup, and routine operational tasks.
- [Upgrade guide](../../docs/src/guides/upgrade.md) — version-specific migration notes.
```

---

## Implementation

This RFC is pure writing — no code changes. The entire diff is:

```diff
docs/src/reference/configuration.md   1 file, +N lines (stub replaced)
```

**Process:**

1. Replace the 6-line stub in `docs/src/reference/configuration.md` with
   the content specified in this RFC.
2. Verify accuracy against `crates/sui-id/src/config.rs`: every field,
   type, default, and validation rule must match what the code actually
   enforces.
3. `cargo check --workspace` — trivial (no code change), but confirms the
   doc references compile.

**Ongoing maintenance:** When new config fields are added (e.g., future
settings for SMTP, federation, or metrics), the RFC adding them should
update `configuration.md` as part of the same changeset. Add an item to
the project's RFC template checklist:

> - [ ] If this RFC adds or changes config fields, update
>   `docs/src/reference/configuration.md`.

## Acceptance criteria

- [ ] `docs/src/reference/configuration.md` covers every field in every
  `[section]` of `Config` as defined in `crates/sui-id/src/config.rs`.
- [ ] All defaults, valid values, and startup validations match the code.
- [ ] The two example configurations (minimal and production-ready) are
  valid TOML and pass `Config::validate()`.
- [ ] Runtime flags and subcommands are documented.
- [ ] The environment variables (`SUI_ID_MASTER_KEY`, `SUI_ID_SETUP_TOKEN`,
  `SUI_ID_BACKUP_PASSPHRASE`) are listed.
- [ ] Cross-references to `operators.md` and `deployment.md` are correct.
- [ ] The stub text and its redirect link are removed.
- [ ] `cargo check --workspace` clean.
