# Changelog

All notable changes to sui-id will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.28.0] - 2026-05-04

Dev mode: a `--dev` flag that brings up a fully working OIDC IdP
in seconds, against an in-memory SQLite database, with a
pre-seeded admin / users / OIDC client. Aimed at developers
building relying parties (RPs) who need a real OIDC provider for
local testing without spending five minutes clicking through the
setup wizard each time.

### Why `--dev` instead of a separate binary

A second binary would mean two shipping artefacts, two CI
matrices, and a path for "dev features" to drift away from the
real sui-id over time. A `--dev` flag on the existing binary
means there is exactly one sui-id, the dev path is kept honest
by being right next to the production path in the same source
tree, and operators can audit the dev-mode behaviour by reading
`crates/sui-id/src/dev_mode.rs` instead of a sister codebase.

### What dev mode is and is not

**Dev mode IS**:

- An ephemeral, single-command IdP. `sui-id --dev` starts up,
  seeds an admin and two test users and one test OIDC client,
  prints the credentials to stderr in plain text, and listens
  on `127.0.0.1:8801`. RPs point at
  `http://127.0.0.1:8801/.well-known/openid-configuration` and
  go.
- Designed to be honest about what it relaxes. The startup
  banner says `cookie_secure off`, `HIBP off`, `lockout
  relaxed`, and points at the seed source. Operators see what
  is happening before any data is written.

**Dev mode is NOT**:

- A production starting point. Every cryptographic invariant
  that holds in production also holds in dev mode (see
  "Invariants kept" below). The relaxations are operational
  (cookie scope, breach-check enforcement, rate limits), not
  cryptographic.
- A second product. The same OIDC code paths run; PKCE
  S256-only is enforced exactly as in production, the
  authorization-code flow is the same, refresh-token theft
  detection is the same, AAD-bound column encryption is the
  same.

### Invariants kept

Dev mode does NOT relax any of the following:

- **PKCE.** S256-only at the authorization-code flow.
- **Column-encryption AAD binding.** Every sealed column still
  has its dedicated AAD; ciphertext-substitution attacks on
  the in-memory DB still fail to decrypt.
- **`unsafe_code = forbid`.** Workspace-level invariant; not
  relaxed.
- **Argon2id parameters.** Same as production.
- **Password policy.** The 12-character minimum still holds —
  the hardcoded defaults (`admin-admin-admin`,
  `alice-alice-alice`, `bob-bob-bob-bob`) deliberately satisfy
  the production policy so the seed itself never has to
  bypass the check.
- **`redirect_uri` exact match** at the authorize endpoint.
- **Migration set / schema version.** Same migrations run, so
  a dev-mode SQLite file is a normal sui-id DB.

### Invariants relaxed (with operator-visible warnings)

- `cookie_secure = false` (no HTTPS in dev).
- `hibp_mode = off` (no outbound HIBP requests; offline-friendly).
- Login lockout relaxed via `max_lockout = 0` (no lockout from
  password attempts during testing).
- Defaults to in-memory SQLite (data evaporates on shutdown).

Each is mentioned in the startup banner.

### Hybrid seed model

Three sources of dev-mode seed data, in priority order
(highest first):

1. **TOML file** via `--dev-seed PATH`. Full schema:
   `[admin]` block, zero-or-more `[[user]]` blocks (with
   optional `preferred_lang`), zero-or-more `[[client]]`
   blocks (with `public = true` for PKCE-only clients,
   `client_secret = "…"` for confidential clients with
   predictable secrets).
2. **CLI flag overrides**: `--dev-admin-password STR` and
   `--dev-client-secret STR`. Apply on top of whatever was
   resolved from the TOML or the defaults.
3. **Hardcoded defaults**: admin / alice / bob (each with a
   `<name>-<name>-<name>`-style 12+ char password), one
   confidential test client with three localhost redirect
   URIs (`:3000`, `:5173`, `:8000`).

A TOML file with only `[admin]` falls back to the default
users and the default client. A TOML file with only `[[user]]`
blocks suppresses the default users and uses the supplied
ones. The "supply any → default suppressed for that section"
rule keeps cross-cutting overrides simple to reason about.

### Bind defaults to loopback; non-loopback requires confirmation

`--dev` listens on `127.0.0.1:8801` by default. Operators who
need the IdP reachable from another host (Docker container, LAN
demo, CI box) pass `--dev-bind 0.0.0.0:8801` or similar. The
non-loopback case **requires** an explicit `yes` typed on stdin
before sui-id binds — printed warnings alone are not enough,
because `sui-id --dev --dev-bind 0.0.0.0` from a shell-history
search is too easy to fire accidentally and would expose
plaintext seed credentials to whatever LAN the host is on.

### Added

#### `crates/sui-id/src/dev_mode.rs`

- `DevSeed` / `DevAdmin` / `DevUser` / `DevClient` structs
  representing the in-memory seed plan.
- `DevSeed::default()` — the hardcoded 3-user / 1-client seed.
- `DevSeedToml` (private) + `load_seed_from_toml` — TOML
  schema with `[admin]`, `[[user]]`, `[[client]]` blocks. Any
  omitted section falls back to defaults.
- `DevFlagOverrides` + `apply_overrides()` for CLI flag
  overlay.
- `resolve_seed(seed_path, overrides) -> (DevSeed, source)`
  — the orchestrator that returns both the resolved seed and
  a human-readable source description (used in the warning
  banner).
- `open_dev_db(Option<&Path>)` — fresh in-memory DB or a
  truncated-and-reopened pinned file under a freshly
  generated master key.
- `apply_seed(db, clock, setup_token, &seed) -> SeedOutcome`
  — runs `setup::create_initial_admin`, `admin::create_user`
  per user, `admin::create_client` per client. For
  confidential clients with operator-supplied secrets,
  patches the `secret_hash` post-creation via the new
  `repos::clients::set_dev_secret_hash` helper.
- `print_dev_warnings(bind, source)` — stderr banner with
  the `DEV MODE` warning header.
- `print_seed_summary(seed, outcome, listen_addr)` — stderr
  per-credential summary printed after seeding succeeds.
- `confirm_external_bind(bind)` — stdin-based `yes`
  confirmation for non-loopback bind. Aborts with an error
  if the line is anything other than `yes`.

#### Storage

- `repos::clients::set_dev_secret_hash(db, id, Option<&str>)`
  — patches `secret_hash` on a client row. Used only by dev
  mode to give a confidential client a predictable secret.
  Not on any HTTP path.

#### CLI

- `sui-id --dev` flag handling in `main.rs`. Dispatches into
  `serve_dev` when present.
- `serve_dev` (~110 lines): flag parsing
  (`--dev-bind`, `--dev-db`, `--dev-seed`,
  `--dev-admin-password`, `--dev-client-secret`), loopback
  vs non-loopback bind detection, `confirm_external_bind`
  invocation, `Config::sample()` synthesis with bind /
  issuer patched in, `open_dev_db`, `apply_seed`, AppState
  construction, axum listener with graceful shutdown.
- Help text updated with the dev section and all dev-mode
  flags.

#### Examples

- `examples/dev-seed.toml` — annotated sample TOML
  illustrating every option (`[admin]`, multiple `[[user]]`,
  confidential and public `[[client]]` blocks). Included so
  developers can copy-edit-paste rather than reading the
  parser source.

### Tests

- 6 new lib unit tests in `dev_mode::tests`:
  - `defaults_are_sensible` (also asserts password policy)
  - `flag_overrides_apply_in_order`
  - `toml_partial_falls_back_to_defaults`
  - `toml_empty_users_uses_defaults`
  - `toml_full_replacement`
  - `toml_invalid_returns_error`
- 5 new e2e tests in `tests/e2e.rs`:
  - `dev_mode_default_seed_creates_admin_users_and_client`
  - `dev_mode_flag_overrides_apply_to_seed` (+ verifies
    overridden admin can actually sign in)
  - `dev_mode_toml_seed_replaces_defaults` (covers
    `public = true`, default users suppressed)
  - `dev_mode_pinned_db_truncates_existing_file`
  - `dev_mode_resolve_seed_applies_priority` (TOML →
    flag override layering)
- All existing e2e and lib tests continue to pass.

### Notes — what we deliberately did NOT do

- **Custom client_id.** sui-id assigns a UUID-shaped client_id
  at creation; dev mode prints that UUID in the seed summary
  and operators copy-paste it into RP configs. We chose this
  over leaking a "predictable id override" into the
  production `CreateClientSpec`.
- **Hot reseed.** A long-running `sui-id --dev` does not
  re-read the TOML file when it changes. Restart `sui-id` to
  reseed. The behaviour matches the rest of sui-id (config
  changes are picked up at startup).
- **Persistent dev installs.** `--dev-db PATH` is supported
  but each restart truncates the file under a fresh master
  key. There is no mode that preserves dev-mode state across
  restarts; if you want persistence, you want production
  mode, not dev mode.

## [0.27.0] - 2026-05-04

Threat-model document refresh. The previous `docs/threat-model.md`
predated the v0.20.x design overhaul and the v0.21–v0.26 security
features (step-up authentication, email flows, multilingual UI,
HIBP integration, idle-session timeout / concurrent-session cap,
master-key rotation). v0.27.0 replaces it with a from-scratch
rewrite that reflects every defence shipped through v0.26.0 and
introduces structure for three different reader profiles
(operators / developers, security auditors, enterprise adopters).

### Why a full rewrite

The previous document was structured around fourteen adversaries
in a flat list (A1–A14), each with its own defence narrative.
Patch-style updates to it would have produced something
increasingly hard to read: v0.21's step-up touches several
adversaries, v0.22's email features add new attack surfaces, and
so on. A clean rewrite lets the design philosophy that emerged
across v0.20–v0.26 — column-encryption AAD binding, master-key
separation, fail-open external dependencies, FIFO eviction,
post-incident rotation — show up as architectural invariants in
their own right.

### Structure

The new document is organised into five parts plus a reporting
section:

- **Part 1 — Foundations.** Scope, trust boundaries
  (six interfaces with the outside world), adversary taxonomy
  (N / C / L / L+K / B), asset taxonomy (six classes of
  secret).
- **Part 2 — Threat scenarios.** Twelve scenarios, each with a
  consistent shape: what the attacker is trying to do, how
  they would attempt it, the adversary class, the defences
  that block or bound the attack, and the residual risk that
  remains. Scenarios cover credential stuffing, token grinding,
  session hijack, account enumeration, CSRF, stolen DB,
  stolen DB + master key, phishing reset, hostile HIBP, SMTP
  credential leak, intercepted backups, and signing-key
  compromise.
- **Part 3 — Defensive properties.** Eight architectural
  invariants that hold across all features: master-key
  separation, AAD binding, refresh-token theft detection,
  audit-log hash chain, step-up freshness, user-enumeration
  neutrality, idle-timeout throttle, and the workspace-level
  `unsafe_code = forbid`.
- **Part 4 — Detailed concerns.** STRIDE-style breakdown
  (spoofing / tampering / repudiation / information disclosure
  / denial of service / elevation of privilege), an
  attack-tree fragment for "account takeover" mapping each
  leaf to a Part 2 scenario or Part 3 property, compliance
  hints for GDPR, SOC 2 type II / ISO 27001, NIST 800-63B,
  and OWASP ASVS V2, and a short auditor FAQ. Marked as
  "skippable for operators on first read".
- **Part 5 — Known limitations and future work.** Adversaries
  we don't plan for, limitations we intend to fix (with
  pointers into ROADMAP), and things explicitly out of scope.
- **Reporting security issues.** Channel for vulnerability
  reports, target acknowledgement window (48 hours),
  high-severity fix window (two weeks).

### Reader-profile separation

Three audiences read the document for different reasons. The
opening paragraph spells out which parts each profile should
read:

- Operators / developers: Parts 1–3, Part 4 optional.
- Security auditors: Parts 1–4 in order.
- Enterprise adopters: Parts 1, 4, 5 are highest density.

This keeps Part 4's STRIDE / attack-tree / compliance material
out of the operator's path while making it easy to find for the
auditors and enterprise reviewers who specifically come looking
for it.

### What's in the document that wasn't before

- **Architectural invariants** (Part 3) as a first-class
  section. Properties like AAD binding and the audit hash-chain
  formula now have names callable by reference from Part 2
  scenarios.
- **STRIDE breakdown** (4.1) — six categories, each pointing
  back into the Part 2 scenarios that defend against it.
- **Attack-tree fragment** (4.2) — the "account takeover"
  goal decomposed into 13 leaves, each annotated with the
  scenario or property that bounds it.
- **Compliance hints** (4.3) for GDPR, SOC 2 / ISO 27001,
  NIST 800-63B, OWASP ASVS V2.
- **Auditor FAQ** (4.4) — six common questions about RNG
  source, password hashing, TLS posture, session lifetime,
  MFA policy, and HSM integration.
- **Coverage of every feature shipped between v0.20.0 and
  v0.26.0**, in particular: setup wizard / email column
  (v0.20.x), step-up authentication (v0.21.x), email flows
  + SMTP credential storage (v0.22.0), multilingual UI
  (v0.23.0), Pwned Passwords integration (v0.24.0), idle
  timeout + concurrent cap (v0.25.0), master-key rotation
  (v0.26.0).

### Notes — what was removed from the previous document

- The flat A1–A14 adversary list was reorganised into the
  five adversary classes (N / C / L / L+K / B). Information
  is preserved; the classes are referenced from each Part 2
  scenario.
- The old "What to monitor" section's content moved into the
  audit / hash-chain discussion (3.4).
- The previous reporting section's content is preserved
  near-verbatim at the bottom of the new document.

No code changes; this is a documentation-only release. Build,
all 188 lib tests, and all e2e tests continue to pass without
modification.

## [0.26.0] - 2026-05-04

Master-key rotation CLI. Re-seal every encrypted column in the
database under a new 32-byte XChaCha20-Poly1305 master key, with
the old key file archived beside the new one. Runs offline.

### Why offline

Hot rotation (live re-key while the server is running) was
evaluated and rejected. The complexity ladder is steep: every
sealed read needs to fall back through the old key, every seal
needs to choose the new one, the cutover has to be globally
consistent, and partial-rotation crash-recovery requires its
own state machinery. Offline rotation gives the strongest
guarantee — every row is fully old-keyed or fully new-keyed at
any point an operator can observe — at the cost of a few
seconds of downtime once or twice in the lifetime of a
deployment, which is the right trade for an IdP.

### Atomicity

All re-seals run inside a single SQLite transaction. On any
error during the loop the transaction rolls back; the DB file
remains entirely under the old key, the old key file has not
yet been renamed (the rename happens AFTER COMMIT), and
re-running with the same arguments is a clean retry. There is
no half-rotated state to recover from.

### Old-key preservation

After the transaction commits, the old key file is renamed to
`<original_path>.bak.<RFC3339-Z timestamp>`:

- The old material is available for restoring from a pre-
  rotation DB backup. (The CLI does NOT take a DB backup
  itself — that's the operator's responsibility.)
- The file is moved out of the path the server reads on next
  startup, so the server picks up the new key without further
  configuration changes.
- The file stays in the same directory the operator already
  manages permissions on.

Old key files are not auto-deleted. The operator decides when
(or whether) to remove them.

### Two ways to source the new key

- `--generate-new-key` (default if neither flag given): the
  CLI generates a fresh 32-byte key from the OS RNG and writes
  it as base64 to the configured key file path.
- `--new-key PATH`: the operator has prepared a key file
  externally (HSM-backed workflow, key-server, etc) and points
  the CLI at it. The contents are validated as base64-encoded
  32 bytes and replace the configured key file.

Both paths flow through `sui_id_store::crypto::MasterKey`, so
the file format is identical regardless of source.

### Confirmation

The CLI prints a summary (DB path, key file path, new-key
source, what the rename will do) and waits for the operator to
type `yes`. Pass `--yes` / `-y` to skip the prompt for non-
interactive use; the summary is printed either way so the
operator's terminal scrollback always shows what was done.

### Added

#### Storage

- `Database::with_tx(|tx| ... )` — exclusive SQLite transaction
  helper. Commits on `Ok`, rolls back on `Err`. Used by master-
  key rotation; available for any other operation that wants
  the same all-or-nothing semantics.
- `repos::signing_keys::reseal_all`,
  `repos::refresh_tokens::reseal_all`,
  `repos::user_totp::reseal_all` (returns `(secrets, recovery)`),
  `repos::user_webauthn_credentials::reseal_all`,
  `repos::smtp_config::reseal_all`. Each iterates the table,
  decrypts under the old key, re-seals under the new key,
  preserves the AAD, and runs inside the caller's transaction
  (no commits).

#### Core

- `core::key_rotation::rotate_master_key(db, new_key) ->
  RotationReport`. The orchestrator. Opens the transaction,
  walks each table's `reseal_all`, returns counts.
- `core::key_rotation::reseal_one(old_key, new_key, sealed,
  aad)`. Pure helper exposed for unit testing.
- `core::key_rotation::RotationReport` with per-table counts
  and a `total()` convenience.

#### CLI

- `sui-id admin rotate-key [--generate-new-key | --new-key PATH]
  [--yes] [--config PATH]`. Help text updated to spell out the
  offline-only nature and the `.bak.<timestamp>` rename.
- The CLI also writes an audit row
  (`admin.master_key.rotated`) before exiting so the operator's
  trail captures the rotation event.

### Tests

- 4 new lib tests in `core::key_rotation`:
  - `reseal_one_round_trip`
  - `reseal_one_fails_with_wrong_old_key`
  - `reseal_one_with_wrong_aad_fails`
  - `rotation_report_total_sums_columns`
- 2 new e2e tests:
  - `rotation_reseal_succeeds_and_old_key_no_longer_decrypts` —
    rotation against a DB with a signing key and a sealed SMTP
    password; asserts old key fails to decrypt and new key
    succeeds.
  - `rotation_on_minimal_db_only_rekeys_signing_key` — pins
    the row count on a fresh install, so future migrations
    that add new sealed columns force this test (and the
    rotation entry list) to be updated.
- All existing e2e (130+) and lib tests (188) continue to pass.

### Notes — what's covered and what isn't

Sealed columns covered by rotation:

- `signing_keys.private_key_enc`
- `refresh_tokens.token_enc`
- `user_totp.secret_enc`
- `user_totp.recovery_codes_enc`
- `user_webauthn_credentials.passkey_enc`
- `smtp_config.password_enc`

Things NOT in scope for the rotation CLI (by design):

- `password_reset_tokens.token_hash` is a SHA-256 hash, not
  encrypted, so there is nothing to rotate.
- The server cannot know the original passphrase for an
  encrypted backup file, so rotation does not re-encrypt
  backup tarballs. Operators who keep encrypted backups beside
  the running deployment should refresh them after rotation.
- Hot/online rotation. See the "Why offline" section above.

## [0.25.0] - 2026-05-04

Idle session timeout and concurrent session cap. Two adjacent
self-hardening knobs over the existing UI-cookie session model:

1. **Idle session timeout**: a session that has not been
   presented for `idle_session_timeout_secs` becomes invalid.
   Bounds the post-compromise window of a stolen cookie when
   the legitimate user has stopped using it.
2. **Concurrent session cap**: a single user holds at most
   `max_concurrent_sessions` active sessions. New logins past
   the cap evict the oldest existing session (FIFO).

Both default to `0` (= disabled). They are opt-in features
that an operator turns on via the admin settings page.

### Why default-off

These two knobs trade convenience for safety in different
ratios for different deployments. A small admin team running
one tab in one browser benefits from neither — the idle-timeout
adds friction with no realistic threat model behind it. A
public sui-id with users spread across machines benefits from
both. Choosing a default that fits everyone is impossible, so
v0.25.0 ships the architecture and tools for opt-in enablement
rather than a guess at the right defaults.

### Why FIFO eviction (and not "refuse the new login")

When a user is at the cap and signs in again, two designs are
plausible:

- **FIFO**: revoke the oldest existing session, accept the new
  login. The user signing in now wins; the user (perhaps the
  same user) on a possibly-stolen old cookie loses.
- **LIFO**: refuse the new login with "you have too many
  sessions; sign out somewhere first". The status quo wins.

We chose FIFO because the typical "I'm at the cap" cause is
either (a) the user has a stale cookie on a device they don't
use any more, or (b) someone has stolen one of their cookies.
In both cases, the new sign-in is the *legitimate* presence and
the right action is to grant it; in case (b), evicting the
stolen cookie is a defence rather than a regression.

LIFO would be the right call only if "the cap was hit because
something is wrong" is the more common case — which would be
true if sui-id were aimed at machine-to-machine clients where
the count corresponds to known instances. It isn't; sui-id's
cap is per *human* user.

### Throttled write pattern

Idle-timeout requires a "most recent presentation" timestamp,
which means writing on every authenticated request. A naive
implementation produces one DB write per HTTP request — fine
for hobby load, wasteful for any real one. v0.25.0 throttles
these writes: the column gets updated only when the value is
more than 60 seconds old. A busy session generates roughly one
write per minute, and the idle-timeout granularity stays
meaningful (a few-minutes timeout still reflects actual usage).
The throttle constant is `core::session::LAST_USED_AT_THROTTLE_SECS`.

### Added

#### Migration 0018: schema for both knobs

- `sessions.last_used_at TEXT` — nullable. Existing rows from
  before the migration get NULL, and the application treats
  NULL as "as old as `created_at`" — under the new idle-timeout
  policy, pre-migration sessions get the same treatment as a
  brand-new session that has not yet been re-presented.
- `idx_sessions_user_active` — partial index on
  `(user_id, created_at) WHERE revoked_at IS NULL`. Backs both
  the per-user active-count query (cap enforcement) and the
  oldest-active query (FIFO eviction).
- `server_settings.idle_session_timeout_secs INTEGER NOT NULL
  DEFAULT 0 CHECK (>= 0)` — `0` means feature off.
  Application-validated in range `[0, 30 * 86400]` (30 days)
  so a fat-fingered config does not silently exceed the
  absolute `expires_at` ceiling.
- `server_settings.max_concurrent_sessions INTEGER NOT NULL
  DEFAULT 0 CHECK (>= 0)` — `0` means cap disabled.
  Application-validated in range `[0, 1000]`.
- `MAX_SCHEMA_VERSION` rolls to 18.

#### Storage

- `SessionRow.last_used_at: Option<DateTime<Utc>>`.
- `ServerSettingsRow.idle_session_timeout_secs / max_concurrent_sessions`.
- `repos::sessions::touch_last_used(db, id, at)` — backing the
  throttled per-request write.
- `repos::sessions::count_active_for_user(db, user_id, now)` —
  cap-enforcement count.
- `repos::sessions::oldest_active_for_user(db, user_id, now,
  limit)` — FIFO selection.
- `repos::server_settings::update_idle_session_timeout(db,
  secs, now)`.
- `repos::server_settings::update_max_concurrent_sessions(db,
  cap, now)`.

#### Core

- `core::session::resolve` now also checks
  `now - last_used_at(or created_at) > idle_session_timeout_secs`
  when the timeout is non-zero. Past the window: revoke the row
  in-place (best-effort) and return `Unauthenticated`. Falls
  back to `created_at` when `last_used_at` is `NULL` — the
  conservative choice for pre-migration sessions.
- `core::session::touch_last_used(db, clock, id)` — public
  function called from each authenticated extractor. Throttled:
  a session whose `last_used_at` is newer than
  `LAST_USED_AT_THROTTLE_SECS` (= 60) is not re-written.
- `core::session::enforce_concurrent_session_cap(db, clock,
  user_id)` (`pub(crate)`) — best-effort post-insert eviction.
  Called from `login`, `mfa::verify_pending` (TOTP and recovery
  paths), and the WebAuthn-finishing variant. Failures here do
  not block the user from signing in; at worst the cap is
  briefly exceeded until the next login.
- `core::time::MockClock` — fixed-time `Clock` for deterministic
  tests of time-sensitive policies.

#### HTTP

- `CurrentUser`, `SessionContext`, and `CurrentAdminJson`
  extractors now call `session::touch_last_used` after
  `session::resolve` succeeds. `CurrentAdmin` is composed on
  top of `CurrentUser`, so it inherits the touch.
- `settings::idle_timeout_post` (`POST
  /admin/settings/security/idle-timeout`) — application bounds
  `[0, 2_592_000]`.
- `settings::max_sessions_post` (`POST
  /admin/settings/security/max-sessions`) — application bounds
  `[0, 1_000]`.

#### Web

- `SettingsSecurityData` gains `idle_session_timeout_secs`,
  `max_concurrent_sessions`, and `csrf_token`.
- `render_settings_security` adds a bilingual "セッション制限 /
  Session limits" section with two number-input forms and
  inline help text spelling out the disabled-on-zero semantics
  and the FIFO behaviour at cap-exceed.

### Tests

- 8 new lib unit tests in `core::session::session_limit_tests`:
  - `resolve_passes_when_idle_timeout_disabled`
  - `resolve_revokes_after_idle_window`
  - `resolve_passes_within_idle_window`
  - `resolve_treats_null_last_used_at_as_created_at`
  - `touch_last_used_throttles_within_window`
  - `touch_last_used_writes_when_stale`
  - `enforce_cap_does_nothing_when_cap_zero`
  - `enforce_cap_evicts_oldest_in_fifo_order`
- 6 new e2e tests:
  - `session_no_idle_timeout_when_disabled`
  - `session_idle_timeout_revokes_after_window`
  - `session_cap_evicts_oldest_in_fifo`
  - `session_cap_disabled_keeps_all_sessions`
  - `admin_settings_security_idle_timeout_change`
  - `admin_settings_security_max_sessions_change`
- All existing e2e (120+) and lib tests (184) continue to pass.

### Notes — interaction with step-up freshness

The v0.21.x step-up freshness check (`is_fresh`) is independent
of the v0.25.0 idle timeout. A session can be inside its idle
window (recently presented) but outside its step-up freshness
window, and vice versa. Both checks have to pass for a
sensitive action to proceed. This is intentional: idle timeout
is "have you been here recently", step-up freshness is "have
you re-proven a strong factor recently".

### Notes — last_used_at and OAuth refresh tokens

The idle timeout applies only to UI cookie sessions (the
`sessions` table). OAuth refresh tokens have their own lifetime
machinery (`refresh_tokens.expires_at`, rotation, theft
detection) and are not retroactively gated by idle timeout
here. A refresh-token client that has been silent for an hour
keeps working as long as the token itself is still valid.

## [0.24.0] - 2026-05-04

Pwned Passwords (HIBP) breach check. Adds an optional pre-acceptance
check that asks the public Pwned Passwords API whether a candidate
password has shown up in known data breaches. Used at the setup-
wizard admin-creation step in this release; other entry points
(self-service password change, admin reset, forgot-password
redemption) get the same treatment in v0.24.x patches per the
ROADMAP "HIBP scope expansion" entry.

### Why this design

#### k-anonymity, not "send the hash"

The naive "send the password and ask" is obviously wrong. Even
sending the SHA-1 hash is dangerous: SHA-1 is small enough that
HIBP could (and would) be a plaintext oracle for popular
passwords.

The Pwned Passwords API is built around k-anonymity: the client
SHA-1s the candidate, sends only the first 5 hex characters of
the hash (the *prefix*), receives a list of matching `<35-char
suffix>:<count>` pairs (hundreds per prefix), and looks up its
own suffix locally. sui-id never sends the password, never sends
the full hash, and never logs the SHA-1 anywhere. The only thing
on the wire is the 5-character prefix, shared by tens of thousands
of distinct passwords.

We also send `Add-Padding: true` so HIBP pads responses to a
uniform size, defending against traffic-analysis attacks that
infer the queried prefix from response length.

#### Three operational modes

Operators run sui-id everywhere from "fully internet-connected
production" to "air-gapped". A single hard-coded behaviour can't
fit all of these, so the check has three modes stored in the
singleton `server_settings` row:

- `'off'`: no check, no outbound request. The right setting for
  offline / air-gapped deployments and policies that ban
  third-party API calls.
- `'warn'`: check is performed; a hit is logged at `warn` level
  but the password is accepted. Default at install time —
  operators get visibility without the flow breaking on the
  first encounter with a flaky external API.
- `'block'`: a hit refuses the password with a friendly error.

#### Fail-open

When the HIBP request itself fails (timeout, DNS, TLS, 5xx), the
policy is to *let the password through* regardless of mode
(including `block`). The audit trail records the failure so an
operator can investigate, but a flaky external service must not
be allowed to lock an admin out of password operations. This
trade-off is documented in migration 0017's comment and pinned
by the test
`setup_wizard_fails_open_when_hibp_unavailable_in_block_mode`.

#### Why `ureq` rather than `reqwest`

We chose the synchronous `ureq` over the async `reqwest` for
three reasons:

- Cold path. The call site is one GET per password set, and at
  v0.24.0 only at setup-wizard time. The benefit of async would
  be drowned in the size and complexity it brings.
- Smaller dependency surface. `ureq`'s rustls integration matches
  the `wasm-smtp-tokio` rustls already in our tree without
  pulling tokio's networking stack a second time.
- Readability. The `ureq` request is short and direct; an async
  equivalent would need `tokio::task::spawn_blocking` either way
  to keep the axum runtime free, so async would buy us nothing
  but more code.

The `ureq` call is wrapped in `tokio::task::spawn_blocking` at
the HTTP-handler call site so the runtime is not stalled on
network I/O.

### Added

#### Migration 0017: `server_settings.hibp_mode`

`ALTER TABLE server_settings ADD COLUMN hibp_mode TEXT NOT NULL
DEFAULT 'warn' CHECK (hibp_mode IN ('off', 'warn', 'block'))`.
The CHECK is enumerated (unlike `users.preferred_lang`) because
the set is fixed by the application; rejecting a stale value at
the DB layer keeps a typo in a hand-edited DB from silently
treating "of" as "off". `MAX_SCHEMA_VERSION` rolls to 17.

#### Crate `sui-id-core`

- `models::HibpMode` enum (`Off | Warn | Block`) with `as_str()`,
  `parse()`, `Default = Warn`.
- `core::hibp` module:
  - `HibpClient` trait — object-safe, single `check(&self, password)
    -> HibpCheckOutcome`. Sync method; the impl is responsible for
    converting any I/O / parse failure into `Unavailable` rather
    than propagating errors.
  - `HttpHibpClient` — production impl using `ureq`, talks to
    `api.pwnedpasswords.com/range/{prefix}` with `Add-Padding`,
    5-second connect/read timeouts, `User-Agent: sui-id/{version}`.
    SHA-1 hex is `zeroize`d before return.
  - `parse_response(body, suffix)` — pure parser, exposed for
    unit tests. Case-insensitive suffix match, count == 0 lines
    (Add-Padding markers) treated as not-breached.
  - `enforce_hibp(mode, client, password)` — pure policy
    function returning `HibpEnforcement::{Allowed,
    AllowedWithWarning { count }, Blocked { count }}`. Short-
    circuits on `Off` (no client call) and on `Unavailable`
    (fail-open, both Warn and Block).
  - `enforce_hibp_or_reject(...)` — convenience wrapper that
    converts `Blocked` into `CoreError::BadRequest`.
- `core::hibp::test_support::InMemoryHibpClient` (gated on
  `feature = "test-support"`) — deterministic stub for tests.
  `set_breached(pw, count)` and `set_unavailable(pw)` plan its
  responses; unconfigured passwords return `NotBreached`.

#### Storage

- `repos::server_settings::update_hibp_mode(db, mode, now)`.
- `ServerSettingsRow.hibp_mode: HibpMode`.

#### HTTP

- `AppState.hibp_client: Arc<dyn HibpClient>`. Production
  constructs `HttpHibpClient`; tests inject `InMemoryHibpClient`.
  The field is unconditional even when `hibp_mode = off` — the
  cost is one Arc clone, and keeping the field unconditional
  avoids a mode-checked match at every dispatch site.
- `AppState::new(... mailer, hibp_client)` gains the new param.
- `setup::admin_post` performs the HIBP check between the
  password-mismatch check and the `setup::create_initial_admin`
  call. Block-mode rejection re-renders the form with a
  Japanese flash message; Warn-mode hits log at `warn` level;
  Off-mode short-circuits before the HTTP request.

### Tests

- 12 new lib unit tests in `core::hibp` covering parse-response
  (match, no match, padding, case-insensitivity), policy
  (Off skips, Warn allows-with-warning, Block rejects, Allowed
  on clean, fail-open on Unavailable in both Warn and Block),
  and the `enforce_hibp_or_reject` rejection shape.
- 6 new e2e tests covering each path through the setup wizard:
  - `setup_wizard_accepts_clean_password_in_warn_mode`
  - `setup_wizard_accepts_breached_password_in_warn_mode`
  - `setup_wizard_rejects_breached_password_in_block_mode`
  - `setup_wizard_accepts_clean_password_in_block_mode`
  - `setup_wizard_off_mode_skips_check`
  - `setup_wizard_fails_open_when_hibp_unavailable_in_block_mode`
- All existing e2e (120+) and lib tests (176) continue to pass.

### Notes — admin UI for the setting

There is no admin UI for `hibp_mode` at v0.24.0; the field is
edited via direct DB write or via the `repos::server_settings::update_hibp_mode`
API in `core`. An admin-settings UI for HIBP, alongside the
existing Email tab, lands in v0.24.x as part of the scope
expansion entry.

### Notes — what about caching

We deliberately do *not* cache HIBP responses. The API responses
are k-anonymity sets that update as new breaches are catalogued,
and even the SHA-1 hex of the first 5 chars is sensitive enough
that we'd rather not persist anything HIBP-related to disk. The
production call rate is low (one GET per password set) so caching
is not a performance need.

## [0.23.0] - 2026-05-03

Multilingual support v1. Adds Japanese ↔ English UI selection
across the resolution chain user-preference → cookie →
Accept-Language → server default, plus the typed i18n foundation
(`sui-id-i18n` crate, `Locale` enum, `Strings` struct) on which
the rest of the UI translation work will land.

### Why a typed Strings struct rather than Fluent / external files

Three considerations drove the design:

- **Compile-time completeness.** Every translation is a field on
  the `Strings` struct, and every locale is a `static Strings`
  constant. Adding a locale means adding a variant to `Locale`
  and a new `STRINGS_*` constant — the exhaustive `match` in
  `Locale::strings` then guarantees no field is forgotten.
  Adding a string means adding a field — every per-locale
  constant fails to compile until populated. There is no
  runtime "key missing" failure mode.
- **No new dependencies for the foundation tier.** Fluent or
  ICU MessageFormat would buy plural-form rules and complex
  bidi at the cost of pulling a substantial dependency. For the
  enumeration-style messages we have today, per-locale functions
  are simpler and faster, and the architecture leaves room to
  upgrade individual messages to Fluent later if real plural-form
  requirements appear.
- **Translator workflow.** The Strings struct is a single Rust
  file per locale, friendly to standard diff tooling, and easy
  to PR. External translators get a single file to edit.

### Locale resolution chain

The four-tier chain in `core::i18n::resolve`:

1. `users.preferred_lang` — explicit per-user setting from
   `/admin/profile`. Wins because the user actively chose it.
2. Cookie `sui_id_lang` — per-browser override. Lets a
   signed-out user pick a language and lets a signed-in user
   override on a different machine without changing the
   per-user setting.
3. `Accept-Language` header — browser default. q-weights are
   intentionally ignored (first recognised tag wins).
4. `server_settings.default_lang` — admin-configured fallback
   editable at `/admin/settings/basic`.

Final fallback is `Locale::Ja` in code, reachable only if the
singleton settings row has been tampered with.

### Added

#### Crate `sui-id-i18n` (new)

- `Locale::{Ja, En}` enum with `tag()` (BCP-47),
  `native_name()`, `parse()` (region-suffix-tolerant), `strings()`,
  `ALL` slice.
- `Strings` struct — ~150 fields covering generic UI, navigation,
  login, setup wizard, step-up, forgot password, settings hub,
  profile, password change, email subjects/bodies, errors,
  audit-log labels.
- Static constants `STRINGS_JA`, `STRINGS_EN` with full translations.
- `negotiate_from_accept_language(header)`.
- 6 unit tests (parse round-trip, region suffix tolerance, unknown
  returns None, accept-language negotiation, every locale populated,
  native names in their own script).

#### Migration 0016: schema for the user and server tiers

- `users.preferred_lang TEXT NULL` — application-validated, no
  CHECK constraint so locale additions do not require a fresh
  migration.
- `server_settings` table — singleton row keyed on `'singleton'`,
  modeled on `smtp_config`. Today only `default_lang`; future
  process-wide knobs can extend the row without a new migration.
  Default row INSERTed by the migration.
- `MAX_SCHEMA_VERSION` rolls to 16.

#### Core

- `core/src/i18n.rs` — `LocaleInputs` struct,
  `resolve(db, inputs)` walking the four-tier chain, `Locale`
  re-exported. 5 new unit tests pinning the priority order, the
  cookie-over-header rule, the accept-language fallback, the
  server-default fallback, and the unknown-tag fallthrough.
- `forgot_password::request_reset` — reset-link mail composed
  in the recipient's preferred locale (user.preferred_lang →
  server default → Ja). Subject and body come from
  `Strings.email_*`.
- `forgot_password::notify_password_changed` gains a
  `locale: Locale` parameter; callers resolve from
  `user.preferred_lang` and the server-settings fallback before
  calling.

#### Storage

- `users::set_preferred_lang(db, id, lang, now)` — pass `None`
  for "clear".
- `repos::server_settings` — `get`, `update_default_lang`.
- `UserRow.preferred_lang: Option<String>` — nullable, no
  CHECK constraint.
- `ServerSettingsRow` model.

#### HTTP

- `RequestLocale(Locale)` extractor — pulls the cookie jar,
  best-effort resolves a session id to a user_id (no expiry
  check; the locale path is read-only and a stale id is
  harmless), walks `core::i18n::resolve`. Never fails.
- `LANG_COOKIE` constant (`"sui_id_lang"`).
- `admin::profile_lang_post` — `POST /admin/profile/lang`.
  Validates tag against `Locale::ALL`, calls
  `users::set_preferred_lang`, sets/clears cookie
  (`SameSite=Lax`, secure-flag from config, max-age 1 year for
  set / 0 for clear), redirects to `/admin/profile`.
  Empty tag clears; unknown tag returns 400.
- `settings::basic_lang_post` — `POST /admin/settings/basic/lang`.
  Validates tag, writes `server_settings.default_lang`.
- `login_get` / `login_post` extract `RequestLocale` and pass
  it into `render_login`.

#### Web

- `Shell` and `AuthShell` gain optional `lang: Option<Locale>`
  prop; rendered `<html lang>` reflects it. Existing callers
  unchanged.
- `render_login(flash, next, lang)` — first page fully
  translated, every label/hint/helper from `Strings`. Remaining
  pages get the same treatment in v0.23.x patches; see the
  ROADMAP "i18n scope expansion" entry.
- `render_profile` gains a "表示言語 / Display language"
  section with a `Browser default | 日本語 | English` select.
  Bilingual UI on this picker so a user accidentally on the
  wrong language can still recognise their own.
  `ProfileData.preferred_lang` carries the current setting.
- `render_settings_basic` gains a "サーバーデフォルト言語 /
  Server default language" form section.
  `SettingsBasicData.default_lang` and `csrf_token`.

### Tests

- 6 new lib tests in `sui-id-i18n`.
- 5 new lib tests in `core::i18n`.
- 8 new e2e tests:
  - `login_page_html_lang_defaults_to_ja`
  - `login_page_html_lang_follows_accept_language`
  - `lang_cookie_overrides_accept_language`
  - `profile_lang_post_persists_and_sets_cookie`
  - `profile_lang_clear_resets_to_browser_default`
  - `profile_lang_post_rejects_unknown_tag`
  - `admin_settings_basic_default_lang_change`
  - `forgot_password_email_in_user_preferred_locale`
- All existing e2e (110+) and lib tests (164) continue to pass.

### Notes

- The login page is fully translated. Forgot-password,
  reset-password, profile and settings pages render with their
  Japanese strings unchanged at this release; full per-page i18n
  is queued for v0.23.x patches. The plumbing — `RequestLocale`
  extractor, `Strings` struct, `<html lang>` on every page — is
  in place, so each per-page conversion is a mechanical pass over
  view! literals.
- `users.preferred_lang` has no CHECK constraint by design.
  Locale additions should not require a migration; application-
  layer validation is the single source of truth.
- Email locale resolution is decoupled from HTTP locale resolution.
  A signed-in user requesting a password reset for themselves
  will see the form in one locale (HTTP chain) and receive the
  email in another (recipient chain) if their cookie/header
  differ from `users.preferred_lang`. Intentional — the email
  follows the recipient, the form follows the browser session.

## [0.22.0] - 2026-05-02

Email features. Two user-facing flows land:

- **Forgot-password / reset**: `/forgot-password` form, token-link
  email, `/reset-password?token=…` form, single-use 30-minute
  tokens whose plaintext never touches the database.
- **Password-change notification**: any password change (via
  `/me/security/password` or `/reset-password`) sends a
  notification email when the user has an email on file.

Plus the operator-facing settings UI to configure SMTP, with a
`Test Connection` button that runs a real EHLO/STARTTLS/AUTH
dance against the configured relay without sending a message.

### Why these design choices

#### SMTP config in the database, not `sui-id.toml`

- Operators can change settings without restarting (matters when
  troubleshooting delivery).
- The settings page can offer a `Test Connection` button — debug
  value of this is large, mail delivery is one of the hardest
  things to debug after-the-fact.
- Credentials sit alongside our other encrypted columns
  (XChaCha20-Poly1305 sealing via the master key). TOML would
  either store the password plaintext or push the operator into
  env-var juggling.
- Setting changes feed the audit chain naturally
  (`auth.smtp_config.changed`).

#### Inline send + audit-log on failure

The forgot-password handler awaits the SMTP send; failures
record an `auth.password.reset_email_failed` event and the
exterior response stays neutral (200 + same acknowledgement
page) so the endpoint cannot be a user-enumeration oracle.

We don't queue. The volume is tiny, the latency is acceptable,
and the simplest implementation that survives a process crash
is "do it inline, log if it fails". A persistent outbox + worker
is a clear future enhancement (ROADMAP) but not necessary at
this scope.

#### Email opt-in

When `smtp_config` has no row or the row's `enabled = 0`:

- `/forgot-password` and `/reset-password` return 404.
- `/me/security/password` continues to work; the post-change
  notification is skipped silently.

The forgot-password endpoint does not exist when SMTP isn't
configured. Better than rendering a form that silently no-ops.

### Added

#### Schema

- **migration 0014 (`smtp_config`)**: singleton row keyed
  `'singleton'`. `tls_mode` is `'implicit'` (port-465 TLS-from-the-
  start) or `'starttls'` (port-587 plaintext-then-upgrade); we
  do not expose a plain-text mode. `password_enc` is
  XChaCha20-Poly1305 sealed with AAD `b"smtp.password"`.
- **migration 0015 (`password_reset_tokens`)**: stores SHA-256
  hashes only; plaintext tokens never touch the database.
  Single-use, 30-minute TTL, unique index on `token_hash`.
- `MAX_SCHEMA_VERSION` rolls to 15.

#### Models

- `SmtpTlsMode { Implicit, StartTls }` with `as_str()` / `parse()`.
- `SmtpConfigRow`, `PasswordResetTokenRow`.
- `PasswordResetTokenId` typed id in `sui-id-shared`.

#### Repos

- `repos::smtp_config::{get, upsert, decrypt_password, seal_password,
  SMTP_PASSWORD_AAD}`.
- `repos::password_reset_tokens::{insert, find_by_hash,
  mark_consumed, delete_expired, count_active_for_user}`.
- `repos::users::find_by_email`, `find_by_id_opt`.

#### Core

- `core::mail::{MailSender, OutgoingMail, MailSendOutcome,
  SmtpMailSender, InMemoryMailSender}`.
  - `SmtpMailSender` reads the live `smtp_config` row on every
    send so config changes apply without restart.
  - `SmtpMailSender::test_connection()` runs the protocol dance
    without sending a message, suitable for the admin UI test
    button.
- `core::forgot_password::{request_reset, validate_token,
  consume_and_reset_password, notify_password_changed,
  DEFAULT_TOKEN_TTL}`.
  - 32 random bytes via `OsRng` → URL-safe base64 → SHA-256 hash.
  - `request_reset` always returns `Ok(())` externally; internal
    outcomes (no match, throttled, mail failed, etc) are only
    visible in the audit log.
  - `MAX_OUTSTANDING_TOKENS_PER_USER = 3` ceiling per user
    prevents inbox spam.
- `events::SecurityEvent` gains five reset variants:
  `PasswordResetRequested`, `PasswordResetThrottled`,
  `PasswordResetEmailSent`, `PasswordResetEmailFailed`,
  `PasswordResetCompleted`.

#### HTTP

- `handlers::forgot_password`: 4 endpoints
  (`forgot_password_get` / `_post` / `reset_password_get` /
  `_post`). All return 404 when SMTP isn't enabled.
- `handlers::settings::{email_get, email_post, email_test}`.
- `RateLimitKey::ForgotPassword` + `Limiters.forgot_password`.
- `HttpError::not_found_html()` for feature-gated routes.
- `AppState.mailer: Arc<dyn MailSender>` threaded through
  `AppState::new` — production constructs `SmtpMailSender`,
  tests `InMemoryMailSender`.

#### Web

- `pages::render_forgot_password` / `render_forgot_password_sent` /
  `render_reset_password` / `render_reset_password_invalid` /
  `render_settings_email`.
- `SettingsTab::Email` variant; the settings hub now has 6 tabs.
- The admin login page gains a "パスワードをお忘れですか?" link
  pointed at `/forgot-password` (only useful when SMTP is
  enabled, but the link is unconditional — pointing at a 404 is
  better than dynamically hiding the link based on whether
  someone happens to need it).

#### Password-change handler integration

`me_security::password_change_post` now invokes
`forgot_password::notify_password_changed` inline (awaited)
when the user has an email on file. We initially drafted this
as `tokio::spawn` ("fire and forget") so the user's redirect
isn't gated on the SMTP relay's latency; we switched to inline
because the test path needs deterministic ordering and because
the production SMTP timeout is small (single-digit seconds),
which is acceptable for a self-service action that already
holds a database write. Notification failures `tracing::warn!`
but do not roll the password change back.

### Tests

- 10 new e2e tests:
  - `forgot_password_get_404_when_smtp_disabled`
  - `forgot_password_get_renders_form_when_smtp_enabled`
  - `forgot_password_post_neutral_response_for_unknown_email`
  - `forgot_password_post_sends_mail_for_known_email`
  - `reset_password_get_invalid_for_unknown_token`
  - `reset_password_full_flow_changes_password_and_sends_notification`
  - `settings_email_get_renders_for_admin`
  - `settings_email_get_requires_admin`
  - `password_change_sends_notification_mail_when_email_is_set`
  - `password_change_sends_no_mail_when_email_is_unset`
- All existing e2e (100+) continue to pass without changes.
- All lib tests (153) continue to pass.

### Notes — wasm-smtp v0.9.3 integration

We use the `wasm-smtp` core crate with the `mail-builder`
feature, the `wasm-smtp-tokio` adapter for our axum runtime, and
`mail-builder` 0.4 for RFC 5322 + MIME composition.

The crate's design surfaced several decisions worth highlighting
because they removed work for sui-id:

- TLS modes are surfaced as two distinct connect functions
  (`connect_implicit_tls`, `connect_starttls`) — type-level
  separation, not a string discriminator. We mirrored that in
  our own `SmtpTlsMode`.
- `mail-builder` produces `multipart/alternative` automatically
  when both `text_body` and `html_body` are set, so our reset
  email gets HTML and plain-text variants for free.
- `ProtocolError::UnexpectedCode { actual, enhanced }` exposes
  the SMTP reply code and parsed RFC 3463 enhanced status code,
  so transient (4xx) vs permanent (5xx) classification is
  one-line.
- Cargo features use `compile_error!` to make crypto-provider /
  trust-store choices mutually exclusive — wrong combinations
  fail at build time, not runtime.
- `StartTlsBufferResidue` as an explicit error variant
  protects us against the RFC 3207 §5 buffer-injection class
  of attack without us implementing the check ourselves.

A short list of suggestions we'd send back upstream:

- **Capture the SMTP relay's "queue" string from the 250 OK
  reply**. Many relays (Postfix, Sendmail) include a
  `queued as XXX` token; surfacing it in the send return value
  would let consumers correlate sui-id's audit log with the
  relay's logs.
- **`tracing` feature** that emits `trace!`/`debug!` at major
  protocol transitions (EHLO, STARTTLS, AUTH, MAIL FROM, …)
  would make debugging delivery problems much easier than
  reading-error-or-nothing today.
- **DKIM signing hook** — a closure/trait taking the serialized
  RFC 5322 message before it goes on the wire, allowing a
  consumer to inject a `DKIM-Signature` header via a separate
  crate (e.g. `mail-auth`) without forking message construction.
- **`IoError::is_timeout()` / `is_connection_refused()`
  helpers** so retry logic doesn't have to walk `source` chains.

These are wishes rather than complaints — the existing API is
clean and the integration was straightforward.

## [0.21.1] - 2026-05-02

WebAuthn step-up. Users with passkeys but no TOTP enrolled can
now satisfy a step-up gate by completing a WebAuthn assertion
ceremony, the same kind they use to sign in. The TOTP / recovery-
code path landed in v0.21.0; this release closes the gap for
passkey-only accounts.

### Why

The v0.21.0 step-up gate calls `step_up::verify_totp_code`,
which requires TOTP enrolment. A user whose only second factor
is a passkey would hit `InvalidCredentials` and have no path
forward — short of registering a TOTP solely to clear the gate.
That defeats the security model: the user's passkey is at least
as strong as the TOTP they don't have, so it should satisfy the
gate too.

### Added

#### `core::step_up::start_webauthn` / `finish_webauthn`

Thin wrappers around the existing `webauthn::start_authentication`
/ `finish_authentication` that already work as pure functions.
Differences:
- The pending row is tagged `WebauthnPendingKind::StepUp` so a
  step-up ceremony can never be misused as a login-MFA
  verification (and vice versa) even if a pending_id leaked
  across contexts.
- On success, `touch_step_up` is called; no new session is
  minted.
- `finish_webauthn` refuses pending rows whose `kind` isn't
  `StepUp`, and refuses rows whose `user_id` doesn't match the
  signed-in user — the wrong-shape pending row is left intact
  in either case so the legitimate flow that owns it can still
  complete.

#### Migration 0013: `webauthn_pending.kind` widened

The `kind` CHECK constraint on `webauthn_pending` originally
allowed `'register'` and `'authenticate'`. Migration 0013
rebuilds the table with `'step_up'` added. SQLite doesn't
support direct CHECK alteration, so the rebuild dance: copy
into a new table, drop the old, rename. Pending rows are
short-lived (5-minute TTL) so any in-flight ceremony at
migration time is no worse than a one-time retry.
`MAX_SCHEMA_VERSION` rolls to 13.

#### `WebauthnPendingKind::StepUp` enum variant

Round-trips through the existing `as_str` / `parse` helpers as
the wire string `"step_up"`.

#### HTTP endpoints

- `POST /me/security/step-up/webauthn/start` — verify CSRF,
  begin the ceremony, set the pending-id in the
  `sui_id_step_up_webauthn_pending` HTTP-only cookie, return
  the challenge JSON.
- `POST /me/security/step-up/webauthn/finish` — verify CSRF,
  read the pending-id from the cookie, run
  `step_up::finish_webauthn`. On success, clear the pending
  cookie and 303 to `return_to`. On failure, clear the
  pending cookie and 400 with a JSON error so the JS can
  surface the message without navigating.

The pending-id cookie is HTTP-only, SameSite=Lax, `Secure`
when configured, 5-minute max-age. Choosing a cookie over
returning the id in the response body ensures the browser
can't be tricked into binding a different ceremony's id to
the finish call.

#### `/static/step-up-webauthn.js`

The browser-side script that drives the ceremony. Identical
shape to `webauthn.js`'s authentication flow — base64url
encode/decode helpers, navigator.credentials.get(), POST to
finish. Loaded only on `/me/security/step-up` and only when
the user has a passkey enrolled.

#### `render_step_up` gains a `has_passkey` parameter

When the user has at least one registered passkey, the
challenge page renders an additional "パスキーで再認証"
section below a divider, with a button and the WebAuthn JS.
When they don't, the section is omitted entirely so the JS
asset isn't pulled in unnecessarily.

#### Tests

3 new core unit tests covering the negative paths that protect
the step-up flow from cross-context misuse:

- `finish_webauthn_refuses_pending_with_wrong_kind` — an
  `Authenticate` pending row never satisfies a step-up finish,
  and refusing it does not consume the row.
- `finish_webauthn_refuses_pending_for_other_user` — even a
  `StepUp` pending must belong to the requesting user.

5 new e2e tests:

- `step_up_form_shows_passkey_section_for_users_with_passkey`
- `step_up_form_omits_passkey_section_for_users_without_passkey`
- `step_up_webauthn_start_requires_csrf`
- `step_up_webauthn_finish_without_pending_cookie_fails`
- `step_up_webauthn_start_for_user_without_passkey_returns_bad_request`

Full WebAuthn end-to-end flow (a real browser completing the
ceremony) is not covered by tower-oneshot tests because
webauthn-rs verifies a real cryptographic signature that needs
a real authenticator. Manual verification with a hardware key
or platform passkey covers that path.

### Notes

- The kind-swap-then-delegate trick in `finish_webauthn` (re-
  writing the pending row from `StepUp` to `Authenticate`
  before calling `webauthn::finish_authentication`, which kind-
  checks against `Authenticate`) is deliberate: the
  alternative would be duplicating the entire finish-authentication
  body, including the credential-counter update logic, which
  is exactly the kind of code we don't want to maintain twice.
  The swap is internal and never observable from outside the
  module.
- WebAuthn credentials counter is updated by
  `webauthn::finish_authentication` as part of the assertion,
  so a successful step-up cycles the counter the same way a
  login MFA cycle would.
- The login MFA WebAuthn flow (kind = `Authenticate`) is
  unchanged. Existing e2e and runtime behaviour for sign-in
  with passkey is unaffected.

## [0.21.0] - 2026-05-02

Step-up authentication. Sensitive admin and self-service actions
now gate on a fresh strong-factor proof: an MFA-enrolled user
who hasn't completed a TOTP / recovery-code challenge in the
last 5 minutes is bounced through `/me/security/step-up` before
the action proceeds. The schema groundwork for this landed in
v0.17.0 (migration 0010, `sessions.last_step_up_at`); v0.21.0
wires the gate into the HTTP layer and the affected handlers.

### What "step-up" means here

A regular session is fine for routine reads (the dashboard,
profile, audit log). It is *not*, on its own, enough to authorise
a destructive or security-relevant action — rotate a signing key,
delete a user, force-reset another user's MFA, sign every other
browser out at once. The threat model is **session theft**:
someone who has stolen a cookie can navigate the UI, but
should not be able to immediately ratchet that into permanent
damage.

The freshness window is intentionally short (5 minutes,
`STEP_UP_FRESHNESS_SECS`). Long enough to absorb a sequence of
related admin actions in one cleanup pass, short enough that a
session stolen after the last step-up will be re-challenged.

### Added

#### `core::step_up::verify_totp_code`

Verify a TOTP code (or single-use recovery code) entered into a
step-up form by an already-signed-in user. Distinct from
`mfa::verify_pending`:
- Does **not** create a new session (the user already has one).
- On success, calls `touch_step_up` to bump the session's
  `last_step_up_at`.
- On failure, returns `CoreError::InvalidCredentials` for both
  "wrong code" and "user has no MFA enrolled" — a step-up form
  must look the same to a typo as to a probe.
- Recovery codes are accepted on the same shape as the login
  flow, single-use; same `mfa::consume_recovery_code` helper
  (now `pub(crate)`).

3 unit tests cover correct code, wrong code (no session touch),
and no-MFA user (same error shape).

#### `handlers::SessionContext` extractor

Like `CurrentUser` but also carries `session_id`. The step-up
gate needs to read the session row for `last_step_up_at`, so
handlers that participate in step-up extract this instead of
`CurrentUser`.

#### `handlers::require_fresh_step_up(app, ctx, return_to)`

The gate. Returns `Ok(())` when the session is fresh (or the
user has no MFA enrolled, in which case step-up is a no-op),
or `Err(Response)` carrying a 303 redirect to the challenge
page. Idiom:

```rust
if let Err(redirect) = require_fresh_step_up(&app, &ctx, "/admin/users") {
    return Ok(redirect);
}
// ...do the sensitive thing...
```

`return_to` is URL-encoded into the challenge URL so the user is
bounced back to the original action after a successful prove.

#### `/me/security/step-up` HTTP endpoint

- `GET /me/security/step-up?return_to=<path>` renders the
  challenge form (TOTP / recovery code input) inside an
  `AuthShell`, narrow column, focused.
- `POST /me/security/step-up` accepts the code, calls
  `step_up::verify_totp_code`, and on success 303-redirects
  to the sanitised `return_to`.

#### `return_to` validation (`sanitise_return_to`)

A defensive whitelist on the redirect target. Refuses:
- empty / non-`/`-leading paths;
- `//` (protocol-relative URLs);
- `\\`;
- backslash, line break, or NUL anywhere;
collapsing any of the above to `/me/security`. Off-site
redirects after a successful prove are the worst possible
outcome — better an undisputed safe default than a clever
attempt at preserving the user's intent.

### Changed — handlers gated on fresh step-up

The handlers chosen are the irreversible / system-wide-impact
ones. Reversible operations (disable user, disable client) and
operations whose primary credential gate is the user's own
password (password change) are deliberately not gated, to
avoid stacking proofs without a meaningful security gain.

| Handler | Why gated |
|---|---|
| `me_security::revoke_all_others` | Signs every other browser out at once; significant blast radius even though revocations aren't "irreversible", recovery cost on other devices is high. |
| `admin::users_delete` | Irreversible. |
| `admin::users_mfa_reset` | Strips another user's MFA factors; high-trust operation. |
| `admin::clients_delete` | Irreversible; in-flight tokens for the client also revoked. |
| `admin::signing_keys_rotate` | System-wide impact (changes which key signs new tokens). |
| `admin::signing_keys_delete` | Tokens already issued under the deleted key fail to verify. |

### Behaviour for no-MFA admins

An admin who has never enrolled MFA (TOTP or WebAuthn) sees
no change: `policy_for_session` returns `Allow` for them, and
the gate is a no-op. This is deliberate — a step-up challenge
for a password-only account would be a password re-prompt,
which buys nothing against an attacker who already has the
cookie *and* the password. The `admin_with_no_mfa_passes_step_up_gate_transparently`
e2e test pins this behaviour against future refactors.

### Tests

- 3 new core unit tests (`verify_totp_code_*`) bringing the
  step_up module to 8 tests total.
- 7 new e2e tests covering the gate, the form, the `return_to`
  sanitiser (3 cases: friendly path, off-site, protocol-
  relative), the verify happy path, the wrong-code failure
  shape, the gate firing on a sensitive action with stale
  step-up, and the no-MFA pass-through.
- All existing e2e (90+) continue to pass without changes
  because they run with the no-MFA test admin.

### Notes

- WebAuthn-based step-up is **not** in this release. TOTP /
  recovery code is enough to gate sensitive actions for any
  account that has TOTP enrolled. WebAuthn step-up requires
  lifting the assertion-flow code out of `webauthn.rs` into a
  shared helper; that's a follow-up.
- The freshness window (5 minutes) is currently a constant
  in `step_up::STEP_UP_FRESHNESS_SECS`. If we ever expose it
  to operators it'll go through `Config.security` so the
  setting survives a restart.
- A "remember this device for 30 days" flow is *not* on the
  table. The freshness model is intentionally per-session, not
  per-device; cross-device persistence would need a per-device
  trust token, an "untrust this device" UI, and a binding
  policy — three independent features the simple model can be
  extended into later if a need lands.

## [0.20.4] - 2026-05-02

3-step setup wizard at `/setup` → `/setup/admin` → `/setup/done`.
The single-page setup form is split into a welcome screen, the
admin-creation form (with email and password-confirmation fields),
and a completion screen. The wizard adds an optional `email`
column to the user schema for the admin and for any user created
through `/admin/users`.

The design memo also describes a fourth screen ("encryption
settings"). sui-id deliberately omits it — the rationale lives
below under [Setup wizard: encryption screen omission](#setup-wizard-encryption-screen-omission).

### Added

#### 3-step wizard

- `GET /setup` → 画面 1 (welcome). Static intro page with a single
  "セットアップを始める" button to step 2. Redirects to
  `/admin/login` when the system is already initialized.
- `GET /setup/admin` → 画面 2 (admin form). Fields: setup token,
  username, optional email, optional display name, password,
  password confirmation. Redirects to `/admin/login` when already
  initialized.
- `POST /setup/admin` → consumes the form, creates the admin and
  the first signing key, marks the system initialized,
  auto-logs the operator in (so the post-setup dashboard link
  lands authenticated), and 303-redirects to `/setup/done`.
- `GET /setup/done` → 画面 4 (completion). Shows success +
  "next steps" hint + "管理画面へ進む" button. If a curious
  operator visits before finishing step 2 (uninitialized system),
  the page renders a "セットアップは完了していません" notice
  with a link back to `/setup`.

A 3-dot step indicator (1 ようこそ / 2 管理者作成 / 3 完了)
renders at the top of every wizard screen, with badges showing
which step is active, complete, or upcoming. No JavaScript: the
indicator is static structural HTML driven by the active step.

#### `email` column on users (migration 0012)

- `ALTER TABLE users ADD COLUMN email TEXT` (nullable).
- `CREATE UNIQUE INDEX idx_users_email ON users(email) WHERE
  email IS NOT NULL` — partial-unique so multiple NULL emails
  don't conflict, but a set email is unique across the table.
  `MAX_SCHEMA_VERSION` rolls to 12.
- `UserRow.email: Option<String>`. `users::create` /
  `find_by_username` / `list` round-trip the column.
- `CreateUserSpec.email: Option<&str>` for `admin::create_user`.
- `setup::create_initial_admin` takes a new `email: Option<&str>`
  parameter.
- `/admin/users` create form now has an "メールアドレス(任意)"
  field; `CreateUserForm.email` is parsed and passed through.

#### Form validation

- `POST /setup/admin` rejects mismatched password / confirmation
  with HTTP 400 and a friendly Japanese flash before consuming
  the setup token. Mismatch is checked at the form layer rather
  than against the password policy, so the user sees a clear
  "一致しません" message instead of a generic policy error.
- Empty email or display name are normalised to `None` rather
  than the empty string so the partial-unique index never
  conflicts on `""`.

#### E2E tests (8)

- `setup_welcome_renders_when_uninitialized`
- `setup_welcome_redirects_when_initialized`
- `setup_admin_form_renders_with_email_and_confirm`
- `setup_admin_post_creates_admin_with_email_and_redirects_to_done`
- `setup_admin_post_rejects_mismatched_confirm`
- `setup_done_renders_after_initialization`
- `setup_done_says_not_yet_when_uninitialized`
- `admin_users_create_form_accepts_email`

### Changed

- `complete_setup_and_login` test helper now POSTs to
  `/setup/admin` with the new `email` and `confirm_password`
  fields. All existing e2e tests using this helper continue to
  work unchanged.
- `MaxLockoutDuration::label()` continues from v0.20.3.
- The legacy single-page `render_setup` view function is gone;
  `render_setup_welcome`, `render_setup_admin`, and
  `render_setup_done` replace it.

### Setup wizard: encryption screen omission

The design memo's screen 3 asks the operator to "configure
encryption" during setup. sui-id does not surface this in the
UI on purpose:

- The master key is **resolved before the HTTP server starts**.
  `SUI_ID_MASTER_KEY` (env) wins; otherwise `storage.key_file`
  is read, auto-generating a 32-byte file with `0600` perms on
  first run.
- By the time `/setup` renders, the database is already encrypted
  and the key is already loaded. There is nothing for a "set the
  encryption key" UI to do.
- Adding one would either be cosmetic (key already decided) or
  dangerous (a process that holds the master key advertising an
  interface to manipulate it).

This is documented in `docs/operators.md` under
"Why there is no 'encryption' step in the wizard" so future
operators don't go looking for the missing step.

If a master-key rotation feature lands in a later release, it
will be a `sui-id admin` CLI command operating against the
database file offline, *not* a UI on a running process.

## [0.20.3] - 2026-05-02

Settings hub at `/admin/settings/*` — five tabs surfacing the
current effective configuration as a read-only overview, plus
deep links into the existing detail pages where the operator can
actually act on something.

### Added

#### Five settings tabs

Each tab is its own route so the URL is the source of truth for
which tab is active — refresh, bookmark, and back-button all
work without JavaScript, and a server-side flash redirect cleanly
preserves the active tab.

- **`/admin/settings`** → 303 to `/admin/settings/basic`
- **`/admin/settings/basic`** — Issuer URL, listen address,
  cookie Secure flag, trusted_proxies CIDR list, links to
  Discovery and JWKS endpoints
- **`/admin/settings/security`** — max-lockout duration label,
  HSTS / CSP / X-Frame-Options DENY / Permissions-Policy status
  badges, CORS policy summary (token endpoint dynamic from
  registered redirect_uris, public OIDC endpoints open)
- **`/admin/settings/authentication`** — password policy
  (min length, Argon2id), TOTP / WebAuthn enablement, 8 recovery
  codes per enrollment, PKCE-required flag, per-token lifetimes
  (access / id_token / refresh), refresh rotation + theft
  detection flags
- **`/admin/settings/logs`** — current log format and filter
  expression, last-24h counts for `auth.login.success` /
  `auth.login.failure` / `auth.login.locked` /
  `auth.password.changed_self`, audit-chain tail-verify status
  with `badge--ok` (正常) or `badge--danger` (破損検知). Deep
  link to `/admin/audit` for full history
- **`/admin/settings/other`** — sui-id binary version, supported
  schema version (`MAX_SCHEMA_VERSION`), DB file path, master
  key file path, server clock now, user/client counts with
  inline links to the manage pages

#### Tab strip

A 5-element `.app-nav__link` strip renders at the top of every
settings page. The active tab gets `aria-current="page"`, picking
up the same accent-pill treatment the main nav uses. The strip
wraps on narrow viewports.

#### Settings handler module

- New `handlers::settings` with one `*_get` per tab plus
  `index_redirect`. Each handler reads `Config` and / or the DB
  and produces the per-tab view data.
- `MaxLockoutDuration::label()` added to surface the human
  string the operator chose (e.g. "12h"), matching the form they
  would write in `sui-id.toml`.

#### Logs tab counters

Reuses `audit::count_by_action_in_window` from v0.20.2 with a
single 24-hour bucket — same query path the dashboard sparkline
uses, so the index added in v0.20.2 keeps these counters fast.

### Changed

- Admin nav: `Settings` link added between Audit and Profile.
  The link points at `/admin/settings`, which redirects to
  `/admin/settings/basic` so the active-tab pill always lights
  up cleanly.

### Tests

7 new e2e tests — one per tab plus the redirect and an
admin-required check:

- `settings_index_redirects_to_basic`
- `settings_basic_renders_for_admin`
- `settings_security_renders_lockout_and_headers`
- `settings_authentication_renders_lifetimes`
- `settings_logs_renders_with_24h_counts_and_chain_status`
- `settings_other_renders_versions_and_paths`
- `settings_pages_require_admin`

Lib totals unchanged (no logic added to the lib layer that needed
direct unit coverage; the read-only handlers consume existing
APIs).

### Notes

- All values are read-only. Mutating settings goes through the
  existing dedicated admin pages (`/admin/users`,
  `/admin/clients`, `/admin/signing-keys`, `/admin/profile`) or
  by editing `sui-id.toml` and restarting. The settings hub deep
  links to those where applicable.
- Adding an editable item to a settings tab in the future does
  *not* require restructuring the page — drop a `<form>` into the
  appropriate `.card` and wire a `*_post` handler. The current
  visual structure already accommodates that.

## [0.20.2] - 2026-05-01

Dashboard sparkline. The admin dashboard now shows the
distribution of sign-in attempts over the recent past — both
successes and failures, stacked, with a hover tooltip per
bucket. The operator can switch between a 24-hour, 7-day, or
30-day view at the top of the chart. The implementation needs
no JavaScript: the sparkline is inline SVG, the tooltips are
native `<title>` elements, the range tabs are anchors with
`?range=` query params.

### Added

#### Audit-log time-window query

- `audit::count_by_action_in_window(actions, since, until,
  bucket_minutes)` returns rows of `(bucket_start, action,
  count)` for any list of actions and any bucket size, grouping
  by Unix-epoch-aligned bucket boundaries. The alignment is
  important: two requests landing at 09:00 and 17:00 produce
  buckets at the same wall-clock moments, so the dashboard
  doesn't shift visibly each time it's reopened.
- `ActionCountBucket { bucket_start, action, count }` carries
  one such row.

#### Migration 0011: composite index

- `idx_audit_log_at_action ON audit_log (at, action)`. The
  `at`-leading order matches every dashboard query (range scan
  on `at`, then refine on `action` via `IN`) and covers the
  existing `audit::recent` / `recent_for_user` queries that
  only filter on time. `MAX_SCHEMA_VERSION` rolls to 11.

#### `sui-id-core::dashboard`

- `SparklineRange::{Last24Hours, Last7Days, Last30Days}` with
  `as_query` / `from_query` for the URL round-trip,
  `bucket_minutes` / `bucket_count` so handlers don't have to
  remember which range pairs with which bucket size, and a
  Japanese label per range. Default is `Last7Days`.
- `LoginActivity { range, buckets, total_success, total_failure }`
  — the dense bucket array (zero-filled missing buckets) plus
  range totals.
- `LoginActivityBucket { bucket_start, success, failure }` for
  one column of the chart.
- `login_activity(db, clock, range)` does the heavy lifting:
  asks the audit-log query for the right window, fills the
  dense array on the same Unix-epoch grid the SQL used, and
  computes totals.
- 6 unit tests covering: empty database, bucket alignment, rows
  in window are counted into the right bucket, rows outside
  window are ignored, unrelated actions are never counted,
  range query strings round-trip.

#### Sparkline view (in `sui-id-web`)

- `DashboardSparkline { active_range_query, range_options,
  total_success, total_failure, buckets }` — the data shape the
  view consumes.
- `DashboardSparkBucket { label, success, failure }` — one
  pre-formatted bucket. The handler decides the label format
  (1-hour buckets get `YYYY-MM-DD HH:MM`, day buckets get
  `YYYY-MM-DD`) so the renderer is bucket-size agnostic.
- Inline SVG sparkline, `viewBox="0 0 200 60"`, stacked bars
  per bucket. Failures sit on the bottom (so a streak of red
  is visible regardless of the success count above it),
  successes stack on top. Each bar carries a transparent
  full-height hover-target rect so the tooltip fires even
  when both counts are zero. Per-bucket `<title>` element
  delivers the tooltip natively — no JS, no CSP relaxation.
- The "成功" / "失敗" totals appear next to the chart, coloured
  to match the bars (`var(--accent-default)` and
  `var(--danger-default)` respectively).

#### Dashboard handler & range tabs

- `dashboard(...)` now takes `Query<DashboardQuery>` and reads
  `?range=24h|7d|30d`. Anything else (or absent) falls back to
  `SparklineRange::default()` — no 400 for a typo, just the
  default view.
- Range tabs render as `.app-nav__link` anchors at the top of
  the sparkline section, with `aria-current="page"` on the
  active one.

#### E2E tests (3)

- `dashboard_sparkline_renders_with_default_range`: GET
  `/admin` shows the sparkline section, default-range label,
  the SVG with its aria-label, and tooltip-formatted bucket
  text.
- `dashboard_sparkline_honours_explicit_range_query`: each of
  `?range=24h|7d|30d` produces a 200 with the matching anchor
  href in the response body.
- `dashboard_sparkline_falls_back_to_default_on_garbage_range`:
  `?range=banana` returns 200 with the default range, not 400.

### Notes

- The sparkline uses CSS variables for colours (`--accent-default`,
  `--danger-default`) so it picks up dark-mode automatically
  through the existing `[data-theme]` cascade.
- Range persistence is currently URL-only. A future revision can
  layer a localStorage-backed default on top by reusing the same
  early-inline-script pattern the theme toggle already uses.

## [0.20.1] - 2026-04-30

Per-screen design pass for the **non-core** pages. This is a
visual-only release: no handler logic, no storage schema, no
authentication or authorization changes. Every page now uses
the same component vocabulary (page-header, card, .field,
.table-wrap, badge, flash) that the core path picked up in
v0.20.0, and the Japanese localisation extended uniformly to
the rest of the admin surface.

### Changed — page rebuilds

- **`render_setup`** — moved to `AuthShell` + `.auth-card` so it
  shares the centred narrow layout with login. Field hints
  ("起動ログに 1 度だけ出力された値" / "12 文字以上") added.
  Heading is now "sui-id へようこそ".
- **`render_mfa_challenge`** — `AuthShell` + `.field`. Passkey
  block lives below a `.divider`. Headings and labels in
  Japanese.
- **`render_profile`** — full rebuild: `page-header`, two
  `<section>`s (TOTP and passkeys), each managing state via
  `.card` + `.card__footer`. TOTP status shown as a `badge--ok`
  / `badge--warn` rather than prose. Passkey table is in a
  `.table-wrap`. New passkey registration is a `.card`-wrapped
  form with `.field` + `.field__hint`.
- **`render_mfa_setup`** — three `.card`s in sequence (手順 / QR
  と秘密鍵 / 確認), each scoped to one task. The QR `<svg>` keeps
  its inline `max-width:240px` since it's a one-off raster size.
- **`render_client_edit`** — `page-header` + two `.card`s. The
  immutable "基本情報" card surfaces Client ID, type, and status
  with badges; the "設定" card holds the form. Each `.field`
  has a `.field__hint` explaining what the operator can put in
  it.
- **`render_audit`** — `page-header` + `.table-wrap`. The
  `result` column shows a `badge--ok` for `ok`, `badge--danger`
  for `fail` / `error` / `denied`, and a neutral `.badge` for
  anything else. Row count is surfaced in the lede ("直近 N 件").
- **`render_signing_keys`** — `page-header` + a "キーローテー
  ション" `.card` with the rotate button in `card__footer`, plus
  a `.table-wrap` of all keys. Status uses `badge--ok` for
  active, neutral `.badge` for retired.
- **`render_error`** — moved to `AuthShell`. The error message is
  the `.flash.error` banner; the request id sits in a `.muted`
  paragraph below; and the recovery link is a `.button.secondary`.
- **`render_me_security`** — `page-header` + three `<section>`s
  (二段階認証, サインイン中の場所, 最近のアクティビティ). 2FA
  state collapses to a single `badge--ok` line ("認証アプリ /
  パスキー N 件") when on, or a `.flash.warn` when off. Recent
  activity rows render their `result` as a badge.
- **`render_password_change`** — `page-header` + a single `.card`
  containing the form. `.field` + `.field__hint` for each input.
  Submit and Cancel sit in a `.row`.

### Changed — copy

All headings, section titles, button labels, form labels, field
hints, and confirmation dialogs on the rebuilt pages translate
to Japanese, matching the screen-design memo and the v0.20.0
core-path treatment. Technical strings (Client ID, Key ID, JWT,
hashes) stay in Latin and live inside `.code` for monospace
legibility. Operator-facing audit verbs (`Revoke`, `ok`, `fail`,
`denied`) stay in Latin since they are also the wire-protocol
strings recorded in the audit log.

### Changed — tests

Three e2e tests had to follow the copy:

- `me_security_page_renders_for_authenticated_user` — section
  headings updated to "アカウントセキュリティ", "サインイン中の
  場所", "最近のアクティビティ".
- `mfa_enroll_then_login_with_totp_succeeds` — the secret-key
  extraction logic now anchors on the Japanese label "秘密鍵:"
  and skips past the inline-styled `<span class="code"
  style="...">` to read the secret. The previous "Secret key:
  <span class="code">" needle no longer matches.
- `me_password_change_form_renders` — substring match changed to
  "パスワードを変更".

No other tests required changes — the rest match on form `name=`
attributes, CSS class names, and structural HTML, all of which
are stable across the design pass.

### Items deferred to v0.20.2+

- Dashboard sparkline ("過去 7 日間のサインイン数") — the next
  visible-feature pass on the dashboard. Needs a time-window
  count over `audit_log` (e.g. `audit::count_by_action_in_window`)
  bucketed per day, then a small inline SVG sparkline rendered
  next to the existing stat cards.
- Settings page tabbed structure (基本 / セキュリティ / 認証 /
  ログ / その他). Currently the operator-facing settings live in
  a flat list; v0.20.3 reorganises into the five tabs the
  screen-design memo asks for.
- Setup multi-step wizard (ようこそ → 管理者作成 → 暗号化設定 →
  完了). The setup token + admin creation are still a single page
  in this release; v0.20.4 splits them into the four-step flow
  per screens 1–4 of the design figure.
- Authorize / consent screen visual rework, per screen 11. The
  page is functionally complete but still uses the v0.19.0
  layout. v0.20.5 brings it on to the new components.
- Step-up auth (v0.21.0). The schema groundwork (migration 0010,
  `sessions.last_step_up_at`) is already in place; v0.21.0
  rebuilds the core logic from scratch and wires it into
  sensitive actions (password change, bulk revoke, signing-key
  rotation, etc.).

## [0.20.0] - 2026-04-29

Design language overhaul. The Lavender-Jade palette, an 8/16/24/32
spacing rhythm on a 4px base, a 5-step typography scale, and a
proper component vocabulary land together. Light and dark themes
ship as first-class citizens, with a footer toggle that remembers
the user's choice across pages without a Cookie round-trip. The
core path of the UI — login, the admin nav and shell, the
dashboard, the user list, and the client list — is rebuilt on top
of the new components. Every other screen still works (no
behavioural change) and inherits the new tokens automatically;
those screens get their first pass in v0.20.1.

### Added

#### Design tokens (`sui-id-web::tokens`)

A single CSS file's worth of `:root` variables defines the
palette and metric scales. Every component reads these — there
are no more raw hex codes anywhere in the component sheet. The
tokens are organised as:

- **Surface**: `--surface-default`, `--surface-elevated`,
  `--surface-subtle`, `--surface-sunken`, `--surface-inverse`.
  Three z-level steps so cards visibly sit on the page background
  without a heavy shadow.
- **Foreground**: `--fg-default` (≈14:1 on default), `--fg-muted`
  (≈5:1), `--fg-subtle` (≈3:1), `--fg-on-accent`, `--fg-inverse`.
  All four contrast pairs hit AA or better in both modes per the
  contrast pairings document.
- **Accent**: lavender `--accent-default` (#7C6BCF light /
  #A89BFF dark), `--accent-emphasis` for hover, `--accent-subtle`
  as a safe text-bearing background.
- **Semantic**: `--danger-default`, `--warning-default`,
  `--success-default` (jade-influenced), `--info-default` —
  separately tuned per mode.
- **Interaction**: `--state-hover`, `--state-active`,
  `--state-focus`, `--state-disabled`. The focus token doubles as
  the global focus-ring colour, applied via `:focus-visible` for
  keyboard users only.
- **Spacing**: `--space-1` (4px) through `--space-6` (48px), a
  4px-based rhythm. The page header→section→card→field cascade
  uses 32 / 24 / 16 / 8 with the dominance the design memo asks
  for.
- **Typography**: 28 / 22 / 18 / 15 / 13 px display / h2 / h3 /
  body / caption, with line-heights tuned per size (1.3 → 1.6).
  Weights regular 400 / medium 500 / bold 700. Numbers default
  to tabular-nums in stat callouts.
- **Radius**: 6 / 10 / 16 px for sm / md / lg.
- **Shadow**: three steps tuned per mode (light uses subtle black
  alphas, dark uses heavier alphas to read on near-black surfaces).

#### Component sheet (`sui-id-web::components`)

A single CSS that defines every primitive in terms of tokens —
the visual language of the product, in 400 lines. Components:

- `.app-header`, `.app-nav`, `.app-nav__link`, `.app-footer`
- `.app-main`, `.app-main--narrow`, `.auth-page`, `.auth-card`
- `.card`, `.card__title`, `.card__body`, `.card__footer`
- `.stack`, `.stack-tight`, `.row`, `.grid-cards`
- `.stat`, `.stat__value`, `.stat__label`
- `.field`, `.field__label`, `.field__hint`
- Inputs, selects, textareas, checkboxes — focus ring + hover +
  disabled all consistent
- Buttons: primary (filled accent), secondary (outlined), danger,
  ghost, link-button. Min height 36px for touch.
- `.table-wrap` + `<table>` styling — uppercase caption headers,
  alternating-row hover, rounded outer container
- `.badge`, `.badge--ok`, `.badge--warn`, `.badge--danger`,
  `.badge--accent`
- `.flash` info / warn / error
- `.page-header`, `.page-header__title`, `.page-header__lede`,
  `.page-header__actions`
- `.theme-toggle`, `.theme-toggle__btn` for the footer toggle
- `.sr-only` for screen-reader-only content

#### Light / dark theme switching

- The Lavender-Jade dark palette activates either via
  `[data-theme="dark"]` on `<html>` (explicit user choice) or via
  `prefers-color-scheme: dark` (when the user hasn't chosen).
- A footer toggle lets the user pick **Light / Auto / Dark**. The
  choice is persisted in `localStorage` under `sui_id_theme`
  (values: `"light"` / `"dark"` / `"system"`).
- An early inline script in `<head>` reads `localStorage`
  *synchronously* before first paint and sets `data-theme`. There
  is no FOUC: the page paints in the chosen theme on the first
  frame.
- The `aria-pressed` state on the three toggle buttons reflects
  the active choice.
- No Cookie round-trip — the SSR HTML is theme-neutral, the
  inline script sets the theme client-side. This keeps page
  caching trivial and the server stateless.

#### Multilingual font strategy

The font stack is system-ui with explicit CJK fallbacks. No web
fonts are bundled — there is **zero increase in distributed
binary size** from typography. The browser's Unicode font
fallback handles each script with the OS-native UI font: SF Pro
on Apple, Segoe UI on Windows, Hiragino Sans / Yu Gothic UI for
Japanese, Noto Sans CJK on Linux/Android. When v1 multilingual
support adds `<html lang="...">` to localised pages, `:lang()`
rules can pin per-script fonts on top — no asset additions
required.

`.code` / `.mono` / `<code>` / `<pre>` use `ui-monospace` with
SF Mono / Cascadia Code / JetBrains Mono / Consolas / Menlo
fallbacks — important for technical IDs (Client ID, UUID, JWT)
where `0` vs `O` and `l` vs `1` legibility matters.

#### Footer chrome

The footer carries:

- The product tagline ("🌱 sui-id · 静かで、凛として、やさしい
  ID 基盤を。")
- Three accessibility badges (Keyboard / Screen reader / Contrast)
  per the screen-design figure
- The theme toggle
- The version string from `CARGO_PKG_VERSION` so a glance at the
  footer always reveals what's deployed

### Changed

- `Shell` component (used by every authenticated admin page)
  rebuilt on the new tokens and component classes. No layout
  prop API changes — existing call sites work unchanged.
- New `AuthShell` component for centred narrow layouts (login).
  The setup page will move onto `AuthShell` in v0.20.0's
  follow-up rework of the setup wizard (v0.20.x or later).
- **`render_login`** rebuilt on `AuthShell`, `.auth-card`, and
  `.field`. Japanese copy ("ユーザー名またはメールアドレス" /
  "パスワード" / "ログイン") matching the screen-design figure.
- **`render_dashboard`** rebuilt — `page-header` + three stat
  cards (users / clients / service status with a status badge) +
  a dedicated OIDC endpoints table. Removed the
  five-row "everything in one table" layout.
- **`render_users`** rebuilt — `page-header`, a `card`-wrapped
  "新しいユーザーを追加" form using `.field`, a `.table-wrap`-
  wrapped table with status badges (active / admin / disabled /
  deleted) and MFA badge. Action buttons grouped in a `.row`.
- **`render_clients`** rebuilt — same treatment as users. The
  "Save this client secret now" warning uses the new `.flash.warn`
  with a `.stack-tight`. Status badges replace plain text
  status.
- Admin nav links use `.app-nav__link` with hover + `aria-current`
  pill styling. The Sign out link auto-pushes to the right.

### Documentation

- Visual design memo: token names + scales, light/dark contrast
  pairings, font strategy, focus-ring policy. Lives next to
  `tokens.rs` for now; will graduate to `docs/design-system.md`
  alongside v0.20.1's screen sweep.

### Items deferred to v0.20.1+

- Per-screen rebuild for the **non-core** pages: `setup`,
  `signing-keys`, `audit`, `me/security`, `me/security/password`,
  `authorize` (consent), `mfa-challenge`, `mfa-setup`, `profile`,
  `client-edit`. They render correctly today and inherit the new
  tokens for colour and typography automatically; they just don't
  yet use the new card/badge/page-header vocabulary.
- Dashboard sparkline ("過去 7 日間のサインイン数"), per the
  screen-design figure. Needs an audit-log query over a time
  window — small but separable, comes with the v0.20.x dashboard
  pass.
- Settings tabbed structure (基本 / セキュリティ / 認証 / ログ /
  その他), per screen 9 of the design figure. Comes with the
  v0.20.x settings pass.
- Setup multi-step wizard (4 steps: ようこそ → 管理者作成 →
  暗号化設定 → 完了), per screens 1–4 of the design figure. The
  setup token + admin creation are currently a single-page form;
  the wizard split is a UX upgrade, not a security one. Comes
  with the v0.20.x setup pass.
- Authorize / consent screen visual rework, per screen 11 of the
  design figure. Functionally complete today (consent is
  obtained at `/oauth2/authorize`); the visual rework lands in
  the v0.20.x consent pass.

## [0.19.0] - 2026-04-28

Self-service password change at `/me/security/password`. A
signed-in user can change their own password, optionally sweeping
every other session and every active refresh token in one step.
The current session stays alive so the user isn't ejected from
the form they just submitted.

### Added

#### `/me/security/password` page

Reachable via the "Change password" button on `/me/security`.
Form fields:

- Current password.
- New password (12–256 characters, no composition rules per
  NIST SP 800-63B).
- Confirm new password.
- "Sign out my other browsers and apps after changing the
  password" — checkbox, checked by default.

On submit:

1. CSRF check.
2. Rate limit against the shared `Login` bucket (IP-keyed).
   Even a session-holder shouldn't be able to grind the
   current-password field at unbounded rate from a stolen
   cookie.
3. New / confirm match check.
4. Verify current password against the stored Argon2id hash.
   Wrong current password → `InvalidCredentials`. **No account
   lockout** is applied on this path: the user is already
   authenticated by their session, and locking yourself out
   by mistyping the form would be unhelpful. The order —
   verify-current then policy-check-new — is deliberate, so the
   endpoint doesn't become an oracle for "is X actually a
   password?" via differentiated errors.
5. Policy check on the new password.
6. Hash, upsert credential row. The `must_change` flag is
   cleared if it was set (admin-driven reset is now satisfied).
7. If the box was checked: revoke every other session for this
   user, and **every** active refresh token. The current
   session is preserved.
8. Append an `auth.password.changed_self` audit event with
   sweep counts.

#### `core::me_security::change_password_self`

The action lives in `sui_id_core::me_security`. Returns a
`PasswordChangeReport { sessions_revoked, refresh_tokens_revoked }`
so future callers (or a future REST API) can render counts —
the current HTML handler doesn't surface them, the audit row is
the durable record.

#### Tests

5 new unit tests in `sui-id-core::me_security`:

- `happy_path_replaces_hash_and_returns_zero_sweep_when_box_unchecked`
- `wrong_current_password_is_rejected_as_invalid_credentials`
- `weak_new_password_is_rejected_after_current_is_verified`
- `must_change_flag_is_reset_on_self_change`
- `audit_event_is_appended`

5 new e2e tests in `sui-id`:

- `me_password_change_form_renders`
- `me_password_change_happy_path_replaces_password`
- `me_password_change_wrong_current_is_refused`
- `me_password_change_mismatched_confirm_is_refused`
- `me_password_change_with_revoke_others_sweeps_other_sessions_and_refresh_tokens`

Workspace lib totals: shared 13, store 15, core 59 (+5), sui-id
47 — **134** lib tests, all passing.

### Changed

- `/me/security` page now has a "Change password" button next to
  "Manage authenticators" in the two-factor section.

### Documentation

- `docs/operators.md` — new "Self-service password change" and
  "Things `/me/security/password` deliberately does **not** do"
  subsections under "Self-service security". The non-goals call
  out the missing notification email (lands when SMTP is added),
  no re-MFA prompt (step-up-auth is a separate pass), and no
  reuse policy / HIBP (separate v0.20+ pass).
- Audit event table: added `auth.password.changed_self`.

### Internal — schema groundwork for step-up auth

Schema migration 0010 introduces a nullable `last_step_up_at`
column on `sessions`, and `SessionRow` carries the field through
in repository code. The actual step-up logic — challenging the
user to re-prove a strong factor before sensitive actions — is
not yet wired and is deferred to v0.20.0. The column being
nullable means existing rows are unaffected and no behaviour
changes here. We register the column now so v0.20.0 doesn't have
to ship a schema migration alongside the new logic.

### Items deferred to v0.20.0+

- Confirmation email on password change (waits on SMTP support
  via `wasm-smtp v0.6`).
- Step-up auth: re-prompt for MFA before sensitive actions.
- Password-reuse policy and HIBP integration (opt-in).
- Recording IP and User-Agent on session creation.
- Idle session timeout, concurrent session cap, suspicious
  activity heuristics.
- Master-key rotation command.

## [0.18.0] - 2026-04-28

Self-service security overview at `/me/security`. Every signed-in
user — admin or not — gets a per-account view of their active
sessions and recent authentication events, plus the tools to
revoke individual sessions or sweep every session except the
current one.

### Added

#### `/me/security` page

A new authenticated route that does *not* require admin
privilege. Shows three sections:

- **Two-factor summary.** Whether TOTP is enrolled, how many
  passkeys are registered, with a button that deep-links to
  `/admin/profile` (the existing MFA enrollment page already
  worked for non-admin users; we just point at it).
- **Sessions table.** Every active session belonging to this
  user, newest-first. The session that issued the current
  request is labelled `current session`; every other row has a
  Revoke button. Below the table, "Sign out everywhere else"
  sweeps every session except the current one in a single click.
- **Recent activity.** Up to 30 audit rows where this user is
  either the actor or the target — covers
  `auth.login.success/failure/locked`,
  `auth.refresh.theft_detected` (when relevant),
  `mfa.admin_reset`, etc. The page tells the user plainly: if
  you see something you didn't do, rotate your password and
  sign out other sessions.

#### Ownership enforcement

Server-side ownership check on revoke: `revoke_one` looks up the
target session and refuses (silently — same redirect as for an
unknown id) if the session's `user_id` doesn't match the caller.
There is no oracle for guessing other users' session ids. The
e2e suite includes a regression test
(`me_security_cannot_revoke_someone_elses_session`) that pins
this.

#### Storage helpers

- `sessions::list_active_for_user(user_id)` — newest-first list
  of unrevoked, unexpired sessions for one user.
- `sessions::revoke_all_for_user_except(user_id, keep)` — bulk
  revoke matching the "Sign out everywhere else" semantic. The
  current session is determined from the cookie, not the form
  field, so a tampered hidden field cannot make the user revoke
  the "wrong" current session.
- `audit::recent_for_user(user_id, limit)` — newest-first audit
  rows where the user is either `actor` or `target`. Used to
  drive the activity table.

#### Routes

- `GET  /me/security`
- `POST /me/security/sessions/{id}/revoke`
- `POST /me/security/sessions/revoke-all-others`

CSRF tokens enforced on every POST. The bulk revoke emits a new
`auth.sessions.bulk_revoke_self` audit event recording how many
sessions were swept.

#### Tests

- 5 new e2e tests in `sui-id`:
  - `me_security_page_renders_for_authenticated_user`
  - `me_security_redirects_when_not_signed_in`
  - `me_security_revoke_one_signs_target_session_out`
  - `me_security_revoke_all_others_keeps_current_session`
  - `me_security_cannot_revoke_someone_elses_session`

Workspace lib totals unchanged from v0.17.0 (no logic added to
the lib layer that needed direct unit coverage). Lib still 129;
e2e suite +5.

### Changed

- `with_csrf_cookie` helper in `handlers::admin` is now
  `pub(crate)` so it can be reused from `handlers::me_security`.
  Behaviour unchanged.

### Documentation

- `docs/operators.md` — new "Self-service security
  (`/me/security`)" section describing the page, its scope, the
  ownership enforcement, and the things it deliberately does
  *not* do (no in-place password change, no HIBP, no IP/UA
  metadata since the session table doesn't record those today).
  Audit event table updated with `auth.sessions.bulk_revoke_self`.

### Items deferred to v0.19.0+

- Self-serve password change (currently admin-only).
- Recording IP and User-Agent on session creation, so the
  `/me/security` rows can show "MacBook · 192.0.2.10 · 3 hours
  ago" instead of just "started 2026-04-26 14:01 UTC".
- HIBP password breach check (opt-in).
- Idle session timeout, concurrent session cap, suspicious
  activity heuristics.
- Master-key rotation command.

## [0.17.0] - 2026-04-28

Security strengthening pass. Five reinforcements that close
gaps surfaced by an internal security audit, organised as two
delivery blocks.

### Block 1 — response surface

#### Added — security headers middleware

Every response now carries a fixed set of security-relevant
headers. These are not configurable; they are part of the
program's defended posture.

- `Content-Security-Policy: default-src 'self'; script-src 'self';
  style-src 'self' 'unsafe-inline'; img-src 'self' data:;
  font-src 'self'; connect-src 'self'; frame-ancestors 'none';
  base-uri 'self'; form-action 'self'; object-src 'none'`
- `X-Frame-Options: DENY` (belt-and-braces alongside CSP for
  older browsers)
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: strict-origin-when-cross-origin`
- `Permissions-Policy` denying camera, geolocation, microphone,
  payment, USB and friends
- `Strict-Transport-Security: max-age=63072000; includeSubDomains`
  (only when `cookie_secure = true`; HSTS preload is a deliberate
  operator commitment, not something we default on)

The middleware leaves any header an inner handler set deliberately
untouched, so a route that wants a stricter local policy can
override.

#### Added — CORS for the OIDC public endpoints

Routes that legitimately need browser cross-origin access now
return appropriate `Access-Control-Allow-Origin` headers; routes
that don't, don't.

| Route | Policy |
|---|---|
| `/.well-known/openid-configuration` | `*` |
| `/.well-known/jwks.json` | `*` |
| `/oauth2/userinfo` | `*` |
| `/oauth2/token` | Origin allowlist computed at request time from registered `redirect_uris` |
| `/oauth2/introspect`, `/oauth2/revoke` | none — server-to-server |
| `/oauth2/authorize`, `/oauth2/logout` | none — top-level navigation |
| `/admin/*` | none — same-origin |

Browser-based SPA relying parties can now complete the OIDC flow
against sui-id without proxy gymnastics, but only from origins
matching some registered `redirect_uri` of some active client.

#### Added — `Cache-Control: no-store` on `/oauth2/userinfo`

OIDC Core §5.3.2 SHOULD. Without it a CDN or shared proxy could
serve one user's claims to another.

#### Removed — PKCE `plain` from the verifier

`code_challenge_method=plain` was already rejected at the
`/oauth2/authorize` entry point, but `verify_pkce` itself still
contained a working `"plain"` branch that would never be reached
under normal flow. As defense-in-depth the branch is gone:
`verify_pkce` now refuses anything other than `S256`. If the
upstream check ever regresses, this layer still says no.

### Block 2 — token and audit hardening

#### Added — refresh-token theft detection

A refresh-token "family" is a chain of rotations rooted at one
original issuance. We now detect replay of an already-rotated
token and revoke the entire family on detection.

- Schema migration 0008 adds `refresh_tokens.family_id`. Initial
  issuance roots a new family at the new row's id. Each rotation
  copies the parent's `family_id` onto the new row.
- `exchange_refresh` looks up the supplied token via `find_any`
  (which returns even revoked rows). If the token decrypts to a
  *revoked* row, that's a theft signal: the legitimate client
  already rotated it, and an attacker is replaying the captured
  copy. We revoke every active row in the same family — the
  attacker can no longer use the captured token, the legitimate
  client discovers this on next refresh and re-authenticates.
- A new `auth.refresh.theft_detected` audit event records the
  `family_id` and `client_id` so an operator's SIEM can correlate.
- The HTTP response on detection is the same `400 invalid_grant`
  the legitimate-but-already-rotated case would get; we don't
  give an attacker a different response shape to detect.

This follows OAuth 2.1 §6.1 / RFC 6819 §5.2.2.3 / OAuth 2.0
Security Best Current Practice.

#### Added — audit-log hash chain (tamper-evidence)

Every audit row now carries `prev_hash` and `hash`, where
`hash = SHA-256(prev_hash || canonical_bytes(row))` and
`canonical_bytes` is a length-prefixed serialisation that fully
distinguishes field boundaries.

- Schema migration 0009 adds the two columns. Pre-migration rows
  default to empty strings; the verifier treats empty `hash` as
  "predates v0.17.0" and counts them separately rather than
  flagging tampering.
- `audit::append` reads the latest row's hash inside the same
  transaction it inserts, so concurrent appends serialise into a
  single chain.
- `audit::verify_chain_tail(db, limit)` walks the most recent
  rows newest-first and reports the first row whose stored hash
  disagrees with recomputation. Returns a `ChainVerifyReport` with
  `checked`, `legacy_unhashed`, and `broken_at_seq`.
- Startup runs a 5,000-row tail verification and emits
  `tracing::error!` with `broken_at_seq` on detection. We
  deliberately *do not* refuse to start — corrupting a single
  row would otherwise be a denial-of-service amplifier.

This is local tamper-evidence: an attacker who controls the
binary can rewrite the chain end-to-end, but the much more common
"DB-only access" attacker (SQL injection, misconfigured backup,
file-system access) will leave a detectable mismatch. External
timestamping (RFC 3161 or notary service) is a follow-up topic
when there's a concrete operator need.

### Added — tests

- 3 unit tests in `sui-id::security_headers`
- 4 unit tests in `sui-id::cors` (origin parsing)
- 5 unit tests in `sui-id-store::repos::audit` (chain construction,
  empty-prev-hash root, tamper detection, legacy-row handling,
  field-boundary disambiguation)
- 2 unit tests in `sui-id-core::tokens` (PKCE plain rejected,
  unknown methods rejected)
- 4 e2e tests in `sui-id`:
  - `admin_responses_carry_security_headers`
  - `discovery_endpoint_allows_cross_origin_fetch`
  - `jwks_endpoint_allows_cross_origin_fetch`
  - `userinfo_response_carries_no_store_cache_control`
- 2 e2e tests for refresh-token theft detection:
  - `replaying_a_rotated_refresh_token_revokes_the_whole_family`
  - `theft_detection_writes_audit_event`

Workspace lib totals: shared 13, store 15 (+5), core 54 (+2),
sui-id 47 (+7) — **129** lib tests, all passing. Plus 6 new e2e
tests on top of the existing suite.

### Added — documentation

- `docs/operators.md` — new "Security headers" and "CORS"
  sections describing what's emitted and why, with the per-route
  CORS matrix and a note on what an operator's reverse proxy can
  override.

### Audit notes (gaps surfaced and resolved)

- `post_logout_redirect_uri` exact-match: already correct, no
  change needed; deprecation note added on the legacy
  `redirect_uris` fallback.
- PKCE plain at `/oauth2/authorize`: already correct (S256-only);
  defense-in-depth strengthened in `verify_pkce`.
- Session id rotation on login: already correct
  (`SessionId::new()` + cookie overwrite on each login).
- JWT alg constraint: already correct (`EdDSA` only).
- Cookie attributes: already correct (`HttpOnly`, `SameSite=Lax`,
  `Secure` config-controlled).
- Authorization-code single-use, redirect_uri / client_id
  re-validation: already correct.

### Items deferred to v0.18.0+

- `/me/security` self-service UI (active session list, self
  revoke, recent auth events) — UI-heavy, separate release.
- HIBP password breach check, idle session timeout, concurrent
  session limit, suspicious-activity detection,
  master-key rotation command — operator-judgement items;
  staged delivery makes them easier to review individually.

## [0.16.0] - 2026-04-28

Account lockout. After enough consecutive failed password attempts
on an account, sui-id refuses further sign-in attempts even with
the correct password — temporarily, with a configurable cap, and
recoverable by an admin command. The lockout is per-account and
orthogonal to the per-IP rate limiter that's been there since
v0.1.0; together they prevent both single-account hammering and
spread-across-many-accounts hammering.

### Added — `[security]` config section

A new top-level config section, currently with one knob:

```toml
[security]
max_lockout = "24h"
```

Allowed values: `"15min"`, `"1h"`, `"4h"`, `"12h"`, `"24h"`,
`"48h"`. Default is `"24h"`. Picking from a fixed enum avoids
operator typos that would put the cap somewhere wild. The 48-hour
ceiling is deliberate — locking past two days is more likely to
lock out a real user than to deter an attacker.

### Added — progressive backoff curve

The lockout curve in `sui_id_core::session::lockout_backoff`:

| Consecutive failures | Lock window |
| -------------------- | ----------- |
| 1, 2                 | none        |
| 3                    | 30 seconds  |
| 4                    | 1 minute    |
| 5                    | 5 minutes   |
| 6                    | 30 minutes  |
| 7                    | 2 hours     |
| 8                    | 6 hours     |
| 9                    | 12 hours    |
| 10+                  | 24 hours    |

Each value is then capped at `max_lockout`. A successful password
verification clears the counter and lifts any active lock. Two
properties on the curve are tested with `proptest`:

- `backoff_is_monotone_in_failure_count` — more failures never
  produce a *shorter* lock than fewer.
- `backoff_is_bounded_by_max_secs` — the cap is honoured.

### Added — schema migration 0007

Two new columns on `users`:

- `failed_login_count INTEGER NOT NULL DEFAULT 0` — running count
  of consecutive password failures since the last success.
- `locked_until TEXT` — wall-clock time before which password
  verification will be refused. NULL means "not locked".

Pre-migration rows default to `(0, NULL)` — the unlocked state.

### Added — timing-equivalent lockout check

`login_with_mfa` now takes a `max_lockout_secs` parameter and
checks `users.locked_until` *before* fetching the credential row
or running Argon2id. There's no value in grinding the hash for an
account we already plan to refuse.

To avoid leaking the locked state to a remote observer through
timing, the lockout branch runs Argon2id against a fixed dummy PHC
string before returning — same wall-clock cost as a real verify.
A remote observer cannot distinguish "user doesn't exist", "user
disabled", "user locked", and "wrong password" by timing or by
status code; all four return `401 Unauthorized` after a
constant-ish ~80 ms.

### Added — `auth.login.locked` audit event

A new event distinct from `auth.login.failure`, emitted only when
a failed attempt *just* triggered or extended a lock. The note
field includes the consecutive-failure count and the new window
length in seconds. SIEM rules that alert on bursts of locks now
have a clean signal to filter on.

### Added — `sui-id admin unlock-user` CLI subcommand

```bash
sui-id admin unlock-user --username alice --config /etc/sui-id/sui-id.toml
```

Resets `failed_login_count` to 0 and clears `locked_until`. The
operation is recorded as `admin.user.unlock` in the audit log.

The subcommand is the recovery path for legitimate users who've
been locked out before the auto-unlock window expires.

### Added — tests

- 5 new lib tests in `sui_id_core::session::lockout_tests`: 3
  units (no-lock-for-typos, third-failure-window, cap-honoured)
  + 2 properties (monotonic, bounded).
- 3 new e2e tests:
  - `three_consecutive_wrong_passwords_lock_the_account`
  - `admin_unlock_clears_an_active_lock`
  - `successful_login_clears_partial_failure_count`

Lib tests now total **115** across the workspace (shared 13,
store 10, core 52, sui-id 40), all passing. The Argon2id
properties (3 tests) need their own slow run with
`PROPTEST_CASES`.

### Added — documentation

- `docs/operators.md` — new "Account lockout" section with the
  backoff table, the configuration knob, the recovery command,
  the audit-log vocabulary, and an explanation of the timing-
  equivalence behaviour. The Logging section's event vocabulary
  table picks up `auth.login.locked` and `admin.user.unlock`.
- `docs/threat-model.md` A5 (online password guessing) rewritten
  to describe the lockout curve and the trade-off vs. the
  account-takeover-DoS-amplification concern that v0.15.0 and
  earlier deliberately accepted.
- `sui-id.example.toml` — `[security]` block with the curve table
  inline.

### Note for operators

Existing deployments pick this up automatically on first start of
v0.16.0 — the new schema columns default to the unlocked state.
A pre-existing user who has been failing logins before the upgrade
starts the curve from zero on the first failure after the upgrade.

If you'd rather not deploy lockout at all, set `max_lockout =
"15min"` to keep the cap minimal and rely on the per-IP rate
limit as the primary defence. The lockout itself is not
disable-able; we judge "no lockout" to be the wrong default at
this point in sui-id's life.

## [0.15.0] - 2026-04-28

`acr` and `amr` claims in ID tokens, so relying parties can tell
how the user actually authenticated.

### Added — `acr` claim (Authentication Context Class Reference)

ID tokens now carry an `acr` claim with one of three values:

- `"1"` — single factor. Password only.
- `"2"` — multi-factor with a software second factor (TOTP or
  recovery code).
- `"3"` — multi-factor with a phishing-resistant hardware-bound
  key (WebAuthn).

These are the bare numeric ISO/IEC 29115 LoA strings, which is the
form Keycloak and most off-the-shelf IdPs produce. Longer URI
variants (NIST AAL `http://idmanagement.gov/ns/assurance/aal/2`,
eIDAS LoA) target specific national contexts and are needlessly
verbose for a general-purpose IdP — see the design rationale in
`docs/integrators.md`.

### Added — `amr` claim (Authentication Methods References)

ID tokens now carry an `amr` array using RFC 8176 method tokens:

- `pwd` — password
- `otp` — one-time code (TOTP or recovery code; both are OTPs from
  the relying party's perspective, per RFC 8176)
- `hwk` — hardware-bound key (WebAuthn passkey)
- `mfa` — umbrella signal added when two or more *distinct* factor
  types were used. Single-factor sign-ins, even one with a
  hardware key, do not earn `mfa`.

Resulting per-path claims:

| Sign-in path                | `acr` | `amr`                       |
| --------------------------- | ----- | --------------------------- |
| Password only               | `"1"` | `["pwd"]`                   |
| Password + TOTP             | `"2"` | `["pwd", "otp", "mfa"]`     |
| Password + recovery code    | `"2"` | `["pwd", "otp", "mfa"]`     |
| Password + WebAuthn passkey | `"3"` | `["pwd", "hwk", "mfa"]`     |

### Added — `sui_id_shared::AuthMethod`

A typed enum (`Pwd`, `Totp`, `RecoveryCode`, `Webauthn`) plus pure
helpers `acr_from_methods` and `amr_from_methods`. Lives in the
shared crate so all three layers (store models, core flows, HTTP
handlers) reference one canonical representation.

### Added — schema migration 0006

A new `auth_methods TEXT NOT NULL DEFAULT '[]'` column on three
tables: `sessions`, `auth_codes`, `refresh_tokens`. Pre-migration
rows default to `'[]'`, which the issuance code treats as "no
recorded factors" — an empty list produces *no* `acr` / `amr`
claim rather than a misleading `"1"`. New sign-ins from v0.15+
populate the list correctly.

### Snapshot-and-propagate model

The session's authentication factors are recorded once at session
creation. From there:

- `/oauth2/authorize` snapshots the session's `auth_methods` onto
  the new auth code row.
- `/oauth2/token` (authorization-code grant) reads the snapshot
  off the auth code, populates the ID token's `acr` / `amr`, and
  copies the snapshot onto the refresh-token row.
- `/oauth2/token` (refresh grant) reads the snapshot off the
  current refresh token row and copies it onto the new one.

The critical security property: a refreshed ID token reports the
*original* authentication, never a synthesised re-evaluation. A
session that started as password-only can never produce
`acr=2` later, no matter how many refreshes happen, even if the
user enrols TOTP afterwards.

### Added — tests

- 7 lib tests in `sui-id-shared` covering `AuthMethod`,
  `acr_from_methods`, `amr_from_methods` (LoA mapping, dedup,
  RFC 8176 token mapping, `mfa` umbrella semantics).
- 3 e2e tests:
  - `id_token_carries_acr_1_and_amr_pwd_for_password_only_login`
  - `id_token_carries_acr_2_and_amr_with_mfa_after_totp_login`
  - `refresh_grant_preserves_acr_and_amr_from_original_session`

Workspace lib totals: shared **13** (+7), store 10, core 50,
sui-id 40 — **113** total, all passing.

### Added — documentation

- `docs/integrators.md` — "ID token claims" section now describes
  `acr` and `amr`, with the LoA mapping table and the per-path
  examples. Notes that `acr_values` request-side enforcement is
  not yet implemented; relying parties filter on the issued claim
  for now.

### What this does *not* change

- `userinfo` is unchanged. `acr` / `amr` are ID-token claims per
  OIDC Core; the userinfo endpoint continues to expose `sub`,
  `preferred_username`, and `name`.
- The `acr_values` request parameter is *not* honoured. A relying
  party that requires a minimum LoA must filter on the returned
  `acr` claim.

## [0.14.0] - 2026-04-28

Property-based tests (`proptest`) for the parts of sui-id that
guard correctness or security boundaries. No production code
behaviour changes; one tiny refactor extracts the redirect-URI
matcher into its own `pub fn` so a property test can exercise it
directly.

### Added — proptest infrastructure

- `proptest = "1.5"` added as a workspace dev-dependency. Pulled in
  by `sui-id`, `sui-id-core`, and `sui-id-store` under
  `[dev-dependencies]` only — never reaches production builds.
- `CONTRIBUTING.md` gains a "Property-based tests" section
  explaining the case-count convention (256–512 for cheap
  properties, 4 for Argon2-driven ones), how to widen coverage via
  `PROPTEST_CASES=…`, and the rule that proptest regression files
  under `proptest-regressions/` are committed so a shrunk
  counter-example replays forever.

### Added — sui-id-store::crypto: 4 properties on seal / open

  - `round_trip_for_arbitrary_plaintext_and_aad` — `open(seal(p, a), a) == p`
    over arbitrary plaintexts (0..2048 bytes) and AADs (0..256 bytes).
  - `open_with_wrong_aad_fails` — different AAD must reject.
  - `open_with_wrong_key_fails` — different key must reject.
  - `ciphertext_strictly_grows_by_nonce_plus_tag` — output length
    is exactly `plaintext.len() + 24 (nonce) + 16 (tag)`. A future
    framing regression would surface here.

### Added — sui-id-core::tokens: 3 properties on PKCE S256

  - `s256_verifies_iff_challenge_was_derived_from_same_verifier` —
    cross-checked against a separate reference S256 derivation.
  - `s256_rejects_any_distinct_verifier`.
  - `s256_challenge_size_is_43_chars` — the SHA-256 →
    base64url-no-pad framing is exactly 43 characters; anything
    else is a length bug.

### Added — sui-id-core::password: 3 properties on Argon2id

  - `verify_succeeds_for_any_round_trip`.
  - `verify_fails_on_any_distinct_password`.
  - `hashes_differ_across_invocations_for_same_password` — guards
    against a zero-salt regression that would let two users with
    the same password share a hash.

  Cases capped at **4** per property because Argon2id at production
  parameters is intentionally slow. Operators / CI can raise the
  bar with `PROPTEST_CASES=…`.

### Added — sui-id::ipnet: 4 properties on the CIDR matcher

  - `ipv4_contains_matches_naive_implementation` — cross-check
    against an independent brute-force reference. The matcher is
    where off-by-one errors at /0, /32, and the boundaries
    historically surface in this kind of code.
  - `an_address_is_always_in_its_own_slash_32`.
  - `slash_zero_contains_every_v4`.
  - `v4_and_v6_never_cross_match` — a v6 probe must never satisfy
    a v4 CIDR.

### Added — sui-id-core::authorize: 5 properties on redirect_uri matching

  Plus a small refactor: the inline check
  `client.redirect_uris.iter().any(|u| u == &params.redirect_uri)` is
  now a `pub fn is_redirect_uri_registered(&[String], &str) -> bool`,
  with a doc comment explaining why the rule must be byte-exact and
  why no normalisation is allowed. Production behaviour is unchanged.

  Properties:

  - `registered_uri_is_always_accepted`.
  - `one_byte_off_uri_is_rejected` — a single byte flipped anywhere
    must reject.
  - `case_difference_is_not_folded` — `/cb` and `/CB` are different
    URIs.
  - `prefix_extension_is_rejected` — registered + arbitrary suffix
    must reject (defends against attacker-controlled
    `https://attacker.example/cb/../../leak`-style submissions).
  - `multi_registry_matches_each_member_and_only_them`.

### Test counts

  - `sui-id-store` lib: **10** (was 6)
  - `sui-id-shared` lib: 6 (unchanged)
  - `sui-id-core` lib: **50** (was 39, +11 properties)
  - `sui-id` lib: **40** (was 36, +4 properties)

  Workspace lib total: **106**, all passing. The 41 e2e tests in
  `sui-id` are unchanged.

### Note on running times

The Argon2id properties are the slowest in the suite. With the
default `cases: 4` they add ~50 seconds to a debug `cargo test -p
sui-id-core --lib` on the reference build host. The other properties
add well under a second each. This is the reason for the asymmetric
case-count convention; raise it before a release with
`PROPTEST_CASES=…`.

## [0.13.0] - 2026-04-28

Server migration / secure backup. The `backup` and `restore`
subcommands gain provenance metadata, optional passphrase-based
encryption, and a new sibling `verify-backup` for read-only checks.

### Added — `MANIFEST.json` in every backup

Every backup tarball produced by v0.13+ now includes a
`MANIFEST.json` entry alongside `sui-id.sqlite` and `sui-id.key`:

```json
{
  "format_version": 1,
  "sui_id_version": "0.13.0",
  "schema_version": 5,
  "created_at": "2026-04-28T10:31:42Z",
  "hostname": "idp.example.com",
  "issuer": "https://idp.example.com"
}
```

`restore` reads the manifest before doing anything destructive and
refuses to act on:

- a backup whose `format_version` is newer than the running binary
  knows;
- a backup whose `schema_version` is newer than the running binary
  has migrations for.

Both are recoverable operator failures — rebuild with the right
binary version and try again.

Backwards compatible: backups produced by v0.12 and earlier (with
no manifest) continue to restore on v0.13. The compatibility check
treats them as "format_version = 0, schema_version = 0" — i.e. the
strictest reading is no reading.

### Added — passphrase-encrypted backups (`--encrypt` / `--decrypt`)

For backups that will leave the host's trust boundary (cloud
storage, off-site media, transfer to a migration host):

```bash
sui-id backup --to /tmp/backup.tar.enc --encrypt
sui-id restore --from /tmp/backup.tar.enc --decrypt
```

The envelope format:

```
magic(8)    "SUIDIDBK"
version(4)  big-endian u32, currently 1
salt(16)    Argon2id input
nonce(24)   XChaCha20-Poly1305 nonce
ciphertext  the inner tarball
tag(16)     Poly1305 tag
```

Key derivation: Argon2id with `m_cost = 64 MiB`, `t_cost = 3`,
`p_cost = 1`. Salt and nonce are generated fresh per backup. The
choice of parameters targets ~1 second of derivation on commodity
server hardware — comfortably above the OWASP minimum, well below
operator pain.

The passphrase can be supplied:

- **interactively** at the terminal (asked twice for `backup
  --encrypt`, once for `restore --decrypt`); or
- **non-interactively** via `SUI_ID_BACKUP_PASSPHRASE`, for cron
  and scripted use.

Operator misuse is caught:

- `restore --decrypt` against a plain tarball errors out with
  "backup file is not encrypted, but a passphrase was provided"
  rather than silently succeeding.
- `restore` against an encrypted backup without `--decrypt` errors
  out telling the operator to add `--decrypt`.

### Added — `sui-id verify-backup`

A new read-only subcommand:

```bash
sui-id verify-backup --from /tmp/backup.tar.enc --decrypt
```

It reads the file, decrypts if needed, parses the manifest, and
runs `PRAGMA integrity_check` on the inner SQLite snapshot.
Output looks like:

```
Format version: 1
sui-id version: 0.12.0
Schema version: 5
Created at:     2026-04-28T10:31:42Z
Hostname:       old-host.example.com
Issuer:         https://idp.example.com
Encrypted:      true
Tar size:       183808 bytes
Database size:  180224 bytes
Master key:     present

✓ SQLite integrity check passed
✓ Decrypted with provided passphrase
```

Use cases:

- Pre-flight before an upgrade-and-restore sequence on a new host.
- Daily smoke test from cron against the latest backup, so a
  corrupted-snapshot regression doesn't go undiscovered for weeks.
- Inspecting an unfamiliar backup file (when did it come from?
  what version produced it? does it have a key?).

The subcommand never writes to the configured storage paths.

### Added — `sui-id-store::migrations::MAX_SCHEMA_VERSION`

The largest schema version this build's bundled migrations
produce, computed at compile time from the migrations slice. Used
by `restore` to refuse a too-new backup, and exposed for any other
caller that needs the same answer.

### Added — tests

Eight new unit tests in `sui-id::backup`:

- `manifest_present_in_plain_backup`
- `encrypted_backup_round_trips_with_correct_passphrase`
- `encrypted_backup_rejects_wrong_passphrase`
- `restore_of_encrypted_without_passphrase_errors`
- `restore_of_plain_with_passphrase_errors`
- `verify_reports_manifest_and_runs_integrity_check`
- `verify_works_on_encrypted_backup_with_passphrase`
- `restore_refuses_backup_with_too_new_schema_version`

The four pre-existing backup tests were migrated to the new
`BackupOptions` / `RestoreOptions` signatures; all twelve pass.

Smoke-tested end-to-end: a plain backup → `verify-backup` → restore
into a different path round-trips through a real SQLite database;
an encrypted backup with `SUI_ID_BACKUP_PASSPHRASE` round-trips the
same way; an encrypted backup with the wrong passphrase fails
cleanly without writing the destination files.

### Documentation

- `docs/operators.md`: "Backup and restore" section rewritten end
  to end. New subsections cover encrypted backups, `verify-backup`,
  and a recommended migration sequence (old-host backup with
  `--encrypt`, transfer, verify-backup pre-flight, restore on new
  host, DNS cutover, retire old host).
- `docs/deployment.md`: section 9 (Backups) split into plain vs
  encrypted cron examples; adds a daily `verify-backup` smoke test
  to the schedule.
- `docs/threat-model.md`: new threat A13 ("Attacker who intercepts
  a backup tarball in transit") spelling out the encryption model,
  the Argon2id parameter choice, the passphrase-management
  responsibilities, and the deliberate non-recoverability of a
  forgotten passphrase.

### Note for operators

Existing cron jobs that produce plain `.tar` backups continue to
work unchanged. Adopt `--encrypt` (and a passphrase file at
`/etc/sui-id/backup.pass`, mode 0600) when you next review the
backup pipeline; meanwhile, plain backups produced by v0.13 carry
the manifest, which makes future upgrades safer either way.

## [0.12.0] - 2026-04-28

Structured logging and request correlation.

### Added — request_id middleware

Every HTTP request now picks up an `X-Request-Id`. If the caller
supplied one (alphanumeric, dot/dash/underscore, ≤64 chars) we
keep it; otherwise we generate a fresh UUIDv4. The id is:

- attached to the `tracing` span that wraps handler execution, so
  every log line emitted while handling a request — including ones
  from inside use cases — carries it automatically;
- echoed back in the response's `X-Request-Id` header so the caller
  / reverse proxy can correlate;
- stashed in a request extension as `RequestId(String)` for
  handlers that want to read it directly.

The middleware also writes a structured `request received` line on
entry and a `request completed` (with `status` and `latency_ms`)
line on exit. With `log.format = "json"` these become SIEM-ingestible
records:

```json
{
  "fields": { "message": "request completed", "status": 200, "latency_ms": 4 },
  "spans": [{ "method": "POST", "path": "/oauth2/token",
              "request_id": "0c58b960-f963-4427-86f0-d4e16938d8aa",
              "name": "request" }]
}
```

### Added — `sui_id_core::events`

A typed `SecurityEvent` enum (with variants `LoginPasswordSuccess`,
`LoginPasswordFailure`, `MfaSuccess`, `MfaFailure`, `AdminMfaReset`,
`AuthorizeIssued`, `AuthorizeRejected`, `TokenIssued`,
`TokenRefreshed`, `TokenIntrospected`, `TokenRevoked`, `Logout`,
`SessionRevoked`, `LoginPasswordOkMfaRequired`) plus an `emit()`
function that, given a `Context` (actor / client_ip / request_id),
writes a structured tracing line **and** appends an audit-log row
in one go.

This unifies the two parallel paths that used to drift apart —
`tracing::info!` for live observability and `audit::append` for
durable record-keeping — behind a single typed API. Adding a new
kind of security event is now a single match arm.

The existing `audit::append` callers continue to work unchanged.
A follow-up release will migrate them to `events::emit` site by
site; the first wave of migrations needs careful test alignment
because some E2E tests match exact action-string and note values.

### Added — documentation

- `docs/operators.md` "Logging" section now documents the
  request-id propagation, the structured event vocabulary
  (the canonical event-name table and the field shape), and example
  jq queries against the JSON log stream. Reverse-proxy snippets for
  Caddy and nginx show how to forward request ids from the edge.

### Added — tests

- 4 new E2E tests for the request-id middleware:
  - `response_carries_a_generated_x_request_id_when_caller_omits_one`
  - `caller_supplied_x_request_id_is_echoed_back`
  - `caller_supplied_x_request_id_thats_too_long_is_replaced` —
    confirms the 64-char cap defends against log padding attacks.
  - `caller_supplied_x_request_id_with_unsafe_chars_is_replaced` —
    confirms the alphabet-restricting filter rejects (and replaces)
    values containing whitespace.

Lib tests continue green: 79/79 (28 sui-id + 39 sui-id-core + 6
store + 6 shared).

### Note for operators

The log lines have changed shape. If you have a SIEM rule that
matched on the previous unstructured output, point it at the new
event-name field instead — see the table in operators.md. The
data is the same; only the access pattern is more uniform.

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
- Crate authorship and contact: now `nabbisen <nabbisen@scqr.net>` for all
  five workspace crates (was `sui-id contributors`).
- Repository / homepage URLs across the workspace: now
  `https://github.com/nabbisen/sui-id` (was `sui-id/sui-id`). Updated
  in workspace `Cargo.toml`, every crate's `README.md`, the docs
  under `docs/`, the `.github/` files, `PUBLISHING.md`, `ROADMAP.md`,
  and `TERMS_OF_USE.md`.
- The `LICENSE` file's copyright line is now
  `Copyright 2026 nabbisen <nabbisen@scqr.net>`.
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
