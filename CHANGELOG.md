# Changelog

All notable changes to sui-id will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-04-25

### Added
- **Signing key rotation UI** at `/admin/signing-keys`. Rotation
  generates a fresh Ed25519 key, makes it the new active signing key,
  and demotes the previous key to retired status. Retired keys stay in
  the database — and therefore in `/.well-known/jwks.json` — so that
  tokens issued under them continue to verify during their remaining
  lifetime (the JWKS "grace window"). Once those tokens have expired,
  an administrator can permanently delete the retired key from the same
  page.
- **`signing_keys::retire` and `signing_keys::delete`** repository
  operations. `delete` refuses to remove the currently active key
  (returns `Conflict`), so the UI cannot accidentally leave the system
  with no signing key.
- **`admin_uc::rotate_signing_key`** and **`admin_uc::delete_signing_key`**
  use cases on the core layer, wired through the admin UI and the new
  `signing_key.rotate` / `signing_key.delete` audit-log entries.
- Navigation entry "Keys" added to the admin shell.
- `SigningKeySummary` DTO in `sui-id-shared`.
- 4 new end-to-end tests:
  - `signing_key_rotation_publishes_both_keys_in_jwks`
  - `rotation_does_not_break_existing_authorization_flow` — the old
    access token still validates after rotation, exercising the grace
    window.
  - `cannot_delete_active_signing_key`
  - `delete_retired_signing_key_drops_it_from_jwks`

### Changed
- `signing_keys::active` documentation now spells out the
  most-recently-created tie-break used during rotation. Behaviour is
  unchanged.

## [0.3.0] - 2026-04-25

### Added
- **Backup and restore subcommands.**
  - `sui-id backup --to PATH` produces a tarball containing a
    SQLite-consistent snapshot (via `VACUUM INTO`, safe to take while
    the server is running) and a verbatim copy of the master key file.
    The tarball is created with mode `0600` because it carries the key.
  - `sui-id restore --from PATH` is the inverse operation. By default it
    refuses to overwrite an existing database or key file at the
    destination paths; pass `--force` to override.
  - Both subcommands respect `--config PATH` for the storage paths and
    are documented in `--help`.
  - Backup uses an in-house POSIX ustar writer/reader rather than
    pulling in the `tar` crate; the audit surface stays small.
- **Threat model documentation** (`docs/threat-model.md`). Spells out
  the adversaries sui-id plans for (network attacker on path or
  intra-host, stolen DB file, online password guessing, CSRF, open
  redirect, JWT confusion, replay-after-revocation), the adversaries it
  does not (host-root, side-channels, phishing, RP compromise), and the
  assumptions an operator must uphold.
- README now has a `## Documentation` section linking to the operator
  guide, integrator guide, threat model, and publishing notes.
- 4 additional tests: 3 backup/restore unit tests in `sui_id::backup`
  and 1 end-to-end test that round-trips a real database with users and
  clients through `backup` → `restore` → re-open and verifies row
  counts.

### Fixed
- CLI argument parsing now correctly handles flag values whose contents
  start with `/` or otherwise resemble a positional argument (e.g.
  `--config /tmp/x.toml`). The earlier draft of the subcommand
  dispatcher misinterpreted the path as the subcommand.

## [0.2.0] - 2026-04-25

### Added
- **OpenID Connect RP-Initiated Logout 1.0** (`/oauth2/logout`).
  Accepts `id_token_hint`, `post_logout_redirect_uri`, `state`, and a
  `client_id` fallback. Verifies the ID token signature against the JWKS
  (expired hints accepted, per the spec). Validates the
  `post_logout_redirect_uri` against the hinted client's registered
  `redirect_uris` — unregistered URIs are silently ignored, never
  redirected to. Revokes all of the user's outstanding sessions and
  refresh tokens, clears the session cookie, and either redirects back to
  the RP or shows a static "Signed out" page.
- **`server.trusted_proxies`** configuration. When this CIDR list is
  non-empty *and* the immediate socket peer is in it, sui-id walks the
  `X-Forwarded-For` header from rightmost to leftmost (skipping addresses
  that are themselves trusted proxies) to derive the real client IP for
  rate-limiting and logging. Defaults to empty (always use the socket
  peer), which is the correct setting for direct exposure.
- **`sui-id.example.toml`** at the repository root: a fully commented
  starter configuration covering every setting and its trade-offs.
- **In-house CIDR matcher** (`sui_id::ipnet`) for IPv4 and IPv6, used by
  `trusted_proxies`. No additional dependency was required.
- 6 new tests: 7 CIDR unit tests in `sui_id::ipnet`, plus 3 new E2E tests
  (`logout_with_id_token_hint_revokes_session_and_redirects`,
  `logout_rejects_unregistered_post_redirect`,
  `discovery_advertises_end_session_endpoint`).
- `sui_id_core::tokens::verify_id_token` helper for ID token verification
  with optional acceptance of expired tokens (used by logout).
- `sui_id_core::session::logout_user` end-to-end logout helper that
  revokes sessions and refresh tokens together.

### Changed
- Binary crate renamed `sui-id-bin` → `sui-id`. End users now install with
  `cargo install sui-id`.
- `static/` moved from the repository root into `crates/sui-id/static/`
  so that `cargo install sui-id` produces a working binary without
  needing the surrounding workspace.

## [0.1.0] - 2026-04-25

### Added
- Initial workspace skeleton with five crates: `sui-id-shared`, `sui-id-store`,
  `sui-id-core`, `sui-id-web`, and `sui-id` (the binary crate).
- SQLite storage layer with bundled SQLite, schema migration runner, and
  per-column XChaCha20-Poly1305 encryption for sensitive fields.
- Argon2id password hashing with a minimum-length policy (no composition rules,
  per NIST SP 800-63B guidance).
- Ed25519 (EdDSA) JWT signing implementation with kid-keyed verification.
- OAuth 2.0 / OpenID Connect Core endpoints:
  - `/.well-known/openid-configuration` (Discovery)
  - `/.well-known/jwks.json` (JWKS, Ed25519 OKP keys)
  - `/oauth2/authorize` (Authorization Code, PKCE S256 mandatory)
  - `/oauth2/token` (`authorization_code` and `refresh_token` grants;
    refresh tokens rotate on each use)
  - `/oauth2/userinfo` (Bearer-authenticated)
- First-run setup flow: master key generation, signing key bootstrap,
  one-time setup token printed to stderr, single-shot create-initial-admin.
- Server-rendered admin UI built on Leptos 0.8 SSR (no WASM bundle):
  setup, login, dashboard, users, clients, audit log.
- Append-only audit log of administrative *and* authentication events
  (`auth.login.success`, `auth.login.failure` with a generic-reason note).
- Per-IP, per-route fixed-window rate limiting on `/admin/login`,
  `/oauth2/token`, and `/setup`. Rejected requests get HTTP 429 with a
  `Retry-After` header.
- `/healthz` health-check endpoint that touches the database but
  intentionally does not leak system state in its response.
- Background GC task that purges expired authorization codes, sessions, and
  refresh tokens every 15 minutes.
- Command-line flags: `--version` / `-V`, `--help` / `-h`,
  `--print-sample-config`, `--config PATH`.
- TOML configuration with validation; master key resolved from env
  (`SUI_ID_MASTER_KEY`) or a separate key file (created `0600` on first
  run).
- Workspace-wide `unsafe_code = "forbid"` and clippy lints.
- 47 unit tests across all crates plus 7 end-to-end integration tests
  covering the full setup → authorize → token → userinfo → refresh-rotation
  flow plus PKCE-mismatch, redirect-URI-mismatch, rate-limit, healthz, and
  GC negative/positive cases.
