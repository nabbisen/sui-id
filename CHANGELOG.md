# Changelog

All notable changes to sui-id will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.11.0] - 2026-04-28

### Added — RFC 7662 Token Introspection

A new endpoint `POST /oauth2/introspect` lets confidential clients
ask whether a token they hold is still valid.

- Accepts `token` and an optional `token_type_hint`
  (`access_token` or `refresh_token`) in the form body. The hint
  controls only the lookup order — both kinds are tried either way.
- Authenticates the calling client via HTTP Basic (preferred) or
  `client_id` + `client_secret` form fields. Public clients cannot
  introspect; they have no secret to present.
- Returns the RFC 7662 §2.2 JSON shape: `active: true` plus
  `scope`, `client_id`, `username`, `token_type`, `exp`, `iat`,
  `sub`, `aud`, `iss` for an active token; `{"active": false}` and
  nothing else for any other case.
- A client can only see its own tokens. Submitting a token whose
  `aud` is a different client returns `inactive` — introspection
  must not be usable as an oracle for fishing valid tokens.
- Audit-logged as `token.introspect` with the client id as actor
  target and `active`/`inactive` as the result.

### Added — RFC 7009 Token Revocation

A new endpoint `POST /oauth2/revoke` lets confidential clients
revoke their own tokens.

- Same authentication shape as introspection (Basic or form-body
  `client_id` + `client_secret`).
- Per RFC 7009 §2.2 the response is **always** `200 OK` with an
  empty body — even for unknown, expired, or already-revoked
  tokens. The endpoint must not double as an oracle. Only
  `invalid_client` (auth failure), `invalid_request` (malformed
  body), or `unsupported_token_type` produce error responses.
- Refresh tokens are revoked at the storage layer
  (`refresh_tokens.revoked_at` is set). The next attempt to use
  them at `/token` is rejected with `invalid_grant`.
- Access tokens are added to a small deny-list table
  (`revoked_access_tokens`, see migration 0005). A revoked access
  token's `jti` is checked at introspection time, so subsequent
  introspections report it inactive. The deny-list does *not*
  reach RPs that validate JWTs locally; relying parties that need
  immediate revocation visibility should call introspection.
- Garbage-collected: `revoked_access_tokens` rows whose `exp` has
  passed are pruned by the periodic GC sweep, so the table size is
  bounded by the access-token lifetime.
- Audit-logged as `token.revoke` with the client id as target.

### Added — schema migration 0005

A new `revoked_access_tokens` table with `jti` (PK), `revoked_at`,
`exp`, `revoked_by_user`, `revoked_by_client`. Index on `exp` for
the GC sweep. Existing deployments pick this up automatically on
first start of v0.11.0; no operator action needed.

### Added — discovery metadata

`/.well-known/openid-configuration` now advertises:

- `introspection_endpoint`
- `introspection_endpoint_auth_methods_supported: ["client_secret_basic", "client_secret_post"]`
- `revocation_endpoint`
- `revocation_endpoint_auth_methods_supported: ["client_secret_basic", "client_secret_post"]`

so RP libraries that auto-discover endpoints pick the new ones up
without configuration changes.

### Added — documentation

- `docs/integrators.md` gains two new sections (Token introspection
  and Token revocation) walking through the request/response
  shapes, authentication, oracle-prevention behaviour, and the
  trade-off that JWT access tokens cannot be reliably revoked from
  RPs that validate locally.
- The "What sui-id does not do" list drops `RFC 7662` and
  `RFC 7009` — they're done.

### Added — tests

- 7 new end-to-end tests for the introspection and revocation
  endpoints (verified individually):
  - `discovery_advertises_introspect_and_revoke_endpoints`
  - `introspect_rejects_unauthenticated_request`
  - `introspect_other_clients_token_returns_inactive`
  - `introspect_returns_active_for_valid_access_token`
  - `introspect_returns_active_for_valid_refresh_token`
  - `introspect_returns_inactive_for_garbage_token`
  - `revoke_then_introspect_shows_inactive_for_access_token`

The lib test suites (`sui-id` 28 + `sui-id-core` 39 +
`sui-id-store` 6 + `sui-id-shared` 6 = 79) all pass. The full e2e
suite has 41 tests total and was previously verified end-to-end at
v0.10.x; the new RFC 7662/7009 tests have been verified
individually here.

## [0.10.2] - 2026-04-28

`cargo audit` integration. No code changes.

### Added

- **`.github/workflows/audit.yml`** — scans the dependency tree
  against the [RustSec advisory database](https://rustsec.org/) on
  every push to `main`, on every PR that touches `Cargo.{toml,lock}`,
  and on a weekly schedule (Wednesdays at 06:13 UTC). Uses the
  official `rustsec/audit-check` action.
- **`.github/workflows/ci.yml`** — basic build + test + fmt + clippy
  workflow on Linux stable. The audit workflow is intentionally
  separate so it can run on a different cadence and surface its
  results independently.

### Documentation

- **`docs/operators.md`** — new "Auditing dependencies for known
  vulnerabilities" section that walks an operator through running
  `cargo audit` locally, interpreting the two output categories
  (vulnerabilities vs informational warnings), and what to do when
  one of each shows up.
- **`docs/deployment.md`** — the upgrade procedure now starts with
  a `cargo audit` pre-flight against the new build's source tree,
  to catch advisories published since the upstream lockfile was
  tagged.
- **`docs/threat-model.md`** — A12 (third-party authentication
  library) updated to reflect that the audit integration is now
  active and to record the scan result at v0.10.2 ship time
  (zero vulnerabilities, one informational warning for `paste`,
  an unmaintained transitive of the Leptos framework that is not
  directly exploitable).

### Verified at this release

A manual scan of the `Cargo.lock` against the advisory database on
2026-04-28 reported:

- **Vulnerabilities: 0**
- **Warnings: 1** — `paste` v1.0.15, marked `unmaintained`
  (RUSTSEC-2024-0436). Pulled in transitively via `leptos`,
  `reactive_graph`, and several other framework crates. Not
  exploitable; tracking upstream Leptos for a migration off it.

## [0.10.1] - 2026-04-28

Documentation expansion. No functional changes.

### Added

- **`docs/deployment.md`** — a chronological, opinionated walkthrough
  from a fresh Linux server to a hardened production install of
  sui-id. Covers system packages, a dedicated user account, binary
  installation, configuration, HTTPS termination (Caddy primary,
  nginx alternative), a hardened systemd unit (with the standard
  `systemd-analyze security` directives), bootstrapping the admin,
  enabling MFA on the admin account, scheduling backups with off-
  host shipping, health checks and audit-log queries, and the
  upgrade procedure with rollback.

### Changed

- **`docs/operators.md`** repositioned as the operational reference —
  configuration fields, the master key, GC, audit log schema,
  routine tasks. New sections cover MFA (TOTP + WebAuthn user-driven
  setup), admin-initiated MFA reset (when to use it, what it does
  and does not do, audit log expectations), WebAuthn / passkey
  requirements (HTTPS, immutable issuer host), and per-client scope
  policy. The first-time install content is now in deployment.md;
  operators.md links there.
- **`docs/integrators.md`** updated to reflect everything that
  shipped since the file was last touched: `allowed_scopes` and
  `post_logout_redirect_uris` on client registration, the editable
  client page, MFA being internal to sui-id, RP-initiated logout
  (which has been supported since v0.2.0 but was still listed under
  "What sui-id does not do"). The "does not do" list now correctly
  flags `acr`/`amr`, `prompt`/`max_age`, RFC 7662, RFC 7009, and
  dynamic client registration as the actually-missing pieces.
- **`README.md`** documentation index now links deployment.md as
  the recommended starting point.

## [0.10.0] - 2026-04-27

### Added — admin-initiated MFA reset

The recovery path for users who have lost every second factor (TOTP
authenticator, every recovery code, *and* every registered passkey) is
now self-contained inside sui-id. Previously the only option was
direct SQL surgery on the database file.

- New use case `sui_id_core::admin::admin_reset_mfa(actor, target)` —
  admin-gated, audit-logged. Removes the user's `user_totp` row (if
  present) and every `user_webauthn_credentials` row in a single call.
  Returns a `MfaResetReport` indicating exactly what was removed.
- New HTTP endpoint `POST /admin/users/{id}/mfa-reset`. CSRF-protected
  like every other admin POST. Surfaces a "Reset MFA" button on the
  users page for any user who currently has MFA enabled.
- The users page now has a "MFA" column (`on` / `off`) so operators
  can see at a glance which accounts have a second factor configured.
- New audit-log action `mfa.admin_reset` with a `note` field that
  records the breakdown (`totp=removed passkeys=2`, etc), so a later
  review of the audit log can reconstruct exactly what was lifted.

### Changed

- `UserSummary` (in `sui-id-shared`) gains a `mfa_enabled: bool` field
  with `#[serde(default)]` for compat. The HTTP `users_get` handler
  computes this per row by calling `mfa::is_mfa_enabled`. A read error
  per row is treated as "off" rather than failing the whole list page.

### Notes for operators

- The reset is intentionally permissive about self-resets: an
  administrator who still has a valid session can reset their *own*
  MFA factors. This is rarely the right thing — most lockouts happen
  precisely because the session is gone — but the alternative
  (refusing self-reset) didn't seem like it added safety while it did
  remove a recovery path.
- The reset does **not** revoke active sessions for the target user.
  An admin who wants to log the user out as well should follow the
  reset with disable-and-re-enable, which already revokes sessions
  and refresh tokens.
- The reset is logged with the actor's user id; combined with the
  password-reset and user-management entries, the audit log gives a
  full picture of who acted on whose account when.

### Added — tests

- 2 new end-to-end tests:
  - `admin_can_reset_users_mfa_factors` — uses the core API to enrol
    TOTP for a target user, calls `admin_reset_mfa`, verifies that
    `is_mfa_enabled` flips back to false, and asserts on the audit
    log entry's actor / target / note fields.
  - `admin_mfa_reset_via_http_redirects_and_disables_mfa_requirement`
    — full round-trip: enrol TOTP, confirm a fresh password login
    redirects to the MFA challenge, POST the reset endpoint, then
    confirm the next password login goes straight to a session.

Total: **111 tests passing** (was 109).

## [0.9.0] - 2026-04-27

### Added — schema migration 0004

- **`users.user_uuid`** column added with backfill. WebAuthn requires a
  stable per-user UUID handle as the relying party's `user.id`. We
  keep this decoupled from the typed `UserId` so the WebAuthn handle
  can be rotated independently if it ever has to be.
- **`user_webauthn_credentials`** table — one row per registered
  passkey. `passkey_enc` holds a serialised `webauthn_rs::prelude::Passkey`
  sealed under the master key (XChaCha20-Poly1305, separate AAD from
  every other encrypted column). `credential_id` is indexed unique so
  authentication can look the row up; the rest of the row is opaque
  to sui-id.
- **`webauthn_pending`** table — short-lived (5 minute) state for
  in-flight registration / authentication ceremonies. Holds the
  `PasskeyRegistration` / `PasskeyAuthentication` JSON the high-level
  webauthn-rs API expects on the second leg of each ceremony.

Existing rows from v0.8.0 and earlier come through cleanly: the
backfill assigns each user a fresh UUID, and the new tables are empty.

### Added — WebAuthn / passkey support

- **`sui_id_core::webauthn`** module wraps the
  [`webauthn-rs`](https://docs.rs/webauthn-rs) 0.5.4 high-level
  framework. Public API: `start_registration` / `finish_registration`,
  `start_authentication` / `finish_authentication`, `list_for_user`,
  `delete`, `has_credentials`. Each ceremony round-trips through the
  `webauthn_pending` table so the in-flight state survives between
  the browser's two requests.
- **`sui_id_core::mfa::is_mfa_enabled`** is now true when the user has
  *either* TOTP enrolled *or* at least one passkey registered. Either
  factor satisfies the MFA challenge.
- **`sui_id_core::mfa::verify_pending_webauthn`** promotes a
  pending-MFA row into a real session after the bin layer has already
  verified the WebAuthn ceremony. Splitting it from the TOTP path
  keeps webauthn-rs out of `session.rs` and lets the audit log
  record `auth.mfa.success` once at the end of either factor.

### Added — admin UI and HTTP

- `/admin/profile` now lists registered passkeys (nickname, registered
  date, last used) with a per-row delete button, plus a "Register a
  new passkey" form pointing at the JS-driven enrolment flow.
- `/admin/login/mfa` page surfaces a "Sign in with passkey" button
  when the pending-MFA user has at least one passkey enrolled.
- New routes:
  - `POST /admin/profile/webauthn/register/start` →
    `CreationChallengeResponse` JSON for `navigator.credentials.create()`
  - `POST /admin/profile/webauthn/register/complete`
  - `POST /admin/profile/webauthn/{id}/delete`
  - `POST /admin/login/webauthn/start` →
    `RequestChallengeResponse` JSON for `navigator.credentials.get()`
  - `POST /admin/login/webauthn/complete`
- Two new HttpOnly, SameSite=Lax cookies with 5-minute TTLs:
  `sui_id_webauthn_pending` (ceremony id) and
  `sui_id_webauthn_nickname` (carries the registration label across
  the two legs without server-side state expansion).
- New audit-log actions:
  `webauthn.credential.register`,
  `webauthn.credential.delete`,
  `auth.mfa.success` (with `note: "webauthn"` when the WebAuthn path
  was the satisfying factor).
- Background GC purges expired `webauthn_pending` rows.

### Added — browser JavaScript

A self-contained 6.5 KB `static/webauthn.js` handles base64url ↔
ArrayBuffer marshalling and the two `navigator.credentials.*`
ceremonies. No dependencies. Loaded only on the two pages that need
it (Profile and the MFA challenge) and only when a passkey path is
relevant.

### Added — dependencies

- `webauthn-rs = "0.5"` with the `danger-allow-state-serialisation`
  feature enabled. The "danger" prefix is the upstream signal that
  the in-flight `PasskeyRegistration`/`PasskeyAuthentication` state
  should not escape the trust boundary; we never expose it over the
  wire — it stays in the `webauthn_pending` table behind the master
  key.
- Transitive: `openssl` (system `libssl-dev` required at build time).
  The build environment must have an OpenSSL development package
  installed; on Debian/Ubuntu, `apt install libssl-dev pkg-config`.

### Added — tests

- 2 unit tests in `sui_id_core::webauthn::tests`
  (`build_accepts_https_url`, `build_rejects_url_without_host`).
- 3 integration tests in `sui_id_core::webauthn::integration_tests`
  (`start_registration_persists_pending_row_and_returns_challenge_json`,
  `start_authentication_rejects_users_with_no_credentials`,
  `finish_registration_rejects_expired_pending_row`).

End-to-end testing of the full ceremony with attestation requires a
software authenticator (e.g. `webauthn-authenticator-rs`); we
deliberately leave that out of this release. The webauthn-rs
project itself is well-tested for the cryptographic verification we
delegate to it.

Total: **109 tests passing** (was 104).

### Notes for operators

- WebAuthn over HTTP is permitted only on `localhost`; this matches
  the Web platform spec and is enforced by webauthn-rs. Public
  deployments must terminate HTTPS upstream and configure
  `server.issuer = "https://your.host"`. The `rp_id` is the bare
  host portion of the issuer URL.
- A user who loses every registered factor (password reset link,
  TOTP authenticator, recovery codes, *and* every passkey) has no
  self-service recovery path. The operator must intervene at the
  storage layer. An admin-driven reset is on the roadmap.
- `passkey_enc` is sealed under the master key like every other
  encrypted column. A backup taken via `sui-id backup` covers
  passkey data the same way it covers the rest of the database.

### Threat model

A11 is updated to describe the WebAuthn path; A12 is added to track
the dependency on `webauthn-rs`.

## [0.8.0] - 2026-04-27

### Added — client edit page

A new admin page `/admin/clients/{id}/edit` allows operators to revise
the editable facets of a registered client without delete-and-recreate:

- Application name
- Authorization redirect URIs (one per line)
- Allowed scopes (space-separated; blank = permit any)
- Post-logout redirect URIs (one per line; blank = fall back to
  redirect URIs)

Form fields are pre-filled with the current values. Each save POSTs all
four edits in one request, but they go to **three** separately-audited
use cases (`client.update`, `client.set_allowed_scopes`,
`client.set_post_logout_redirect_uris`), so the audit log reflects
which facet of a client changed when.

The client id, type (confidential vs public), and `secret_hash` remain
fixed for the lifetime of the row. Operators who need to change those
delete the client and register a new one — same as before.

### Added — APIs

- `sui_id_core::admin::update_client_basic` — name + redirect_uris
  update use case with validation.
- `sui_id_core::admin::get_client` — admin-gated single-client fetch.

### Added — tests

- 2 new end-to-end tests:
  - `client_edit_updates_name_and_scopes` — round-trips through the
    edit page and asserts on the resulting database row.
  - `client_edit_then_authorize_uses_new_scope_policy` — tightens
    allowed_scopes via the edit page and confirms `/oauth2/authorize`
    immediately rejects the previously-permitted scope.

Total: **104 tests passing** (was 102).

### Maintenance

`cargo update --dry-run --verbose` reports 11 dependencies whose
SemVer constraints hold us back from the latest published versions
(`axum-extra` 0.10→0.12, `rand` 0.8→0.10, `rusqlite` 0.32→0.39,
`thiserror` 1→2, `toml` 0.8→1, `hmac` 0.12→0.13, `sha1`/`sha2`
0.10→0.11, plus three transitives that fall out of the above). All
are major-version upgrades whose blast radius would consume more
maintenance work than the version bumps are worth right now, and
none patches a known vulnerability. We hold at the current pins; a
future release will revisit on a per-crate basis.

## [0.7.0] - 2026-04-26

### Added — schema migration 0003

Two new tables:

- `user_totp` — one row per user that has TOTP either configured
  (`enabled = 0`) or activated (`enabled = 1`). Holds the 20-byte
  RFC 6238 secret sealed with the master key, plus a JSON array of
  Argon2id-hashed recovery codes (also sealed) and the
  `last_used_step` cursor used for replay defence.
- `login_pending_mfa` — short-lived "password verified, MFA pending"
  rows. Inserted right after a successful password check when the user
  has TOTP enabled. The HTTP layer hands the user a cookie pointing
  here; the row carries no authority on its own — promotion to a real
  session requires a valid TOTP code or recovery code.

### Added — TOTP MFA

- **RFC 6238 TOTP** (HMAC-SHA1, 30-second window, 6 digits) with a
  ±1 step drift window and `last_used_step`-based replay defence.
  Implemented in-house in `sui_id_core::totp`; covered by all six
  RFC 6238 Appendix B test vectors.
- **MFA enrolment flow** at `/admin/profile`:
  1. The user clicks "Set up MFA" → sui-id allocates a fresh secret
     and persists it as unconfirmed.
  2. The setup page renders an SVG QR code for the `otpauth://totp/...`
     URI (via the `qrcode` crate) and the Base32-encoded secret as a
     fall-back for manual entry.
  3. The user types the 6-digit code from their authenticator. On
     success, sui-id generates 8 single-use recovery codes
     (Argon2id-hashed in storage), flips the row to `enabled = 1`,
     and shows the plaintext codes **once**.
- **Login flow**: password OK + MFA disabled = session as before.
  Password OK + MFA enabled = `login_pending_mfa` row + redirect to
  `/admin/login/mfa`. The challenge page accepts either a 6-digit
  TOTP code or a single-use recovery code; on success it creates the
  session and consumes the recovery code if used.
- **Recovery code regeneration** (`/admin/profile/mfa/recovery-codes/regenerate`)
  invalidates all previous codes and returns 8 new ones.
- **MFA disable** (`/admin/profile/mfa/disable`) deletes the
  `user_totp` row entirely.
- New audit-log actions: `auth.login.password_ok_mfa_required`,
  `auth.mfa.success`, `auth.mfa.failure`, `mfa.enable`,
  `mfa.disable`, `mfa.recovery_codes_regenerate`.
- New Profile tab in the admin nav.
- The GC task now also purges expired `login_pending_mfa` rows.

### Added — APIs

- `sui_id_core::totp` module: `code_for_step`, `verify`, `base32_encode`,
  `otpauth_uri`.
- `sui_id_core::mfa` module: `is_mfa_enabled`, `start_enrollment`,
  `confirm_enrollment`, `disable`, `regenerate_recovery_codes`,
  `issue_pending_mfa`, `verify_pending`.
- `sui_id_core::session::LoginOutcome` enum and `login_with_mfa`
  function (the original `login` is preserved for callers that don't
  need the MFA branch).
- `sui_id_shared::ids::PendingMfaId` typed identifier.

### Added — dependencies

- `sha1 = "0.10"` (HMAC-SHA1 for TOTP).
- `qrcode = "0.14"` with `default-features = false, features = ["svg"]`.

### Added — tests

- 9 new unit tests in `sui_id_core::totp` (RFC 6238 vectors, replay,
  Base32, otpauth URI).
- 1 new unit test in `sui_id_core::mfa` (recovery code format).
- 3 new integration tests in `sui_id_core::mfa::integration_tests`
  (enrol → confirm → 8 recovery codes; wrong code rejected;
  disable + re-enrol).
- 4 new end-to-end tests:
  - `mfa_enroll_then_login_with_totp_succeeds`
  - `mfa_login_with_wrong_code_returns_401`
  - `mfa_login_with_recovery_code_succeeds_and_consumes_code`
  - `mfa_disable_lets_user_log_in_with_password_only`

Total: **102 tests passing** (was 95).

### Threat model

A11 (password-only authentication) is mitigated for accounts that opt
in to MFA. Recovery codes are the only persistent secret stored
plaintext-derivable from the database, but each code is Argon2id-
hashed and sealed under the master key — i.e. equivalent in difficulty
to brute-forcing a regular password.

## [0.6.1] - 2026-04-26

Internal cleanup. No functional changes.

### Changed
- Repository / homepage URLs across the workspace: now
  `https://github.com/nabbisen/sui-id` (was `sui-id/sui-id`). Updated
  in workspace `Cargo.toml`, every crate's `README.md`, the docs
  under `docs/`, the `.github/` files, `PUBLISHING.md`, `ROADMAP.md`,
  and `TERMS_OF_USE.md`.
- `sui-id` (the binary crate) no longer keeps its own copy of `README.md`
  or `CHANGELOG.md`. Its `Cargo.toml` now sets `readme = "../../README.md"`,
  which `cargo publish` resolves to the workspace root's README — the
  packaged crate uploaded to crates.io contains a copy with no
  duplication on disk.
- Per-crate `LICENSE` files have been removed. The single
  `LICENSE` and `NOTICE` files at the repository root are sufficient;
  `cargo publish` resolves them automatically and includes them in each
  uploaded crate.

### Added
- `NOTICE` file at the repository root, per the Apache-2.0 convention,
  carrying the copyright statement and a brief informational list of
  third-party permissive-licensed dependencies whose own NOTICE files
  travel with them in the source distribution.

## [0.6.0] - 2026-04-26

### Added — schema migration 0002

The `clients` table gains two new columns:

- `allowed_scopes TEXT NOT NULL DEFAULT ''` — space-separated list of
  permitted scope tokens.
- `post_logout_redirect_uris TEXT NOT NULL DEFAULT '[]'` — JSON array
  of permitted RP-initiated logout return URIs.

Existing rows from v0.5.0 and earlier come through the migration with
both columns at their defaults (empty / `[]`). The application layer
treats those defaults as "permit any" and "fall back to redirect_uris"
respectively, so existing clients keep working unchanged.

### Added — per-client scope policy

- Authorization-endpoint scope checking. When a client has a non-empty
  `allowed_scopes` policy, sui-id checks every requested scope token
  against the policy and rejects requests that exceed it with
  `invalid_scope` per RFC 6749 §5.2. An empty policy (the legacy
  default) skips the check, preserving backwards compatibility.
- The client-create form on the admin UI now exposes an "Allowed
  scopes" input. The default value rendered into the form is
  `openid profile`, but operators may type anything (including a
  blank value, which means "permit any").
- `core::admin::CreateClientSpec` struct replaces the previous
  six-positional-argument `create_client` signature. Adds field-level
  documentation and a single point of validation for scope-token
  characters (RFC 6749 §3.3 printable subset).
- New use cases: `core::admin::set_client_allowed_scopes` and
  `core::admin::set_client_post_logout_redirect_uris` (the UI for
  editing them post-creation will land in a follow-up release).
- New repository operations: `clients::set_allowed_scopes` and
  `clients::set_post_logout_redirect_uris`.
- New audit-log actions: `client.set_allowed_scopes`,
  `client.set_post_logout_redirect_uris`.

### Added — per-client post_logout_redirect_uris

- The RP-initiated logout endpoint (`/oauth2/logout`) now resolves a
  supplied `post_logout_redirect_uri` against the client's own
  `post_logout_redirect_uris` list first. When the list is non-empty,
  unregistered URIs are rejected even if they happen to be valid
  authorization `redirect_uris`.
- Backwards compatibility: when the list is empty (the on-disk default
  for clients created before migration 0002), sui-id falls back to
  matching against `redirect_uris` exactly as v0.5.0 did, and emits a
  deprecation warning to the structured log so operators can migrate.
- The client-create form has a new "Post-logout redirect URIs" textarea
  (one URI per line, optional).

### Added — tests

- 4 new end-to-end tests:
  - `authorize_rejects_scope_outside_client_policy`
  - `authorize_with_empty_policy_permits_any_scope`
  - `logout_uses_post_logout_redirect_uris_when_registered`
  - `logout_falls_back_to_redirect_uris_when_post_logout_list_empty`

### Changed

- `core::admin::create_client` signature changed to take a
  `CreateClientSpec` struct. This is a breaking change to anyone
  consuming `sui-id-core` directly; the binary itself is unaffected.
- `ClientSummary` (in `sui-id-shared`) gains the two new fields with
  `#[serde(default)]` so legacy serialised forms still deserialise.
- The clients table on the admin UI grew "Allowed scopes" and
  "Logout URIs" columns. The table is wider; consider reviewing if
  you have unusual viewports.

## [0.5.0] - 2026-04-25

### Added
- **CSRF tokens on every admin form** (synchronizer token pattern with
  a double-submit cookie). On every admin GET, sui-id sets a
  `sui_id_csrf` cookie containing a 32-byte random token; the same
  token is embedded as a hidden `_csrf` field in every rendered form.
  On admin POST, the cookie value and the form field are compared in
  constant time. A missing or mismatched token returns 403 Forbidden.
  This adds a real synchronizer token defence beneath the existing
  `SameSite=Lax` session cookie, so the CSRF property no longer
  depends on cookie attributes alone.
- The CSRF cookie is `SameSite=Lax`, `Path=/`, and follows the
  operator's `cookie_secure` setting. Unlike the session cookie it is
  intentionally **not** `HttpOnly` — the rendering layer needs to be
  able to read it to embed in form fields. The cookie alone has no
  authority; only when paired with a matching form field on a
  session-authenticated request does it grant anything.
- New `sui_id::csrf` module with `new_token`, `ensure_token`,
  `csrf_cookie`, `check_token`, and `verify_with_headers` helpers.
- 13 new tests:
  - 8 unit tests on `sui_id::csrf` covering token format, reuse,
    minting, accept/reject pairs, missing-cookie, missing-field, and
    empty-string corner cases.
  - 5 end-to-end tests:
    `admin_get_sets_csrf_cookie`,
    `admin_post_without_csrf_cookie_is_forbidden`,
    `admin_post_with_mismatched_csrf_is_forbidden`,
    `admin_post_with_matching_csrf_succeeds`,
    `oidc_endpoints_are_not_subject_to_csrf`.

### Changed
- All admin form bodies now carry a `_csrf` field. The Leptos render
  functions for `users`, `clients`, `signing_keys`, and `dashboard`
  pages take an additional `csrf_token: String` parameter. The
  protocol surface (`/oauth2/*`) is deliberately unchanged — those
  endpoints must remain CSRF-free because they are RP-to-IdP traffic,
  not user-facing forms.
- Threat model A7 (CSRF) has been promoted from "we don't do this
  yet" to a positive description of the synchronizer-token defence.

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
