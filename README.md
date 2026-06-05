# sui-id

> A self-hosted, single-binary OpenID Connect provider written in Rust.
> Quiet, careful, and small enough to read end to end.

![Status](https://img.shields.io/badge/status-unstable-red)
[![crates.io](https://img.shields.io/crates/v/sui-id?label=rust)](https://crates.io/crates/sui-id)
[![Rust Documentation](https://docs.rs/sui-id/badge.svg?version=latest)](https://docs.rs/sui-id)
[![Dependency Status](https://deps.rs/crate/sui-id/latest/status.svg)](https://deps.rs/crate/sui-id)
[![License](https://img.shields.io/github/license/nabbisen/sui-id)](https://github.com/nabbisen/sui-id/blob/main/LICENSE)

![logo](https://raw.githubusercontent.com/nabbisen/sui-id/main/docs/assets/logo.png)

---

## Overview

sui-id is an Identity-as-a-Service you run yourself. It speaks OpenID Connect
on the front end, stores its data in a single encrypted SQLite file, and ships
as one statically linked binary. There is no separate database service, no
embedded JavaScript runtime, and no ambient cloud dependency.

The name "sui" comes from Latin *sui generis* — "of its own kind." sui-id is
not aiming to replace large IDaaS products. It is built for the case where you
want an OIDC provider that one person can hold in their head and one operator
can keep healthy on a single VM.

## Scope

sui-id is a **single-realm, first-party IdP.** It manages one flat
namespace of users, one flat namespace of clients, and one global
admin role. There is no `tenant_id` column, no organisation table,
no group table, and no per-tenant scoping anywhere in the schema.
This is a deliberate design choice, not an oversight.

If your use-case is a **single organisation or product** where all
users belong to the same namespace and all clients are first-party
or trusted third-party apps, sui-id is built for you.

If you need **multi-tenant isolation** (separate user namespaces per
customer, per-tenant admin roles, cross-tenant federation), sui-id
is not the right tool today. RFC 025 sketches the expansion path for
a future multi-tenant capability, but that work is explicitly
not-yet-scheduled.

## Why

Running an IDaaS in production usually means accepting one of two trade-offs:
delegate identity to a SaaS vendor (lose control, gain auditability), or stand
up Keycloak / Authelia / Authentik (gain control, gain a pile of moving
parts). sui-id picks a different point in the design space:

- **Single binary, single SQLite file.** No JVM, no separate token database,
  no message bus. `cp` is a backup.
- **Encryption that doesn't depend on filesystem trust.** Sensitive columns
  are sealed with XChaCha20-Poly1305 using a master key kept *outside* the
  database file. A stolen `.sqlite` is not a compromised one.
- **A protocol surface narrow enough to audit.** Authorization Code with
  mandatory PKCE, EdDSA-signed tokens, opaque rotating refresh tokens. No
  implicit flow, no hybrid flow, no RS256 by default.
- **A UI that wants to be quiet.** Server-rendered HTML, no client-side JS
  bundle, dark-mode aware.

If you want SAML, LDAP federation, dynamic client registration over the
internet, or twenty IdP integrations out of the box: sui-id is not for you,
and that's a feature.

## Quick start

```bash
# Install the binary from crates.io
cargo install sui-id

# Generate a starter config
sui-id --print-sample-config > sui-id.toml

# Edit issuer / paths if needed, then start
sui-id --config sui-id.toml
```

If you'd rather build from source:

```bash
git clone https://github.com/nabbisen/sui-id && cd sui-id
cargo build --release
./target/release/sui-id --print-sample-config > sui-id.toml
./target/release/sui-id --config sui-id.toml
```

On first run sui-id will:

1. Create a fresh 32-byte master key at the path in `[storage].key_file` with
   permissions `0600`. **Back this file up.** Without it, the encrypted
   columns of the SQLite file are unreadable.
2. Print a one-time **setup token** to stderr.
3. Wait for you to open `/setup` in a browser and complete the wizard.

After setup, point your relying party at:

| Endpoint            | Path                                    |
| ------------------- | --------------------------------------- |
| Discovery           | `/.well-known/openid-configuration`     |
| JWKS                | `/.well-known/jwks.json`                |
| Authorization       | `/oauth2/authorize`                     |
| Token               | `/oauth2/token`                         |
| Userinfo            | `/oauth2/userinfo`                      |
| Admin UI            | `/admin`                                |

## Features

- OpenID Connect Core 1.0 (Authorization Code + PKCE only)
- OAuth 2.0 Refresh Token grant with token rotation on each use
- EdDSA / Ed25519 token signing, advertised through JWKS
- Argon2id password hashing
- Field-level encryption of refresh tokens, credentials, and signing-key material
- Append-only audit log with SHA-256 hash-chain integrity, filter, and CSV export
- Two-factor authentication: TOTP (authenticator app) and WebAuthn passkeys
- Recovery codes (8 per TOTP enrollment), single-use
- Forgot-password and password-change email notifications via SMTP
- Have I Been Pwned breach-password checking (off / warn / block)
- Step-up authentication for sensitive admin operations
- Server-rendered confirmation screens for all destructive operations
- Session idle timeout and concurrent session cap (both opt-in, per server settings)
- Per-user and server-wide language preference (Japanese, English, Chinese)
- Dev mode: one-flag startup with seed data, no setup wizard
- Per-IP rate limiting on login, token, and setup endpoints
- Background garbage collection of expired authorization codes, sessions,
  and refresh tokens
- `/healthz` endpoint that does not leak system state
- Setup wizard with one-time token, no default credentials
- TOML configuration; master key resolved from env or file
- Single-process, single-binary, single-file deployment
- Built on Rust 1.91 with `unsafe_code = "forbid"` enforced workspace-wide

## Design notes

- **Storage:** SQLite via `rusqlite` with the bundled feature; one connection,
  WAL mode. The schema lives in `crates/sui-id-store/src/migrations/`.
- **Crypto:** XChaCha20-Poly1305 for column encryption; Ed25519 for JWT
  signing; Argon2id for passwords. Implementations are pulled from the
  RustCrypto ecosystem.
- **HTTP:** Axum 0.8 over Tokio. The router is one file: `crates/sui-id/src/router.rs`.
- **UI:** Leptos 0.8 in SSR-only mode. No WASM is shipped; pages are rendered
  server-side and HTML POSTs handle state changes. JavaScript is reserved for
  WebAuthn credential ceremonies only; all other interactions are pure HTML forms.
  Destructive operations route through server-rendered confirmation screens with
  step-up authentication (RFC 030).
- **Observability:** `tracing` + `tracing-subscriber`. Choose `fmt` or `json`
  output via config.

## Project layout

```
crates/
├── sui-id-shared   DTOs, typed ids, public error type
├── sui-id-store    SQLite, migrations, column encryption, repositories
├── sui-id-core     Domain logic: passwords, JWT, OIDC, setup, sessions
├── sui-id-web      Leptos SSR pages (login, setup, admin panel)
└── sui-id          Axum router, config loader, master-key resolution,
                    embedded static assets, the `sui-id` binary
docs/               Operator and integrator documentation
```

## Documentation

- [`docs/deployment.md`](https://github.com/nabbisen/sui-id/blob/main/docs/deployment.md) — chronological,
  opinionated walkthrough from a fresh Linux server to a hardened
  production install. Start here for a first-time deployment.
- [`docs/operators.md`](https://github.com/nabbisen/sui-id/blob/main/docs/operators.md) — reference for
  configuration fields, the master key, GC, the audit log schema,
  and routine operational tasks.
- [`docs/integrators.md`](https://github.com/nabbisen/sui-id/blob/main/docs/integrators.md) — pointing an application
  at a sui-id instance: discovery, registration, the OIDC flow, and the
  shape of the tokens.
- [`docs/threat-model.md`](https://github.com/nabbisen/sui-id/blob/main/docs/threat-model.md) — what sui-id defends
  against, what it does not, and what assumptions the operator must
  uphold for the design to work.
  crates.io. Not relevant to end users.

