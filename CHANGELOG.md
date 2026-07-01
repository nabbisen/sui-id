# Changelog

All notable changes to sui-id will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.76.3] — 2026-06-14

**RFC 008 — Third-Party-Posture Bundle.** Completes the five-capability
bundle enabling sui-id to serve third-party OAuth clients with a proper
consent boundary.

### Added — store layer

**Migration 0035** — `clients` table widened: `registered_via TEXT NOT NULL
DEFAULT 'admin' CHECK (... IN ('admin','dynamic'))`, `logo_uri TEXT`,
`homepage_uri TEXT`, `privacy_policy_uri TEXT`, `tos_uri TEXT`.

**Migration 0036** — two new tables:

- `scope_definition(name PK, requires_consent, is_default, created_at)` —
  seeded with `openid` (no consent), `profile`, `email`, `offline_access`
  (consent required). Operators extend via admin pages.
- `client_registration_token(id PK, token_hash UNIQUE, max_uses, used_count,
  expires_at, revoked_at, note, created_at, updated_at)` — RFC 7591 initial
  access tokens, stored hashed (P5).

**`RegistrationSource { Admin, Dynamic }`** enum in `models.rs`.
**`RegistrationTokenId`** added to `sui-id-shared::ids`.

**`repos::scope_definition`** — list, get, create, delete, consented_names;
4 unit tests.

**`repos::client_registration_token`** — create, list, get, find_by_hash,
`consume` (atomic: reads + validates + increments used_count in one
transaction; `?`-propagates Err to trigger rollback on failure), revoke;
4 unit tests.

**`repos::clients`** — SELECT + map_row extended for 5 new columns;
`update_app_identity` (validates HTTPS-or-localhost per P6);
`set_registered_via`; `is_valid_app_uri` helper.

### Added — `POST /oauth2/register` (RFC 7591)

Handler `crates/sui-id/src/handlers/dynamic_register.rs`:

- Bearer token extracted from `Authorization` header and SHA-256-hashed
  before calling `consume()` (P4/P5).
- Unknown or expired/exhausted token → 400 `invalid_token`.
- `redirect_uris` and `client_name` required; application-identity URIs
  validated HTTPS-or-localhost (P6); validation delegates to core's
  `validate_redirect_uri` (now `pub`).
- Client created with `is_disabled = true` (admin must explicitly enable;
  P4), `consent_policy = 'first_time_only'`, `registered_via = 'dynamic'`.
- Returns RFC 7591 `ClientInformation` JSON with `client_id`,
  `client_secret` (for confidential clients), and reflected metadata.
- Audit event `client.dynamic_register` emitted.

### Added — CLI `sui-id admin issue-registration-token`

Generates a 32-byte random token, SHA-256-hashes it into
`client_registration_token`, prints the raw token once with usage example.
Accepts `--max-uses N` (default 1) and `--note TEXT`.

### Changed — consent screen (RFC 008 capability 4)

`ConsentData` gains `logo_uri: Option<String>` and `homepage_uri:
Option<String>`.  `render_consent` displays the app logo (with "App logo
provided by the application" label per P6) and wraps the client name in a
link to the homepage.  The consent gate in `oidc.rs` passes both fields
from `ClientRow`.

### Added — i18n strings (7 × 3 locales)

`dynamic_reg_token_issued_flash`, `dynamic_reg_token_revoked_flash`,
`dynamic_reg_registered_flash`, `dynamic_reg_token_invalid`,
`dynamic_reg_token_expired`, `scope_catalog_created_flash`,
`scope_catalog_deleted_flash` — in English, Japanese, Chinese Simplified.

### Audit matrix

`client.dynamic_register` added → **46 × 46**.

**135/135 tests pass** (93 in store including 8 new RFC 008 tests).
0 clippy errors. All 5 CI gates PASS.

## [0.76.2] — 2026-06-14

**RFC 009 — Pluggable SQL Backends, Step 1: `Backend` trait + `SqliteBackend`.**
A pure architectural refactor with no observable behaviour change.  Introduces
the storage-backend seam that Steps 2 (Postgres) and 3 (MariaDB) will build on.

### Added — `sui-id-store::backend` module

- **`Backend` trait** — object-safe via type erasure.  Two erased methods:
  `with_conn_erased(ConnFn)` and `with_tx_erased(TxFn)`.  Also exposes
  `key()`, `driver_name()`, and `as_any()` for downcast.
  - `ConnFn = Box<dyn FnOnce(&Connection) -> Box<dyn Any + Send> + Send>`
  - `TxFn  = Box<dyn FnOnce(&Transaction<'_>) -> StoreResult<Box<dyn Any + Send>> + Send>`
  - `TxFn` returns `StoreResult` so `with_tx_erased` can `?`-propagate errors
    to trigger rollback before commit (correct transaction semantics).

- **`SqliteBackend`** — wraps `Arc<Mutex<rusqlite::Connection>>` (cheap clone,
  `'static`-safe for `spawn_blocking`).  Implements `Backend` with the same
  `spawn_blocking`-dispatch strategy as the old `Database::inner`.  Adds
  `with_conn_sync` / `with_tx_sync` (used by the migration runner and blocking
  tests).  `driver_name() = "sqlite"`.

### Changed — `sui-id-store::db`

`Database` is now `struct Database { backend: Arc<dyn Backend> }`.  The
public API is **identical** to before:

- `Database::open(path, key)` → opens SQLite, runs migrations, wraps a
  `SqliteBackend`.
- `Database::open_in_memory(key)` → same for in-memory SQLite (tests).
- `with_conn<F,R>`, `with_tx<F,R>` → generic wrappers that box the caller's
  closure into the erased `ConnFn`/`TxFn`, dispatch through the `Backend`
  trait, and unbox the result.  The caller API is byte-for-byte identical.
- `with_conn_sync`, `with_tx_sync` → downcast to `SqliteBackend` (Step 1 only).
- `key()`, `driver_name()` → forwarded to the backend.

### Added — 5 backend tests

`driver_name_is_sqlite`, `with_conn_executes_on_blocking_thread`,
`with_tx_commits`, `with_tx_rolls_back_on_error`, `sync_methods_work`.

### Why this matters

Steps 2 and 3 (Postgres, MariaDB) will implement `Backend` and be
swapped in by constructing a different `Arc<dyn Backend>` — zero call-site
changes above the repo layer.  The P6 startup driver-match check will read
`driver_name()` and compare it with the `sui_meta.schema_version` record.

**127/127 tests pass** (up from 122).  0 clippy errors.
All 5 CI gates PASS (text-leaks, css-tokens, semantic-parity=36,
inline-style=1, audit-matrix=45×45).

## [0.76.1] — 2026-06-14

**RFC 005 — Pluggable User Backends (Read-Only LDAP User-Source).** Adds a
`UserSource` trait and an LDAP implementation that lets sui-id authenticate
users against an external directory without owning their credentials.  Local
users are always checked first (P4); LDAP is purely additive.

### Added — `UserSource` trait (`sui-id-store::user_source`)

- `UserSource: Send + Sync` async trait with `authenticate(username, password)`
  returning `Ok(Some(ExternalUserRecord))` on success, `Ok(None)` on miss
  or wrong password (deliberately conflated — P3), `Err(UserSourceError)` on
  transport failure.
- `ExternalUserRecord`: `stable_id`, `display_username`, `email`, `display_name`,
  `source_slug`.
- `cascade_sources(sources, username, password)` — tries each source in order;
  transport errors are logged and the cascade continues (P4 fail-soft).
- `InMemoryUserSource` — in-memory test double.
- 5 unit tests: match/miss/wrong-password/cascade-first-match/cascade-past-error.

### Added — LDAP implementation (`sui-id-store::ldap_source`, `ldap` feature)

Behind the `ldap` feature flag (enabled in the release binary; off by default
for tests).

- `LdapUserSource` / `LdapUserSourceConfig` using `ldap3 = "0.12"` (Tokio,
  Rustls — no openssl).
- **P1 (DN injection):** `escape_filter_value` escapes RFC 4515 metacharacters
  `* ( ) \ NUL` before substituting `{username}` into the search filter.
- **P2 (TLS):** URL must start with `ldaps://`; checked at config-load and at
  connect time.
- **P3 (timing):** search-then-bind; both miss and wrong-password return
  `Ok(None)`.
- **P6 (least-privilege):** service account used read-only for search.
- 4 unit tests for RFC 4515 escaping (plain / metacharacters / NUL / injection).

### Added — migration 0034 (`users.source`, `users.external_stable_id`)

```sql
ALTER TABLE users ADD COLUMN source TEXT NOT NULL DEFAULT 'local'
    CHECK (source IN ('local', 'ldap'));
ALTER TABLE users ADD COLUMN external_stable_id TEXT;
CREATE UNIQUE INDEX idx_users_external ON users (source, external_stable_id)
    WHERE external_stable_id IS NOT NULL;
```

### Added — `UserSource` enum in models, users repo additions

- `UserSource { Local, Ldap }` enum with `as_str()`, `parse()`, `Default`
  (`Local`).
- `UserRow` gains `source: UserSource` and `external_stable_id: Option<String>`.
- `users::find_by_external_stable_id` — lookup by `(source, external_stable_id)`.
- `users::upsert_ldap_shadow` — creates a password-less shadow row on first
  LDAP sign-in; updates display fields on subsequent sign-ins; returns `UserId`.
- 4 unit tests: create/update/source-field/not-found.

### Added — `AuthMethod::Fed` variant

New `Fed` variant in `sui-id-shared::AuthMethod` for LDAP-sourced sessions,
with `as_amr() = "fed"`, `is_second_factor() = true`,
`is_phishing_resistant() = false`.

### Added — cascade hook in binary crate

- `AppState.user_sources: Vec<Arc<dyn UserSource>>` — populated from
  `[[user_source]]` config blocks in `startup.rs`.
- `UserSourceConfig` in `config.rs` with `validate_and_resolve_password()`
  (reads bind password from environment variable — P2; rejects `ldap://` — P2;
  rejects empty `bind_dn` — P2 no anonymous bind).
- `try_login_with_cascade` in `auth.rs` — calls local `login_with_mfa` first;
  if `InvalidCredentials` and the username is unknown locally, runs the
  cascade; on `Matched`, upserts the shadow row, emits
  `auth.user_source.matched` audit event, creates a session.
- `login_post` updated to call `try_login_with_cascade`.

### Added — audit events and matrix

`auth.user_source.matched` and `auth.user_source.transport_failure` added to
`docs/src/reference/audit-coverage-matrix.md`. Gate passes at **45 × 45**.

**122/122 tests pass** (19 shared + 20 store-pre + 80 store-all + 3 web).
0 clippy errors. All 5 CI gates PASS.

## [0.76.0] — 2026-06-14

**RFC 006 — Prometheus Metrics Endpoint.** Adds a `/metrics` endpoint
(disabled by default) exposing a bounded, no-PII Prometheus catalog of
counters, gauges, and histograms for operational observability.

### Added — `sui_id_store::metrics` module (`prometheus = "0.13"`)

A `Metrics` struct owns the entire metric registry: 10 counters,
3 gauges, and 2 histograms.  Typed increment helpers (`m.signin(result)`,
`m.token_issued(kind)`, …) are the sole call-site API; no call site
imports Prometheus types directly.  Published catalog (P3/P4 — no PII,
no UUID labels):

**Counters:**
`sui_id_signin_attempts_total{result}`,
`sui_id_signin_via_passkey_total`,
`sui_id_token_issued_total{kind}`,
`sui_id_token_revoked_total{reason}`,
`sui_id_mfa_enrolled_total{kind}`,
`sui_id_mfa_recovery_consumed_total`,
`sui_id_forgot_password_requested_total`,
`sui_id_audit_appended_total`,
`sui_id_email_outbox_enqueued_total`,
`sui_id_email_outbox_failed_total{reason}`.

**Gauges:**
`sui_id_active_sessions`,
`sui_id_signing_keys_active`,
`sui_id_signing_keys_retired`.

**Histograms:**
`sui_id_http_request_duration_seconds{route, status_class}` (buckets
5ms–10s), `sui_id_argon2_verify_duration_seconds` (buckets 10ms–5s).

5 unit tests: registry construction, counter increments by kind, gauge
set, catalog presence + PII guard.

### Added — global metrics handle (`sui_id_store::set_global_metrics`)

A `OnceLock`-backed global handle installed at startup lets store
internals (`audit::append`, `email_outbox::enqueue`) increment counters
without any signature change at their 40+ call sites.

### Added — `AppState.metrics: Option<Arc<Metrics>>`

`None` when `metrics_enabled = false`; handlers use the
`app.metric()` accessor which returns `Option<&Metrics>`.

### Added — call-site hooks

- **`audit::append`** → `audit_appended_total` (via global handle).
- **`email_outbox::enqueue`** → `email_outbox_enqueued_total` (via
  global handle).
- **`handlers/admin/auth.rs`** — `login_post` and `mfa_challenge_post`:
  `signin_attempts_total{result=success|wrong_password|mfa_failed}`.
- **`handlers/oidc.rs`** — token endpoint: `token_issued_total{kind}`.

### Added — `GET /metrics` handler (RFC 006 P1/P2/P5)

Registered **only** when `metrics_enabled = true` (P5 — no route, no
404-vs-empty signal). Auth: admin session cookie OR `Authorization:
Bearer <token>` with constant-time SHA-256 comparison against
`server_settings.metrics_token_hash` (P2). No credential → 401.
Output: Prometheus text format.

### Added — migration 0033

`ALTER TABLE server_settings ADD COLUMN metrics_token_hash TEXT` (nullable).
Populated via `sui-id admin rotate-metrics-token`.

### Added — `sui-id admin rotate-metrics-token` CLI subcommand

Generates a fresh 32-byte random token, SHA-256-hashes it into
`server_settings.metrics_token_hash`, prints the raw token once to
stdout. Rotation is immediate — no two-valid-tokens grace window (P6).

### Added — config (`[server]` section)

`metrics_enabled = false` (default), `metrics_listen_addr = ""` (blank =
same listener; set to e.g. `"127.0.0.1:9090"` for a private port).

**112/112 tests pass** (19 shared + 20 store-pre + 70 store-all
including 5 new metrics tests + 3 web).
All 5 CI gates PASS (text-leaks, css-tokens, semantic-parity=36,
inline-style=1, audit-matrix=43×43).

## [0.75.1] — 2026-06-14

**Documentation: detailed write-ups for the six proposed post-1.0 RFCs.**
No code change. The exploratory sketches for RFCs 004, 005, 006, 008, 009,
and 025 were expanded into the project's canonical 15-section RFC structure
(Summary, Motivation, Background, Target code areas, Security properties,
Non-goals, Proposed design, Data model impact, API impact, Testing
strategy, Migration strategy, Rollout plan, Risks and mitigations,
Acceptance criteria, Open questions).

All six remain `Proposed` with no scheduled delivery; each requires
explicit owner direction and real-environment soak before implementation.

### Written up

- **RFC 004 — Federation (upstream OIDC RP).** Trust-boundary design
  settled: mapping by upstream `sub` (never email), no auto-merge by
  email, provision-on-first-login gated on verified email, local MFA never
  bypassed, no upstream-token storage. Two new tables
  (`federation_provider`, `federation_link`).
- **RFC 005 — Pluggable user backends (read-only LDAP).** `UserSource`
  trait, local-first hardcoded cascade (fail-soft so a flaky directory
  never locks out the local admin), password-less shadow rows, RFC 4515
  filter escaping, TLS-required, timing-equivalent miss/wrong-password.
- **RFC 006 — Prometheus metrics endpoint.** Auth-gated `/metrics`
  (admin session or constant-time bearer token), bounded no-PII catalog,
  no per-user/per-client cardinality, disabled-by-default (404 when off).
- **RFC 008 — Third-party-posture bundle.** Single-release bundle: consent
  screen (the security boundary), RFC 7591 dynamic registration
  (token-gated by default, clients start disabled), per-client scope
  policy + localised scope catalog, application identity (validated-not-
  fetched URLs), per-user refresh-token revocation. Explicitly ships
  whole-or-not.
- **RFC 009 — Pluggable SQL backends (PostgreSQL, MariaDB).** `Backend`
  trait over `sqlx` (recommended Option C), AAD invariant across backends
  (encrypted-column round-trip canary), per-driver migration directories,
  env-indirected connection secrets, batched-save-point master-key
  rotation, driver/data startup match check. SQLite stays the default
  forever.
- **RFC 025 — Multi-tenant expansion (detailed design).** Flat tenants,
  single-source-of-truth `tenant_id` propagation, per-tenant signing keys
  / audit chains / rate limiting, global-vs-tenant admin with audited
  assumption, slug-validated routing (404 unknown / 503 suspended,
  privacy-preserving), both-issuer one-version migration transition.
  Detailed-design only — exists to prevent piecemeal tenancy and to inform
  current schema decisions.

### Index

`rfcs/README.md` proposed-section note added: these six now carry full
detailed designs and remain post-1.0 candidates out of the v1.0 critical
path.

**107/107 tests pass (unchanged). All 5 CI gates PASS. No code change.**

## [0.75.0] — 2026-06-14

**Codebase audit pass.** No new features. No wire-protocol change.
Findings and fixes across five audit dimensions:

### Fixed — RFC 088 gap: 403 page now uses auditor-specific copy

`render_error` previously rendered HTTP 403 with the generic
`error_generic_title / error_generic_lede` strings because the `match`
had no arm for 403. The `error_403_auditor_title` and
`error_403_auditor_body` keys introduced in RFC 088 were therefore
unreachable. Added `403 => (t.error_403_auditor_title, t.error_403_auditor_body)`
to the match, so auditors hitting a mutation-only GET now see
"Read-only access — Your account has read-only (auditor) access."
instead of the generic error page.

### Fixed — audit coverage matrix: `auth.login.password_ok_mfa_required` added

The event literal `"auth.login.password_ok_mfa_required"` exists in the
source but was absent from `docs/src/reference/audit-coverage-matrix.md`,
causing the backward-check CI gate to fail. Added to the matrix under
the Authentication flow section. Gate now passes 43 × 43 bidirectional.

### Fixed — ROADMAP stale link: RFC 085 pointed to `proposed/`

The auth-core arc table in `ROADMAP.md` had one stale row linking
RFC 085 to `rfcs/proposed/` instead of `rfcs/done/`. Fixed. Also
marked that row `✅` (it shipped in v0.68.0 alongside RFC 083).

### Removed — dead i18n keys (10 keys, all locales)

Keys removed from `strings.rs` and `en.rs` / `ja.rs` / `zh_hans.rs`:

| Key | Reason for removal |
|---|---|
| `users_empty` | Replaced by `empty_users` (RFC 092) |
| `clients_empty` | Replaced by `empty_clients` (RFC 092) |
| `signing_keys_empty` | Replaced by `empty_signing_keys` (RFC 092) |
| `error_404_title` | Old name; codebase uses `error_not_found_title` |
| `error_404_lede` | Old name; codebase uses `error_not_found_lede` |
| `error_429_title` | Old name; codebase uses `error_too_many_requests_label` |
| `error_429_lede` | Old name; codebase uses `error_too_many_requests_lede` |
| `error_500_title` | Old name; codebase uses `error_internal` |
| `error_500_lede` | Old name; codebase uses `error_internal_lede` |
| `signing_keys_rotate_warning` | Replaced by confirm-page copy (RFC 090) |

29 `audit_event_*` keys are **kept** (marked with a comment explaining
they are planned future copy for audit-log display localisation, not yet
wired to any renderer).

### Removed — `table_empty_row` from public exports

`table_empty_row` has zero call sites in page renderers after RFC 092
wired `empty_state` into all four list surfaces. The function definition
is retained (it is still `pub(super)` inside `pages/common.rs`, used
by the old `EmptyStateData` path which `passkey.rs` and `dashboard.rs`
still use), but removed from the `sui_id_web` crate root `pub use` and
from `pages.rs`'s re-export list.

### Added — 3 tests for `get_summary` (`sui-id-store`, RFC 090)

`pending_settings_change::get_summary` was introduced in RFC 090 but had
no test. Three tests added:

- `get_summary_returns_summary_for_live_row` — P6 (non-secret summary
  returned for a live, non-expired row).
- `get_summary_returns_not_found_for_absent_id` — P3 (absent id is
  indistinguishable from expired).
- `get_summary_returns_not_found_for_expired_row` — P4 (expired row
  returns `NotFound`) and **non-destructive contract** (the row is
  not deleted by `get_summary`, only by `consume` or `purge_expired`).

**107/107 tests pass** (19 shared + 20 store-pre + 65 store-all + 3 web).
All 5 CI gates PASS (text-leaks, css-tokens, semantic-parity=36,
inline-style=1, audit-matrix=43×43).

## [0.74.0] — 2026-06-14

**UI-security unit 6: RFC 092 — UI Component Suite.**
Completes the v2.3 contract. ThemeToggle P1–P3 (Appendix E) all met.
EmptyState wired into all four list surfaces. CopyField component.
Error-summary component. No wire-protocol change.

### Changed — `theme-init.js` robustness (RFC 092 P1, P2, P3)

`crates/sui-id/static/theme-init.js` updated:
- **No-JS/JS root-class swap:** adds
  `document.documentElement.classList.replace('no-js', 'js')` at the
  top of the IIFE, before any `localStorage` access. The swap runs
  unconditionally on any JS-enabled browser regardless of storage
  permission.
- **localStorage isolation:** the `localStorage.getItem` call is now
  in its own inner `try/catch` separate from the outer IIFE wrapper,
  so a storage exception falls back to the `"system"` default without
  stopping theme application.
- **Blocking load** (Appendix E): `<script src="/static/theme-init.js"
  defer>` → `<script src="/static/theme-init.js">` in both `Shell`
  and `AuthShell` in `layout.rs`. Removes the flash-of-wrong-theme
  risk on first paint.

### Changed — `<html class="no-js">` in both shells (RFC 092)

Both `Shell` and `AuthShell` now emit `<html ... class="no-js">`.
`theme-init.js` upgrades it to `class="js"` immediately; on any JS-
disabled browser the class stays `no-js` and the CSS rules hide the
non-functional toggle button.

### Added — ThemeToggle no-JS fallback text (RFC 092 P3)

A `<p class="theme-no-js-note">` is rendered after the toggle button
group, carrying `{t.theme_noscript_note}`. CSS hides it when JS is
enabled (`.js .theme-no-js-note { display:none }`). No-JS users see
"Theme follows your system setting." instead of a dead button.

### Added — no-JS/JS visibility CSS rules (RFC 092)

Two new structural rules appended to `CHROME_THEME_TOGGLE_CSS`:
`.no-js .theme-toggle { display:none }` and
`.js .theme-no-js-note { display:none }`.

### Added — `EmptyState` component (`utilities.rs`, RFC 092)

`pub fn empty_state(message, cta: Option<(&str, &str)>)` renders a
centred message and an optional CTA link. The CTA is omitted when
`can_write` is false so auditors see the message without a mutation
control. Wired into:
- Users list — "No users yet." + "Create first user" CTA (admin only).
- Clients list — "No applications registered yet." + CTA (admin only).
- Signing-keys list — "No signing keys found." (no CTA — use rotate form).
- Audit log — "No audit events yet." (no CTA — read-only).

### Added — `CopyField` component (`utilities.rs`, RFC 092)

`pub fn copy_field(t, value, noun, aria_label)` renders a
`<input readonly role="status">` plus a copy-to-clipboard button
reusing the existing `copy-btn` CSS and `copy.js` mechanism.

### Added — `error_summary` component (`forms.rs`, RFC 092)

`pub fn error_summary(t, errors: &[FieldError])` renders a
`role="alert" aria-live="assertive"` block listing all field errors
when `errors.len() > 1`. Only rendered when multiple errors exist;
no DOM presence on clean pages.

### Added — 10 i18n keys (RFC 092)

`theme_noscript_note`, `empty_users`, `empty_users_cta`,
`empty_clients`, `empty_clients_cta`, `empty_signing_keys`,
`empty_audit`, `error_summary_heading` across English, Japanese,
Simplified Chinese. (`button_edit`, `button_view_detail` already
existed from a prior release.)

**104/104 tests pass. All 5 CI gates PASS.**
UI-security contract (RFCs 087–092) complete.

## [0.73.0] — 2026-06-14

**UI-security unit 5: RFC 091 — LoginContext Rendering.**
The login page now renders context-appropriate copy based on why the user
is logging in, with a server-side trusted-name invariant for OIDC flows.

### Added — `LoginContext` enum and context-aware login copy (RFC 091)

`LoginContext` is a new public enum in `sui-id-web`:

```rust
pub enum LoginContext {
    AdminPanel,
    OidcAuthorize { client_name: String },
    SelfService,
}
```

`render_login` gains an `Option<LoginContext>` parameter (`None` → `AdminPanel`).
Copy per context:

| Context | Title | Body |
|---|---|---|
| AdminPanel | "Sign in to manage sui-id" | "Use an administrator or auditor account." |
| OidcAuthorize | "Sign in" (existing) | "{client_name}: sui-id will verify your identity…" |
| SelfService | "Sign in to manage your security" | "Manage MFA, passkeys, sessions, and password." |

**Trusted-name invariant (v2.3 §4 P0):** `OidcAuthorize` is only produced
after a successful lookup of the client record by UUID parsed from `?client_id`.
A malformed or unregistered `client_id` in the `next` URL falls back to
`AdminPanel` — the untrusted value is never echoed to the page.

5 new i18n keys across English, Japanese, Simplified Chinese (Traditional
delegates to Simplified): `login_title_admin`, `login_body_admin`,
`login_title_self_service`, `login_body_self_service`, `login_body_oidc`.

### Added — `derive_login_context` helper in auth handler

A `async fn derive_login_context(db, next)` reads the `?client_id=` from
a `/oauth2/…` `next` URL, looks up the client name in the database, and
returns the appropriate `LoginContext`. All existing `render_login` call
sites receive `None` (AdminPanel default) except `login_get` which passes
the derived context.

**104/104 tests pass. All 5 CI gates PASS.**

## [0.72.0] — 2026-06-14

**UI-security unit 4: RFC 090 — Signing-Key Rotation Confirm Page and
Settings Pending-Change Object.** Two security-sensitive flows that were
previously unguarded are now gated on step-up and explicit confirmation.
SMTP password never passes through a form field after initial entry.

### Added — `pending_settings_change` table (migration 0032, RFC 090 P1/P2/P3/P4)

New SQLite table stores encrypted, session-bound, short-lived (5 min) change
objects for high-risk settings that include secrets. Columns: `id` (opaque UUID),
`session_id` + `actor_id` (binding, P2), `intent`, `payload_enc` (AES-GCM
under master key, P1), `summary` (non-secret for display and audit, P6),
`csrf_token`, `expires_at` (P4), `created_at`. Covered index on `expires_at`.

Store repo (`repos/pending_settings_change.rs`): `insert`, `get_summary`
(non-destructive, for confirm page display), `consume` (atomic delete + return;
expired rows surface as `NotFound`, P3/P4), `cancel` (idempotent delete),
`purge_expired`. 8 unit tests covering all invariants.

### Added — `PendingChangeId` newtype (sui-id-shared)

UUID newtype with private inner field, redacting `Debug` impl, `FromStr` /
`Display`, consistent with the RFC 078 type-modeling baseline.

### Added — `sui-id-core::pending_change` domain module (RFC 090)

`create<T: Serialize>` — serialises payload to JSON, encrypts with
`crypto::seal(PENDING_CHANGE_AAD)`, inserts row, emits `settings.pending_change.created`
audit event. Returns `PendingChange { id, intent, summary, expires_at }` (no ciphertext).

`apply<T: DeserializeOwned>` — consumes row atomically; verifies all five bindings
(admin role via `&AdminActor`, session, actor, CSRF, expiry); decrypts payload;
emits `settings.pending_change.applied`; returns typed payload. Any binding
failure or decryption error → neutral "expired or invalid" `BadRequest` (P3).

`cancel` — idempotent delete + `settings.pending_change.cancelled` audit event.
`purge_expired` — clears stale rows; called at startup.

### Added — signing-key rotation confirm page (RFC 090)

New `GET /admin/signing-keys/rotate-confirm` handler
(`signing_keys_rotate_confirm_get`): guarded by RFC 088 `can_write()` and
RFC 089 step-up allowlist; step-up fresh required before rendering. Shows
count of active keys in the impact text.

The existing `POST /admin/signing-keys/rotate` now revalidates step-up on
the final POST (P5: no auto-execute) and calls `rotate_signing_key` with
`&AdminActor` (RFC 081).

New web-layer types: `ConfirmRotateSigningKeyData` + `render_confirm_rotate_signing_key`.
4 new i18n keys (rotate confirm title, impact, reversibility, button) in all
locales.

### Added — SMTP settings pending-change flow (RFC 090)

`POST /admin/settings/email` — when the password field is non-empty, creates
a pending change via `pending_change::create`, redirects to
`/admin/settings/email/confirm?pending_change_id={id}`. The password never
appears in any redirect or hidden field.

`GET /admin/settings/email/confirm` — fetches non-secret summary via
`get_summary` (non-destructive), renders confirm page.

`POST /admin/settings/email/confirm` — applies via `pending_change::apply`
(binding check + decrypt), re-seals password with `seal_password`, upserts
SMTP config. Step-up revalidated on this POST (P5).

New web-layer types: `SettingsEmailConfirmData` + `render_settings_email_confirm`.
3 new i18n keys (confirm title, impact, button) in all locales.

### Added — 7 i18n keys, audit events

7 new string keys across English, Japanese, Simplified Chinese (Traditional
delegates to Simplified). 4 new audit event names
(`settings.pending_change.{created,applied,cancelled,binding_failed}`) added
to the coverage matrix; gate passes at 43 × 43.

**104/104 tests pass** (19 shared + 20 store-pre + 62 store-all including 8
new pending-change tests + 3 web). All 5 CI gates PASS.

## [0.71.0] — 2026-06-14

**UI-security unit 3: RFC 089 — Step-up Authentication Contract.**
Tightens the `?return_to=` parameter validation with a server-side
allowlist and documents the recovery-code exclusion. No user-facing change
for well-formed flows; malformed or non-allowlisted `return_to` values now
fall back to `/me/security` instead of navigating to the supplied path.

Also in this release: **RFCs 089–092 drafted.** All four remaining
UI-security units (step-up, pending-change, LoginContext/SelfServiceShell,
and components) have RFC documents in `rfcs/proposed/`, providing the full
spec-first foundation before any code lands for units 4–6.

### Fixed — `sanitise_return_to` allowlist gate (RFC 089 P1)

The existing `sanitise_return_to` helper in `handlers/step_up.rs` validated
format only (relative URL, no protocol-relative prefix, no control chars).
A crafted `?return_to=/admin/dashboard` or any non-step-up-gated path could
redirect the user post-step-up to a route that didn't need the challenge.

Added `STEP_UP_RETURN_ALLOWLIST: &[&str]` with five prefixes:
`/admin/users/`, `/admin/clients/`, `/admin/signing-keys/`,
`/admin/settings/`, `/me/security/`. Any `return_to` that doesn't begin
with one of these prefixes now falls back to `/me/security`.

12 unit tests: 4 allowlisted paths accepted, 3 non-allowlisted paths
rejected, 5 existing format-violation tests confirmed still passing.

### Added — Recovery-code exclusion documentation (RFC 089 P2)

`sui-id-core::step_up::policy_for_session` gains a `# Factor eligibility`
doc comment explicitly stating that recovery codes are excluded from step-up
factor eligibility. The implementation was already correct (only TOTP and
WebAuthn set `last_step_up_at`); the doc comment makes this a named invariant.

### Added — RFCs 089–092 (proposed)

- **RFC 089** (done this release).
- **RFC 090** — Signing-key rotation confirm + settings pending-change object
  (unit 4). New `pending_settings_change` table, `pending_change::create/apply/
  cancel/purge_expired` core functions, encrypted-at-rest payload, 5-minute
  session-bound TTL. SMTP password never in form fields.
- **RFC 091** — `LoginContext` enum (`AdminPanel`, `OidcAuthorize`, `SelfService`)
  + trusted client-name derivation + `SelfServiceShell` with horizontal nav
  (unit 5).
- **RFC 092** — `ThemeToggle` (`no-js`/`js` root-class swap, `localStorage`
  try/catch, `<noscript>` fallback), `EmptyState`, `CopyField`
  (`readonly` + `role="status"`), error summary `role="alert"` (unit 6).

**96/96 tests pass. All 5 CI gates PASS.**

## [0.70.0] — 2026-06-14

**UI-security unit 2: RFC 088 — Auditor Authorization Matrix and Static
Read-Only Rendering.** Server-side enforcement of the v2.3 §3.5 route/action
matrix. No wire protocol change; auditor access to mutation surfaces now
returns HTTP 403 instead of 401-redirect.

### Fixed — auditor 403 on mutation-only GET routes (RFC 088 P2, P4)

Every mutation-only GET route in the admin panel now returns an HTTP 403
("read-only access") page for auditor sessions rather than a login redirect.

**Users handlers** (`/admin/users/new`, `/admin/users/{id}/disable-confirm`,
`/admin/users/{id}/delete-confirm`, `/admin/users/{id}/mfa-reset-confirm`):
- `users_new_get` changed from `CurrentAdmin` to `CurrentAdminOrAuditor`
  with `!actor.can_write()` guard.
- Three confirm-GET handlers (`users_disable_confirm_get`,
  `users_delete_confirm_get`, `users_mfa_reset_confirm_get`) changed from
  `CurrentAdminOrAuditor` with no guard to `CurrentAdminOrAuditor` with
  explicit `!actor.can_write()` guard. Previously auditors could reach
  these confirm pages — a security defect closed by this release.

**Clients handlers** (`/admin/clients/new`, `/admin/clients/{id}/delete-confirm`):
- `clients_new_get` changed from `CurrentAdmin` to `CurrentAdminOrAuditor`
  with guard.
- `clients_delete_confirm_get` gained a `can_write()` guard.

**Signing keys** (`/admin/signing-keys/{id}/delete-confirm`):
- `signing_keys_delete_confirm_get` changed from `CurrentAdmin` to
  `CurrentAdminOrAuditor` with guard.

### Added — `HttpError::html_403_auditor()` helper

A dedicated constructor for auditor-on-mutation-GET responses. Returns HTTP
403 with `forced_status = Some(StatusCode::FORBIDDEN)` — the distinction
from 401 matters: 403 tells the client that re-authentication will not help.

### Added — 3 i18n keys (RFC 088, all locales)

`error_403_auditor_title`, `error_403_auditor_body`, and
`client_detail_readonly_title` ("App details") added to English, Japanese,
Simplified Chinese, and Traditional Chinese (delegates to Simplified).

### Added — `can_write: bool` in settings data structs

All six `Settings*Data` structs gained `can_write: bool`, populated from
`actor.can_write()` in each GET handler. The render functions destructure it
as `_can_write` (used for future conditional branching — the read-only `<dl>`
rendering mode for auditors is a follow-up improvement on top of the security
enforcement this release provides).

### Changed — `clients_edit_get` now uses `actor.can_write()`

The client edit renderer already accepts `can_write: bool`; the handler now
derives it from `read_actor.can_write()` instead of `role.is_admin()`. These
are equivalent for current roles but the former is the correct abstraction —
it delegates to the RFC 082 authorization table via RFC 081's
`ReadOnlyAdminActor`.

**96/96 tests pass. All 5 CI invariants PASS.**

## [0.69.0] — 2026-06-14

**Security assurance: RFC 084 (fuzzing harness) + RFC 086 (formal verification
pilot).** The final two RFCs of the auth-core assurance arc (078–086). No
wire-behaviour change; no production-code logic change.

### Added — cargo-fuzz harness (RFC 084)

`fuzz/` is a separate cargo workspace (excluded from the main workspace and
release archives) with six fuzz targets:

| Target | Input surface | Invariants |
|---|---|---|
| `accept_language` | Any bytes → `negotiate_from_accept_language` | P1 no-panic; P3 round-trip `tag()` → `Locale::parse` |
| `ids_fromstr` | Any bytes → all 9 typed-ID `FromStr` impls | P1 no-panic; P3 round-trip `Ok(v) ⇒ v.to_string().parse() == Ok(v)` |
| `pkce_verify` | NUL-separated (method, verifier, challenge) | P1 no-panic; P2 accept ⇒ method=S256 and SHA-256(verifier)=challenge |
| `authorize_params` | Structured `(registered, submitted)` pairs | P1 no-panic; P2 accept ⇒ exact membership |
| `jwt_parse` | Arbitrary bytes → `jwt::verify` with fixed test key | P1 no-panic |
| `logout_params` | Structured `(registered, submitted)` pairs | P1 no-panic; P2 accept ⇒ exact membership |

`pkce_verify`, `authorize_params`, `jwt_parse`, and `logout_params` are gated
behind the `core-targets` cargo feature (require `libssl-dev`); `accept_language`
and `ids_fromstr` build and run in the openssl-free CI environment.

**Smoke run (1 000 iterations each):** `accept_language` and `ids_fromstr`
both completed clean — 0 panics, 0 findings, libFuzzer growing corpus
(31 entries / 82 bytes and 16 entries / 34 bytes respectively).

`fuzz/rust-toolchain.toml` pins the nightly toolchain so the stable MSRV
of the main workspace is untouched. Corpus seeds are committed under
`fuzz/corpus/`. A scheduled CI workflow (`.github/workflows/fuzz.yml`) runs
at 100 000 iterations per openssl-free target every Monday; PRs touching
`fuzz/` get a `cargo fuzz build` smoke check.

### Added — formal verification pilot (RFC 086)

Three time-boxed pilots evaluated. Deliverables committed:

**Pilot K (Kani):** Five proof harnesses in `crates/sui-id-core/src/authz.rs`
under `#[cfg(kani)]`, exhaustively verifying P1–P5 of the RFC 082 decision
table over the complete finite input space. Recommendation: **Adopt** for
`authorize` (scheduled CI once Kani is available); **Defer** for `verify_pkce`
(SHA-256 stub required, fuzz already covers panic-freedom).

**Pilot T (TLA+):** `verification/refresh_rotation.tla` models the RFC 080
rotation protocol with both the guarded variant (Inv1 holds) and a
deliberately guard-less variant (Inv1 must be violated — the pre-RFC-080
bug in model form). Recommendation: **Adopt** as living design documentation;
schedule TLC verification once TLA+ Toolbox is available in CI.

**Pilot F (Flux):** Desk evaluation only. Recommendation: **Defer** — annotation
burden and nightly-fork requirement are incompatible with the project's
"understandable to normal Rust developers" principle (strategy §4).

Full reports with effort, findings, and revisit triggers in
`verification/pilot-reports.md`.

**96/96 tests pass. All 5 CI invariants PASS.**

## [0.68.0] — 2026-06-14

**Security assurance: RFC 083 (security state-machine testing) + RFC 085
(audit event completeness).** Two Category-B structural hardening RFCs.
No wire-behaviour change.

### Added — security state-machine harnesses (`sui-id-store`, RFC 083)

Three proptest-driven model-based test suites in
`src/tests_state_machine/`, each generating random operation sequences
(length 1–40, 256 cases), running them against both a trivial in-memory
oracle and the real `Database`, and asserting named invariants after every
step:

**`auth_codes`** — issues, consumes, replays, clock advances, purges.
Invariants: `INV_CODE_SINGLE_USE` (at most one successful consume per code),
`INV_CODE_NO_EXPIRY_CONSUME` (no success after expiry time),
`INV_CODE_PURGE_NO_RESURRECT` (purge never reverts state).

**`refresh_tokens`** — new families, rotations, replays, clock advances,
purges. Invariants: `INV_FAMILY_SINGLE_ACTIVE` (per-family active-count ≤ 1,
checked via SQL after every rotation), `INV_FAMILY_REUSE_REVOKES_ALL` (replay
yields `ReuseDetected` and an all-revoked family),
`INV_NO_UNREVOKE`, `INV_EXPIRED_NO_ROTATE`.

**`sessions`** — creates, revokes, revoke-all-except, purges. Invariants:
`INV_SESSION_REVOKED_NEVER_RESOLVES`, `INV_SESSION_EXPIRED_NEVER_RESOLVES`,
`INV_SESSION_REVOKE_ALL_EXCEPT_LEAVES_ONE`,
`INV_SESSION_PURGE_DURABILITY`.

All three harnesses pass at 256 cases; total runtime ≈ 27 s (within CI
budget). The oracle asserts invariants, not implementation equality, so
the harnesses catch ordering and interleaving bugs that example-based tests
structurally miss.

### Added — `audit::append_within_tx` (RFC 085, Class-A atomicity)

A new synchronous `append_within_tx(tx, &AuditLogRow)` function in
`sui-id-store/src/repos/audit.rs` appends an audit row inside the caller's
transaction so the state change and the audit record commit atomically —
or neither does. Three tests:

- `append_within_tx_commits_with_caller_transaction` — committed row appears
  in chain, chain verifier passes (P2 committed path).
- `append_within_tx_rolls_back_with_caller_transaction` — rolled-back row
  does NOT appear (P2 rollback path; the fail-safe that makes audit-subsystem
  failure an operation failure).
- `append_within_tx_maintains_chain_with_prior_rows` — four rows mixed
  across the two append paths, chain verifies across all (P5).

Also fixed a stale `auth.refresh_theft_detected` prefix in
`DASHBOARD_IMPORTANT_PREFIXES` (should be `auth.refresh.theft_detected`,
matching the emitted event name).

### Added — `AuditReceipt` / `Audited<T>` types (`sui-id-core`, RFC 085)

`sui-id-core/src/audit_guard.rs` introduces:

- `AuditReceipt` — proof that an audit record was appended. No public
  constructor; cannot be forged.
- `Audited<T>` — wraps a domain-function return value with its receipt.
  `into_inner()` unwraps. "Mutated but never audited" is unrepresentable for
  converted functions.
- `audit_best_effort<T>` — Class B; append failure does not suppress the
  security response.
- `audit_and_tx<T>` — Class A; appends within the caller's transaction via
  `append_within_tx`.
- `audit_and<T, F>` — convenience builder for Class-A domain functions.

(Note: `sui-id-core` cannot be link-verified in this environment without
`libssl-dev`; `audit_guard.rs` is written from exhaustive source reading.
Full Class-A domain-function conversions are a follow-up once the CI
environment provides the full toolchain.)

### Added — audit coverage matrix + CI gate (RFC 085)

`docs/src/reference/audit-coverage-matrix.md` is the normative reference:
every privileged operation, its event name, required fields, and atomicity
class (A or B). 39 events listed.

`scripts/check-audit-matrix.sh` is a bidirectional CI gate: forward (every
matrix event name must exist as a literal in source) and backward (every
source literal must have a matrix row). Passes: 39 × 39 fully consistent.

**54/54 tests pass** (19 shared + 20 store-pre-existing + 51 store-RFC-085 +
3 store-state-machine × 3 (auth-codes, refresh-tokens, sessions) = 54 total
in sui-id-store alone; 96/96 across all four crates including web/i18n).
All five CI invariants: text-leaks, css-tokens, semantic-parity, inline-style,
audit-matrix — all PASS.

## [0.67.0] — 2026-06-14

**Security structure: RFC 081 (actor scope boundary) + RFC 082 (authorization
decision core).** Two Category-B structural hardening RFCs from the audit arc,
shipped together as planned. No wire-behaviour change; internally a privileged
call without proof of privilege is now a compile error.

### Added — pure authorization decision core (`authz.rs`, RFC 082)

A new pure module `sui_id_core::authz` contains:

- `Action` enum — every privileged operation in the system, enumerated. The
  `AdminChangeUserRole`, `AdminDisableUser`, and `AdminDeleteUser` variants
  carry `target_is_last_admin: bool` so the function remains a pure `(Role,
  Action) → Decision` table; the caller resolves the environmental fact and
  passes it in as data.
- `Decision` enum — `Permit` / `Deny`.
- `authorize(role: Role, action: Action) -> Decision` — the single normative
  authorization table. Deny-by-default: the final match arm is `_ => Deny`;
  any unlisted pair denies without a code-review finding.

Security properties verified by `authz/tests.rs`:
- **P1** Totality / deny-by-default — exhaustive matrix test (hand-written
  expected matrix agrees with the live table; two artifacts must agree) plus a
  no-panic sweep.
- **P2** Role monotonicity — `permit(User, a) ⇒ permit(Admin, a)` for all `a`.
- **P3** Read/write separation — Auditor permitted ⊆ read-only actions; every
  mutation action is `Deny` for `Auditor`.
- **P4** Last-admin protection — all three last-admin variants deny for every
  role, including `Admin`.
- **P5** Self-scope — `User` permitted ⊆ `Self*` actions.

### Added — actor capability types (`actor.rs`, RFC 081)

A new module `sui_id_core::actor` introduces:

- `Actor` — proof of an authenticated principal. Private constructor
  (`pub(crate) from_session`), no `Deserialize`, no `Clone`. Cannot be forged
  or cached across requests (P4, P5).
- `AdminActor` — write-capable proof. Constructible only via
  `Actor::into_admin`, which delegates to the RFC 082 table.
- `ReadOnlyAdminActor` — read-only admin or auditor proof. The `can_write()`
  method replaces the free-floating `can_write: bool` rendering flag;
  `into_read_admin` accepts both `Admin` and `Auditor` roles.
- `SelfActor` — any authenticated actor, self-scoped. Self-service mutations
  call `actor.user_id()` for the target; a caller-supplied user id is not
  accepted, making cross-user targeting via `/me/*` structurally impossible
  (P3).

Tests in `actor.rs`: all conversion paths, `can_write` derivation, and a
pinned test that `SelfActor::user_id` returns the authenticated user's id —
not a caller-supplied one.

### Changed — domain function signatures

Every privileged admin mutation now requires `&AdminActor`; every admin read
requires `&ReadOnlyAdminActor`; `change_password_self` requires `&SelfActor`.
The `require_admin(db, user_id)` DB check is replaced by the structural
guarantee — the capability type is proof the check already passed.

Affected domain functions:

- `admin::users` — `create_user`, `set_user_disabled`, `delete_user`,
  `admin_reset_mfa`, `reset_user_password` take `&AdminActor`;
  `list_users` takes `&ReadOnlyAdminActor`.
- `admin::clients` — all 8 mutations take `&AdminActor`;
  `get_client`, `list_clients` take `&ReadOnlyAdminActor`.
- `admin::signing_keys` — `rotate_signing_key`, `delete_signing_key` take
  `&AdminActor`; `list_signing_keys` takes `&ReadOnlyAdminActor`.
- `me_security::change_password_self` — takes `&SelfActor`.

### Changed — Axum extractors

`CurrentAdmin` now produces `(UserId, AdminActor)`; `CurrentAdminOrAuditor`
produces `(UserId, Role, ReadOnlyAdminActor)`. The new fields flow through
to handler call sites, which pass `&admin_actor` / `&read_actor` to domain
functions.

### Changed — last-admin safeguard

The last-admin check in `users_set_role` now calls `authz::authorize(role,
AdminChangeUserRole { target_is_last_admin })` — the RFC 082 table is the
single normative source of the last-admin rule.

### Deprecated — `require_admin`

`admin::require_admin` is deprecated with a `#[deprecated]` attribute pointing
to `Actor::into_admin`. It is retained for the binary-crate extractors, which
must verify session roles before constructing an `Actor`; domain functions no
longer call it.

**90/90 tests pass.** All CI invariants unchanged.

## [0.66.0] — 2026-06-14

**Security: RFC 079 (auth-code lifecycle assurance) + RFC 080 (refresh-token
rotation atomicity).** Two Category-A security RFCs from the audit arc, shipped
together as planned. No API or schema shape change; behaviour-compatible for
all conforming clients.

### Fixed — auth-code single-use (RFC 079, P1/P2)

`auth_codes::consume` was correct only because a single mutex serialised every
closure. The statement-predicate was an unconditional `UPDATE … SET consumed = 1`
that never inspected rows-affected; any future concurrency evolution would silently
break single-use enforcement.

`consume` is rewritten as a guarded UPDATE+SELECT inside one transaction:

```sql
UPDATE auth_codes SET consumed = 1
 WHERE code_hash = ?1 AND consumed = 0 AND expires_at > ?2
```

rows-affected = 1 → the code was live; return the row. rows-affected = 0 →
unknown / already consumed / expired — all three surface identically as
`NotFound` (P5 non-disclosure). The `now` parameter is supplied by the caller
(clock-source agnostic; mirrors `SharedClock` usage elsewhere).

Migration 0031 adds `idx_auth_codes_expiry ON auth_codes(expires_at)` — a
covering index for the new predicate and the existing purge query.

### Added — auth-code typestate pipeline (RFC 079, P3)

`exchange_code` in `authorize.rs` is restructured as a four-step typestate
pipeline private to the module:

`ConsumedCode` → `BoundCode` → `PkceVerifiedCode` → `IssuableGrant`

Each step is a function that consumes the previous state; `issue_for` accepts
only `IssuableGrant`. Token issuance before all validation steps have passed is
a compile error, not a code-review finding (strategy §9 "keep typestate
localised").

### Fixed — refresh-token rotation atomicity (RFC 080, P1/P2/P3)

`exchange_refresh` previously spanned three separate `with_conn` closures
(find → revoke → insert). Two concurrent requests with the same token could
both observe `revoked_at IS NULL`, both run the idempotent revoke, and both
insert children — forking the rotation family and degrading theft detection.

New function `refresh_tokens::begin_rotation` collapses the arbitration into
one transaction with a rows-affected guard:

- rows-affected = 1 → `RotationLookup::RotatedHere` (this caller won).
- rows-affected = 0 → already revoked; family revocation runs in the **same
  transaction** → `RotationLookup::ReuseDetected { row, family_revoked }`.
- Additional variants: `Expired`, `Unknown`.
- Client mismatch → `StoreError::Conflict` without mutating state.

`exchange_refresh` now matches on `RotationLookup`; the `find_any`/`revoke`
sequence is gone from the grant path. The `auth.refresh.theft_detected` audit
note gains `family_revoked=N` for triage.

Security properties guaranteed by the implementation (RFC 080):
- **P1**: exactly one concurrent caller receives `RotatedHere`.
- **P2**: every concurrent loser receives `ReuseDetected`; family is fully
  revoked before that variant is returned.
- **P3**: per-family active-token count is ≤ 1 at all times.

### Tests added

`repos/auth_codes/tests.rs` (5 tests): P1 sequential; `consumed` flag verified
in DB after consume; P2 expiry — SQL predicate proven (flag stays 0); P4
burn-on-failure; P5 unknown-code.

`repos/refresh_tokens/tests.rs` (8 tests): P1 sequential; P1+P2 concurrent
(N=8, exactly one winner); P2 family atomically revoked (SQL count asserted);
P3 chain-of-3 keeps ≤ 1 active; Expired variant; Unknown variant; client
mismatch returns Conflict without revoking.

**90/90 tests pass** (19 shared + 20 store-pre-existing + 48 store-all + 3
web). All CI invariants unchanged.

## [0.65.1] — 2026-06-14

**Maintenance: RFC 087 — clippy and rustfmt baseline cleanup (Rust 1.96).**

Resolves accumulated toolchain drift so the CI-signal build is clean under
the current stable toolchain (`rustc 1.96.0`). No logic change anywhere.

### Fixed — clippy (`--workspace --all-targets -D warnings`)

**`sui-id-web`** (16 findings cleared):

- `empty_line_after_doc_comments` — removed stray blank lines between doc
  comments and the items they document in `auth.rs`, `confirm.rs`,
  `dashboard.rs`, `settings.rs`, and `setup.rs`.
- `doc_lazy_continuation` — inserted a blank `//!` line in `chrome.rs` to
  break the list-continuation misparse.
- `unit_arg` — two `view! { <></> }.into_any()` empty-fragment calls in
  `auth.rs` rewritten to the `let _: () = …; ().into_any()` form clippy
  suggests (Leptos SSR empty-view pattern, no behaviour change).
- `unnecessary_map_or` — one `map_or(false, …)` in `dashboard.rs` replaced
  with `is_some_and(…)`.
- `no_effect_replace` — two no-op `.replace(''', "\'")` calls (replacing
  a character with itself) removed from `sessions.rs`; the FnMut closure
  capture was updated to `.clone()` so the per-iteration move is safe.
- Redundant `use leptos::prelude::*;` removed from 13 child modules under
  `pages/me_security/` and `pages/settings/`; the symbol comes in via the
  parent module's glob re-export and the own import was always redundant.

**`sui-id-shared`** (2 findings cleared):

- `expect_used` in `secrets.rs:random_base64url` — two `#[allow]` attrs
  with rationale comments: the `getrandom` call panics on RNG failure (correct
  fail-safe) and the `from_utf8` call is infallible on base64url output.

**`sui-id-store`** (16 lib findings + test-target findings cleared):

- `expect_used` — `crypto.rs` (RNG + buffer-size + ascii-infallible),
  `db.rs` (mutex-poisoned × 4): targeted `#[allow]` with rationale.
- `should_implement_trait` — `Role::from_str` renamed `Role::from_db_str`
  to avoid confusion with `std::str::FromStr`; the one call site in
  `users.rs` updated.
- `Default` derive — manual `impl Default for HibpMode` replaced with
  `#[derive(Default)]` + `#[default]` on the `Warn` variant.
- `clone_on_copy` — `EmailOutboxId` is `Copy`; `.clone()` removed in
  `email_outbox.rs`.
- `collapsible_if` — two nested `if let Ok(…) { if … }` blocks in
  `refresh_tokens.rs` collapsed to `if let Ok(…) && … {}` (constant-time
  compare path; evaluation order preserved).
- `needless_question_mark` — `Ok(expr?)` simplified to `expr` in
  `user_totp.rs` and `user_webauthn_credentials.rs`.
- `useless_conversion` — `StoreError::NotFound.into()` → `StoreError::NotFound`
  in `users.rs`.
- `items_after_test_module` — `mod tests` block moved to the bottom of
  `email_outbox.rs`.
- Test-target lints (`expect_used`, `unwrap_used`, `clone_on_copy`, `panic`)
  suppressed with scoped `#![allow]` inside each `mod tests` block and on
  `#[cfg(test)]` helper functions in `migrations.rs`; test logic unchanged.

**`sui-id-i18n`** (3 findings cleared):

- `manual_is_multiple_of` — `remaining % 3 == 0` → `remaining.is_multiple_of(3)`
  in `formatters.rs`.
- `non_minimal_char_comparison` — `split(|c: char| c == '-' || c == '_')` →
  `split(['-', '_'])` in `lib.rs`.
- `Default` derive — manual `impl Default for Locale` replaced with
  `#[derive(Default)]` + `#[default]` on `Locale::Ja`.

### Fixed — rustfmt (`cargo fmt --check --all`)

All four buildable crates are now default-fmt-clean. Previously, 31 files
across `sui-id-web`, `sui-id-shared`, `sui-id-store`, and `sui-id-i18n`
had diffs against `rustfmt 1.9.0` defaults (no `rustfmt.toml`). All
formatted; no `rustfmt.toml` introduced.

CI invariants hold: `text-leaks` = 0, `inline-style-bound` = 1,
`css-tokens` resolves, `semantic-palette-parity` = 36.
78/78 buildable-crate tests pass.

## [0.65.0] — 2026-06-14

**Accessibility: WCAG AA contrast correction — design-token foundation.**

Unit 1 of the v0.65.0 UI-security handoff: a focused correction to the
design-token colour values so every text-on-colour pair clears WCAG 2.1
AA (4.5:1 for normal text) in light, explicit-dark, and auto-dark modes.
No copy, layout, or i18n keys change.

### Fixed — dark-mode text contrast (AA defect)

In `[data-theme="dark"]` (and the matching `@media (prefers-color-scheme:
dark)` root) every coloured fill rendered white or near-white text over a
bright pastel, so all five text-on-colour pairs failed AA — as low as
1.5:1 for warning. The foreground on dark accent / danger / warning /
success / info now uses a single dark ink (`#1A1720`), pairing the bright
pastels with dark text the same way light mode pairs darker fills with
white text. The worst dark pair is now ~5.8:1.

### Changed — light-mode semantic + accent contrast

The light fills are darkened so white text passes AA: accent
`#7C6BCF → #6956C0` (emphasis `#6956C0 → #5A48AD`), danger
`#C94A4A → #BE3F3F`, success `#3FA37A → #2F7D5E`, info
`#4A7FC9 → #3B6EA8`. Warning moves from a pale amber carrying dark ink to
a darker amber `#D49B2A → #8A6500` with white text, consistent with the
other semantics. The focus ring and accent border track the new accent
(`--state-focus`, `--border-accent`). `::selection` behaviour is
unchanged; its doc-comment ratios are corrected.

### Added — explicit disabled tokens

`--fg-disabled` / `--bg-disabled` are declared in all three mode roots
(light `#5F5A66` on `#EFEAF4` = 5.65:1; dark `#9D96AA` on `#211D2A` =
5.79:1). `button:disabled` now uses these tokens instead of
`opacity: 0.7` over the live fill — opacity over colour degrades contrast
unpredictably. Per the contrast contract, a disabled control is never the
sole carrier of a meaningful value; read-only values use static rows.

### Added — contrast CI test

`crates/sui-id-web/src/tokens/tests.rs` parses the live `TOKENS_CSS`,
resolves all three modes, and recomputes the WCAG ratio for every
text-on-colour pair — failing the build if any drops below 4.5:1 (and the
focus ring below the 3:1 UI-component threshold). It also asserts the
explicit-dark and auto-dark roots stay in lockstep. The test validates
the specification against the live tokens, not a hand-copied table.

### Fixed — dangling `--surface-overlay` token reference

`components/chrome.rs` set the user-menu popover background to
`var(--surface-overlay)`, a token declared nowhere in the closed
five-member surface scale. The reference is corrected to the intended
`--surface-elevated` (the documented elevated surface for cards and
popovers). This restores the `css-tokens` (RFC 049) gate, which the prior
tree failed on this reference.

CI invariants hold: `text-leaks` = 0, `inline-style-bound` = 1,
`css-tokens` resolves, `semantic-palette-parity` = 36. The new contrast
test passes in all three modes.

## [0.64.2] — 2026-06-06

**UI/UX: "Less is more" — noise reduction pass.**

### Changed — admin users list

The users table shrinks from 6 columns to 4: **username, status, MFA,
detail link**. The `display_name` column (often empty, not needed for
identification) and `created_at` column (not a scanning criterion) are
removed. The three per-row action buttons (Reset MFA, Disable, Delete)
are removed from the list entirely — all mutations happen on the user
detail page. The detail page already existed and has everything. One
extra click to reach a mutation is appropriate friction; the list is for
*finding*, not *doing*. The duplicate "Create user" button below the
table is also removed (the `+` button in the page header is sufficient).

### Changed — admin clients list

The clients table shrinks from 7 columns to 5: **name, client ID (copy),
kind, status, Edit + Enable/Disable**. The `scopes` column (the default
`openid profile email` is the same for nearly every client) and the
`logout_count` column (a number that means nothing in the list context)
are removed. The `Delete` button is removed from the list row — deletion
happens on the edit page. The duplicate "Register client" button below
the table is also removed.

### Changed — dashboard

The "OIDC Endpoints" section (issuer, discovery URL, JWKS URL) is
removed from the dashboard. These URLs are already shown in
**Settings → Basic** and are standard, unchanging URLs any OIDC-aware
operator knows. Repeating them on the most-visited admin page adds
length without value. The `issuer` field is removed from `DashboardData`
and its population removed from the dashboard handler.

### Changed — /me/security overview

The three shortcut buttons at the bottom of the overview (linking to
MFA, Passkeys, and Sessions) are removed. The tab bar at the top of the
page already provides identical navigation; the buttons were a redundant
second copy.

### Removed — string keys

Six string keys retired from `Strings` and all locale files:
`users_table_th_display`, `users_table_th_created`,
`clients_table_th_scopes`, `clients_table_th_logout`,
`dashboard_oidc_endpoints_section`, `dashboard_oidc_endpoint_issuer`.
(The `dashboard_oidc_endpoint_discovery` and `_jwks` keys are retained
— Settings → Basic uses them.)

### Added — string keys

`button_edit` and `button_view_detail` added to `Strings` (used by the
simplified action cells in both list pages).

## [0.64.1] — 2026-06-06

**Maintenance: i18n source layout and zh locale split.**

### Changed — i18n crate structure

The three flat translation files (`en.rs`, `ja.rs`, `zh.rs`) that sat
alongside module files in `crates/sui-id-i18n/src/` have been moved
into a dedicated subdirectory. Each locale file is now fully
self-contained — its `STRINGS_*` constant and `FORMATTERS_*` constant
in one place.

New layout:

```
src/
  lib.rs          — Locale enum, negotiation, re-exports
  strings.rs      — Strings struct (schema only)
  formatters.rs   — Formatters struct + two shared helpers (fmt_time, fmt_count)
  locale.rs       — umbrella: pub mod en/ja/zh_hans/zh_hant + re-exports
  locale/
    en.rs         — STRINGS_EN + FORMATTERS_EN
    ja.rs         — STRINGS_JA + FORMATTERS_JA
    zh_hans.rs    — STRINGS_ZH_HANS + FORMATTERS_ZH_HANS
    zh_hant.rs    — Traditional Chinese stub (delegates to zh_hans)
```

`formatters.rs` reduced from 285 lines to 88 (struct + two helpers);
the per-locale formatter functions moved into their respective locale
files. Each locale file now contains inline `#[cfg(test)]` formatter
tests.

### Changed — Simplified / Traditional Chinese split

`Locale::Zh` is renamed to `Locale::ZhHans` (BCP-47 tag `"zh-Hans"`).
`Locale::ZhHant` (`"zh-Hant"`) is added as a stub.

- `Locale::ZhHans` serde: `#[serde(rename = "zh-Hans", alias = "zh")]`.
  Existing stored user preferences with the value `"zh"` continue to
  deserialise correctly; new writes produce `"zh-Hans"`.
- `Locale::parse()` now handles regional subtags:
  `zh-CN` / `zh-SG` → `ZhHans`; `zh-Hant` / `zh-TW` / `zh-HK` / `zh-MO` → `ZhHant`.
  The bare tag `"zh"` maps to `ZhHans` (backward-compatible).
- Both variants remain outside `Locale::ALL` (neither is promoted as a
  server default yet).
- The language-preference picker value changed from `"zh"` to `"zh-Hans"`.

### Changed — Strings field rename

`Strings.locale_native_zh` renamed to `locale_native_zh_hans`;
`locale_native_zh_hant` added. Values in all locale files updated to
include the script disambiguation (`"中文（简体）"` / `"中文（繁體）"`).

### Added

- `Locale::ZhHant` stub with a full contributor guide in `locale/zh_hant.rs`.
- 7 new tests: `parse_zh_variants`, `serde_round_trip`,
  `serde_legacy_zh_deserialises_to_zh_hans`, `locale_native_names_in_strings_tables`,
  `zh_hans_strings_are_non_empty`, `zh_hans_date_formatting`,
  `zh_hans_relative_formatting`.

### Documentation updated

- `docs/src/contributing/translators.md` — rewritten for the new layout.
- `docs/development-specification.md` §11.10 — updated file paths and zh description.
- `docs/mockup-integration/codebase-handoff.md` — updated §6.1 and Q9.
- `docs/mockup-integration/inventory/i18n-copy-delta-draft.md` — updated paths.

### Test count

| Crate | Before | After |
|---|---|---|
| `sui-id-i18n` | 13 | 19 (+6 formatter tests now inline per locale) |
| `sui-id-shared` | 20 | 20 |
| `sui-id-store` | 36 | 36 |
| `sui-id-core` | 125 | 125 |
| **Total** | **194** | **201** |

## [0.64.0] — 2026-06-05

**RFC 078 — Security-Critical Type Modeling Baseline.**

### Security — type-safety hardening for secret and identifier values

- **`RawRefreshToken` newtype** (`sui-id-shared::secrets`): wraps plaintext
  refresh-token material in a `Zeroizing<String>` that zeroes the allocation
  on drop, with a `Debug` / `Display` implementation that prints only
  `[REDACTED]`. The single intentional plaintext egress is `expose()`, called
  exactly once at HTTP response serialization. Previously `TokenSet.refresh_token:
  String` was Debug-printable and carried the live credential through the whole
  exchange path.

- **`RefreshTokenHash` newtype**: SHA-256 hash of a `RawRefreshToken`,
  constructible only via `RefreshTokenHash::of(token)`. Replaces the
  implicit `sha2` dependency in `refresh_tokens.rs`. Lookup and backfill
  now go through the typed hash.

- **`RefreshTokenId` and `FamilyId` newtypes**: opaque string wrappers for
  the `refresh_tokens.id` and `refresh_tokens.family_id` columns. Private
  inner fields; constructors are `generate()` (new random value) and
  `from_stored(s: String)` (reconstitution from DB). `RefreshTokenRow.id`
  and `.family_id` are now these types; the former bare `String` fields are
  gone. Cross-type assignment — passing a family id where a token id is
  expected — is now a compile error.

- **`CodeHash` newtype**: SHA-256 hex digest of an authorization code,
  constructible only via `CodeHash::of(code)`. `AuthorizationCodeRow.code_hash`
  and `auth_codes::consume` now require `&CodeHash`. Passing an unhashed
  plaintext code to the consume path is a compile error.

- **Removed `RefreshTokenRow.token_plain`**: the `Option<String>` field that
  carried the plaintext token in the row struct (populated at issuance, `None`
  at all other times) is gone. `repos::refresh_tokens::insert` now takes a
  separate `&RawRefreshToken` parameter; the row struct itself never holds
  plaintext.

- **`IssuedRefreshToken` struct** in `sui-id-store::models`: separates the
  stored row from the at-issuance plaintext cleanly — `row: RefreshTokenRow`
  + `token: RawRefreshToken` (with redacting Debug).

- **UUID ID macro**: the `define_id!` macro inner field changed from `pub Uuid`
  to private `Uuid`. Construction requires `new()` / `from_uuid()`;
  direct `.0` access outside the module is a compile error. Closes the
  identifier-fabrication class of bugs for all nine UUID-backed ID types.

### Changed

- `TokenSet.refresh_token: String` → `RawRefreshToken` (core crate).
  Handlers call `.expose().to_owned()` at serialization.
- `RefreshExchangeRequest.refresh_token: String` → `RawRefreshToken`.
  The token endpoint wraps the form field with
  `RawRefreshToken::from_untrusted(...)` before constructing the request.
- `authorize::exchange_code` and `complete_authorization` use `CodeHash::of`
  instead of the free `tokens::sha256_hex`.
- `oauth_token::try_introspect_refresh` and `try_revoke_refresh` wrap their
  `&str` parameters internally.
- `sui-id-shared` gains `sha2`, `getrandom`, `base64ct`, `zeroize` as
  direct dependencies to support the new types.

### Test coverage

- 8 new unit tests in `sui-id-shared::secrets`: redaction of Debug/Display,
  expose() value, hash determinism, CodeHash format, FamilyId root equality,
  RefreshTokenId alphabet.
- Baseline suite (186 tests across 5 crates) passes unchanged. The new
  secrets tests raise the count to 194.

## [0.63.2] — 2026-06-05

**Security-assurance audit, RFC set 078–086, and three baseline fixes.**

### Added — security-critical assurance audit and RFC arc

Deliverables of the architect instruction
`security-critical-assurance-strategy-v0.63.1.md`:

- `docs/security-assurance-audit-v0.63.1.md` — codebase audit of
  the security-critical lifecycles (auth codes, refresh-token
  rotation, sessions, role boundary, secrets, audit events).
  Nine risk-ranked gaps (G1–G9), Category A–D classification per
  strategy §6, and the adoption roadmap. Headline findings:
  refresh rotation spans three non-atomic storage calls and can
  fork a token family under concurrency (G1); the stored
  refresh-token model carries a `Debug`-reachable plaintext
  field (G2).
- Nine proposed RFCs in `rfcs/proposed/` (078–086): type
  modeling baseline, auth-code lifecycle assurance, refresh
  rotation atomicity, actor scope boundary (single-tenant
  translation of the strategy's tenant theme), pure
  authorization decision core, state-machine proptest harness,
  fuzzing strategy, audit-event completeness, and a time-boxed
  formal-methods pilot. Index updated in `rfcs/README.md`.

No production behaviour changes from the RFC set; implementation
follows per the roadmap (suggested v0.64.0 onward).

### Fixed — `cargo test --doc` failure in `sui-id-core`

The `security.rs` module doctest referenced `SecurityLevel`
without importing it (E0433), leaving the doc-test suite red.
The example now imports the type and asserts the thresholds.

### Fixed — `mod.rs` policy regression (spec §8.3)

`crates/sui-id-core/src/admin/mod.rs` and
`crates/sui-id/src/backup/mod.rs` crept in during the RFC 075
split. Both moved to umbrella style (`admin.rs`, `backup.rs`);
`find crates -name mod.rs` is empty again.

### Fixed — unused-import warning (0-warnings gate)

`handlers/admin/clients.rs` imported `Flash` / `FlashKind`
without using them; removed.

### Known issue — rustfmt style-edition drift (not fixed here)

`cargo fmt --check` under current stable rustfmt (Rust 2024 style
edition) reports mechanical diffs across most of the workspace.
Deliberately deferred to a dedicated fmt-sweep release (or a
pinned `rust-toolchain.toml`) so this release's diff stays
reviewable. See the audit report §1.

## [0.63.1] — 2026-06-04

**Three fixes across admin pages.**

### Bug fix — non-admin stuck after logging in to admin panel

`login_post` now checks the user's role before handing out the session
cookie when the redirect target is not an OIDC or `/me/` path.
Non-admin users see a localized error: "This account does not have
access to the admin panel." The OIDC flow is unaffected.

- New i18n key `login_no_admin_access` in all three locales (635 keys).
- `crates/sui-id/src/handlers/admin/auth.rs`

### Bug fix — page header buttons on users/clients didn't work

The header buttons used `<a href="#add-user">` / `<a href="#register-client">`,
which scrolled to the `<details id="…">` element — but `<details>` is
closed by default, so the form stayed hidden. The `<details>` wrapper
is removed; the create-form sections are always visible below the table.
Clicking the header button now scrolls directly to the open form.

### Improvement — icon-only header buttons; list-primary layout

**Users, Apps, Signing keys:**

- Page header action buttons are now icon-only circles (`+` for create,
  `↻` for key rotation) with `aria-label` for accessibility. A new
  `.button--icon` CSS modifier (32 px circle, `border-radius: 50%`) is
  added to `chrome.rs`.
- The list table renders first on all three pages.
- The create / rotate section renders below the table with its anchor
  id (`#add-user`, `#register-client`, `#rotate-key`).

Files changed:
- `crates/sui-id-web/src/pages/users.rs`
- `crates/sui-id-web/src/pages/clients.rs`
- `crates/sui-id-web/src/pages/signing_keys.rs`
- `crates/sui-id-web/src/components/chrome.rs`

### CI

- `cargo check -p sui-id-web -p sui-id`: clean.
- CI invariants unchanged.

---

## [0.63.0] — 2026-06-04

**RFC 077 — Headless setup (`sui-id setup` subcommand).**

### Problem

sui-id could only be initialized through the browser-based setup wizard,
blocking automation (Ansible, cloud-init, Docker entrypoints, CI smoke
tests). The wizard remains unchanged for operators who prefer it.

### New subcommand: `sui-id setup`

```
sui-id setup --config <path> --admin-username <name>
             [--admin-email <email>] [--admin-display-name <name>]
```

Runs against the configured database directly — same `Config::load` →
`keyring::resolve` → `Database::open` path as `backup`/`admin`. No
setup token required: filesystem access to the database and master key
already implies host control (same trust model as `admin unlock-user`).
Fails immediately with exit 1 if the instance is already initialized.

### Password security

There is **no `--admin-password` flag** — command-line arguments leak
into shell history and `ps` output.

Resolution order:

1. **`SUI_ID_ADMIN_PASSWORD` env var**, if set and non-empty. Validated
   at `SecurityLevel::Standard` (12+ chars). For provisioning tools
   that manage secrets already (Ansible vault, Docker secrets).
2. **Generated password** — 24 alphanumeric characters from the OS RNG
   (≈143 bits of entropy, rejection-sampled to avoid modulo bias,
   stored in `Zeroizing<String>`). Printed **once** to stdout in a
   clearly delimited block with a change advisory. Credential row is
   stored with `must_change = true` to record the intent durably.

stdout carries only the credential block (machine-capturable).
All diagnostics (`sui-id initialized (admin id=...)`) go to stderr.

```
============ INITIAL ADMIN CREDENTIALS ============
  username: <name>
  password: <generated>          ← absent when env-var is used
===================================================

This password is shown only once. Change it after first
login at /me/security/password.
```

### Core changes (`sui-id-core/src/setup.rs`)

- `create_initial_admin_headless(db, clock, username, password,
  display_name, email, must_change)` — new public function, skips
  the setup-token check, passes `must_change` through to the credential
  row, records `note: "headless"` in the audit entry.
- `generate_admin_password() -> Zeroizing<String>` — new function.
- `create_initial_admin_inner(...)` — new private helper shared by
  both paths; the web-wizard function delegates to it unchanged.

### Latent bug fixed: nested Tokio runtime panic

`sui-id admin unlock-user` and `admin rotate-key` called
`Runtime::new().block_on()` inside `#[tokio::main]`, which always
panics with "Cannot start a runtime from within a runtime". This was
discovered during the RFC 077 live smoke test. All three async
subcommand wrappers (`run_admin_subcommand`, `run_setup_subcommand`)
are now `async fn`; `main.rs` dispatches them with `.await`.

### Tests

Seven new tests in `sui-id-core/src/setup.rs`:

| Test | Guards |
|---|---|
| `generated_password_is_24_alphanumeric_chars` | Length and alphabet |
| `generated_passwords_differ_across_calls` | RNG freshness |
| `generated_password_satisfies_standard_policy` | Policy integration |
| `headless_setup_creates_admin_and_marks_initialized` | Happy path; `must_change` persisted |
| `headless_setup_fails_when_already_initialized` | Idempotency guard |
| `headless_setup_enforces_standard_password_policy` | "changeme" rejected |
| `web_wizard_path_still_requires_matching_token` | Existing path untouched; `must_change = false` |

**125/125 library tests pass** (7 more than v0.62.4).
CI invariants unchanged.

### Docs

`docs/src/reference/configuration.md` subcommand table updated.
`rfcs/done/077-headless-setup.md` added.

---

## [0.62.4] — 2026-06-04

**Security level — principled password policy thresholds.**

### Problem

The previous approach introduced two ad-hoc constants (`PASSWORD_MIN_LEN = 12`,
`PASSWORD_MIN_LEN_DEV = 8`) scattered in `password.rs`. This is fragile —
adding another security threshold (e.g. session minimum lifetime, token
length) would produce more scattered constants with no coherent model.

### Design: `SecurityLevel`

A new `sui_id_core::security::SecurityLevel` enum replaces the constants.
Each variant defines a coherent set of policy defaults, modelled after
browser security/privacy tiers (strict / standard / relaxed).

```
Standard     — full security; production deployments
Development  — relaxed constraints; --dev flag only; never production
```

`SecurityLevel` has one method per threshold:

| Method | Standard | Development |
|---|---|---|
| `password_min_len()` | 12 | 8 |

Future thresholds (session lifetime, rate-limit ceiling, etc.) are added
as methods here — no scattered constants, no new files.

### Changes

- **`crates/sui-id-core/src/security.rs`** — new file; `SecurityLevel`
  enum with `password_min_len()` method and full doc commentary.
- **`crates/sui-id-core/src/lib.rs`** — `pub mod security` registered.
- **`crates/sui-id-core/src/password.rs`** — constants removed; doc updated.
- **`crates/sui-id-core/src/setup.rs`** — uses
  `SecurityLevel::Standard.password_min_len()` (setup is always Standard).
- **`crates/sui-id/src/state.rs`** — `AppState::security_level()` derives
  the active level from `is_dev_mode`; avoids branching at call sites.
- **`crates/sui-id/src/handlers.rs`** — `password_min_len(&app)` helper
  delegates to `app.security_level().password_min_len()`.
- **`crates/sui-id/src/handlers/settings.rs`** — authentication settings
  display uses `app.security_level().password_min_len()` (shows 8 in
  `--dev` mode, 12 in production).
- **All 4 core function call sites** threaded as before (no change in
  mechanics, only in the source of the value).

### Tests

Four new tests in `password/tests.rs`:
- `policy_accepts_standard_minimum_length` — exactly 12 chars passes at Standard
- `policy_accepts_dev_password_at_development_level` — "changeme" (8) passes at Development
- `policy_rejects_too_short_even_at_development_level` — "short" (5) rejected at Development
- `standard_rejects_development_password` — "changeme" (8) rejected at Standard

**118/118 library tests pass** (4 more than v0.62.3).
CI invariants unchanged.

---

## [0.62.3] — 2026-06-04

**RFC 6749 §4.1.2.1 compliance — authorize endpoint error handling.**

### Problem

The `/oauth2/authorize` handler had two related defects:

**1. Client validation happened after the login redirect.**
When a request arrived with an invalid or unknown `client_id`, the handler
redirected the user to the login page before checking whether the client
existed. The user authenticated, was bounced back to `/oauth2/authorize`,
and only then saw an HTML error page at sui-id. There was no way back to
the calling application.

**2. All protocol errors produced HTML pages.**
Once the user was authenticated, any protocol error (invalid PKCE, bad
`scope`, wrong `response_type`) rendered a full HTML error page at sui-id
even though the redirect_uri had already been validated and it was safe —
and required by RFC 6749 §4.1.2.1 — to redirect back to the RP with an
`error=…` parameter.

### Fix — three-phase authorize flow

**Phase 1 (before login):** Validate `client_id` and `redirect_uri`
against the database. On failure: render HTML error immediately (never
redirect — the redirect_uri is not yet trusted). This is correct per
RFC 6749 §4.1.2.1 and prevents users from wasting time logging in when
the client is misconfigured.

A new `validate_client_and_redirect_uri` function in
`sui-id-core/src/authorize.rs` performs this check.

**Phase 2 (login):** If no session exists, redirect to login as before.
By the time the user returns, Phase 1 is already confirmed.

**Phase 3 (request validation):** Validate `response_type`, PKCE,
`scope`. On failure: call `protocol_error_redirect` which builds a
`Location: {redirect_uri}?error={code}&error_description={...}&state={...}`
redirect response, sending the user back to their application with an
actionable error code rather than stranding them at sui-id.

The new `protocol_error_redirect` helper is also applied to the
`consent_post` handler's `begin_authorization` call.

### Error codes returned to the RP (Phase 3)

| Condition | `error` parameter |
|---|---|
| `response_type` ≠ `code` | `unsupported_response_type` |
| `code_challenge` missing or empty | `invalid_request` |
| `code_challenge_method` ≠ `S256` | `invalid_request` |
| Scope not permitted for client | `invalid_scope` |

### What still shows an HTML page (Phase 1, RFC-required)

| Condition | Reason |
|---|---|
| `client_id` unknown | Cannot trust any redirect_uri |
| Client is disabled / deleted | Same |
| `redirect_uri` not registered for client | Security: open-redirector risk |

### Tests and CI

- `cargo check -p sui-id-core -p sui-id`: clean.
- CI invariants unchanged.

---

## [0.62.2] — 2026-05-24

**Three UX fixes from mockup-integration soak.**

### Dashboard — Sign-in activity tab scroll to top

Clicking a range tab (7d / 30d / 90d) in the Sign-in activity panel
caused a full-page navigation that snapped the browser scroll position to
the top. The user had to scroll back down to see the updated sparkline.

Fix: tab hrefs now include a `#sparkline` fragment
(`/admin?range=7d#sparkline`) and the enclosing `<section>` has
`id="sparkline"`. The browser jumps to the panel automatically after load.

### Settings — Chinese option removed from server default language dropdown

`Locale::ALL` included `Locale::Zh`, causing "中文" to appear as a
selectable server default. The zh translation file is complete and
compiled in; the variant is retained for future use but removed from
`ALL` until zh is officially a supported server-default locale.

### Settings — Server default language: locale resolution + save flash

All seven settings page handlers hardcoded `Locale::Ja` for the UI
language. They now call `resolve_admin_locale` (same as every other admin
handler) so the pages respect the signed-in admin's personal language.

`basic_lang_post` previously redirected silently after save. It now
re-renders with a success flash so the admin sees the new selection
confirmed in the dropdown.

### Tests and CI

- `cargo check -p sui-id-i18n -p sui-id-web -p sui-id`: clean.
- CI invariants unchanged: `text-leaks`=0, `inline-style-bound`=0,
  `css-tokens`=148, `semantic-parity`=36.

---

## [0.62.1] — 2026-05-23

**Bug fix: OIDC redirect after login broken.**

`login_get` was not extracting the `?next=` query parameter and not
threading it into the login form. As a result, after a user signed in
via the OIDC authorization-code flow, the redirect back to the relying
party (the `redirect_uri`) never happened — the user stayed on the
sui-id dashboard instead.

### Root cause

The `GET /admin/login?next=<encoded_authorize_url>` handler signature
had no `Query` extractor and always passed `None` to `render_login`.
This meant the hidden `<input name="next">` in the login form was always
empty, so `login_post` fell through to the default `/admin` redirect.

### Fix

Added a `LoginGetQuery { next: String }` extractor to `login_get`.
The `next` value (when present and starting with `/`) is:
- Rendered into the hidden form field so `login_post` can redirect to it.
- Used as the forward destination when an already-logged-in user hits
  `/admin/login?next=...` (previously always redirected to `/admin`
  regardless of `next`).

The MFA path was **not** affected — it correctly preserved `next` via
a short-lived cookie (`SUI_ID_PENDING_MFA_NEXT`).

### Files changed

- `crates/sui-id/src/handlers/admin/auth.rs` — `login_get` signature
  and body; new `LoginGetQuery` struct; `Query` added to imports.

---

## [0.62.1] — 2026-05-23

**Bugfix: consent route missing; redirect_uri error improvement.**

### Bug 1 — `POST /oauth2/consent` route not registered (critical)

`consent_post` was implemented for RFC 038 (v0.50.x) but its route was
never wired into the router. Effect: when a client's `consent_policy`
is `first_time` or `always`, the consent page is shown after login but
clicking **Allow** POSTs to a 404/405 endpoint. The user cannot complete
the OIDC flow; they stay on the sui-id consent page indefinitely.

Fix: `.route("/oauth2/consent", post(oidc::consent_post))` added to
`router.rs`.

Clients with `consent_policy = "none"` (the default) are **not
affected** — the consent page is never shown for those clients.

### Bug 2 — Cryptic redirect_uri mismatch error

When the `redirect_uri` in an authorization request does not exactly
match any registered URI, `begin_authorization` returned:

> "redirect_uri does not match a registered URI"

This gave operators no information about what was submitted vs. what was
registered. Fix: the error message now includes the submitted URI and the
full list of registered URIs:

> "redirect_uri does not match any registered URI for this client.
>  Submitted: "http://localhost:3000/callback".
>  Registered URIs: ["http://localhost:3000/api/auth/callback/myapp"].
>  The comparison is exact — check for trailing slashes, http vs https,
>  and port numbers."

Additionally, the authorize handler now emits a `WARN`-level tracing
event with the client_id and error description on every rejected
authorization request, so the error is visible in the server log even
without inspecting the audit table.

### Tests and CI

- `cargo check --workspace` clean.
- All CI invariants unchanged.

---

## [0.62.0] — 2026-05-23

**RFC 075 — File-size refactor** and **RFC 076 — Configuration reference.**
Two verification-soak items from the v0.61.0 audit. No behaviour changes
and no schema changes.

---

### RFC 075 — File-size refactor

Three source files exceeded the project's 500-effective-LoC split
threshold. Split mechanically; all public names re-exported so zero
callsites outside the affected crates changed.

| Before | After |
|---|---|
| `sui-id-core/src/admin.rs` — 785 lines | `admin/mod.rs` (71), `users.rs` (239), `clients.rs` (287), `signing_keys.rs` (73) |
| `sui-id/src/backup.rs` — 1063 lines | `backup/mod.rs` (14), `types.rs` (55), `ops.rs` (466), `tar.rs` (112), `tests.rs` (437) |
| `sui-id/src/main.rs` — 825 lines | `main.rs` (318), `cli.rs` (519) |

### RFC 076 — Configuration reference

Replaced the 6-line stub in `docs/src/reference/configuration.md` with
a 193-line complete reference covering all 10 config fields across 5 TOML
sections, 3 environment variables, 4 runtime flags, a subcommand table,
startup validation rules, a minimal configuration block, and a
production-ready annotated configuration block.

### Tests and CI

- `cargo check --workspace` clean; 0 errors, 0 warnings.
- **175/175 library tests pass** (sui-id-i18n 12, sui-id-shared 13,
  sui-id-web 0, sui-id-store 36, sui-id-core 114).
- CI invariants unchanged: `text-leaks`=0, `inline-style-bound`=0,
  `css-tokens`=148, `semantic-parity`=36.

---

## [0.61.0] — 2026-05-23

**RFC 074 — Navigation restructuring and UX polish.**
All pre-1.0 UI/UX cleanup items from the post-MI-arc audit are now shipped.

---

### Item 1 — Admin user-menu dropdown

The admin top-nav's flat "Security" link is replaced by a `<details>/<summary>`
user-menu dropdown at the right end of the nav bar. It shows the signed-in
admin's username and contains "My account" → `/me/security/overview` and
"Sign out." No JavaScript required. `Shell` gained an optional
`admin_username: Option<String>` prop; `Nav` was rewritten to use it.

New CSS: `.user-menu`, `.user-menu__toggle`, `.user-menu__panel`,
`.user-menu__item`, `.user-menu__form` in `components/chrome.rs`.

### Item 2 — "Apps" in admin nav

The nav label for `/admin/clients` is now "Apps" (i18n: `nav_apps`).
Route and handler code unchanged.

### Item 3 — Settings tab labels

Basic → **General**, Other → **Advanced** (label only; routes unchanged).
Full 6→4 group consolidation is deferred.

### Item 4 — Last sign-in line

**Migration 0030** — `ALTER TABLE users ADD COLUMN last_login_at TIMESTAMP`.

`set_last_login(db, user_id, now)` — best-effort helper called after every
successful `sessions::insert`. `/me/security/overview` now shows a
`<p class="muted text-caption">` below the `<h1>`:
- "You last signed in on {date}." (subsequent logins)
- "Welcome — this is your first sign-in." (null / pre-migration)

### New i18n keys (6 × 3 locales)

`nav_apps`, `nav_my_account`, `settings_tab_general`, `settings_tab_advanced`,
`me_overview_last_login`, `me_overview_first_login`.

### Tests and CI

- `cargo check --workspace` clean; 0 errors, 0 warnings.
- **175/175 library tests pass** (sui-id-i18n 12, sui-id-shared 13,
  sui-id-web 0, sui-id-store 36, sui-id-core 114).
- 7 test `UserRow` constructors in `sui-id-core` gained `last_login_at: None`.
- CI invariants unchanged: `text-leaks`=0, `inline-style-bound`=0,
  `css-tokens`=148, `semantic-parity`=36.

---

## [0.60.1] — 2026-05-23

**Documentation and housekeeping.**

- All previously "Unreleased" CHANGELOG entries dated from archive timestamps.
- `README.md`: Scope section updated to describe the three-role model
  (admin / auditor / user) introduced in v0.59.0. Features list extended
  with the three items delivered by the UX-rethink arc (RFC 071–073).
- `docs/src/getting-started/overview.md`: new "Human roles" section.
- `docs/src/guides/operators.md`: Scope callout updated; new Roles
  reference table added.
- `rfcs/proposed/074-nav-ux-polish.md`: new RFC capturing four deferred
  pre-1.0 polish items (user-menu dropdown, "Apps" nav rename, settings
  tabs 6→4, last-login anti-phishing line).
- `ROADMAP.md` Status block updated for v0.60.0 arc completion + RFC 074.
- No code changes; CI invariants unchanged.

---

## [0.60.0] — 2026-05-23

**RFC 072 — End-user app-access surface.** Completes the three-RFC
UX-rethink arc. Users can now see which OAuth clients hold a consent
grant and revoke any grant with one click.

---

### Schema: migration 0029

`ALTER TABLE user_consent ADD COLUMN last_used_at TIMESTAMP`. NULL until
the first token exchange after this migration.

### New repo functions (`user_consent.rs`)

- **`list_for_user(db, user_id)`** — SELECT joining `user_consent` with
  `clients` for non-deleted clients; returns `Vec<ConsentGrantView>`.
- **`revoke_with_tokens(db, user_id, client_id)`** — atomic transaction:
  deletes all `refresh_tokens` for the pair, then the `user_consent` row.
- **`touch_last_used(db, user_id, client_id, now)`** — UPDATE
  `last_used_at`; called best-effort at the token endpoint.

### `TokenSet.user_id: Option<UserId>`

Added to `sui-id-core::tokens::TokenSet`; populated by `issue_token_set`.
Used by the token endpoint to call `touch_last_used` without an extra
DB lookup.

### `/me/apps` — new self-service surface

- **`MeTab::Apps`** — new variant in the tab enum; appears between Sessions
  and Language in the tab strip on every `/me/security/*` page.
- **`render_me_apps`** — one card per grant: client name, Granted / Last
  used dates, scopes (reusing `.consent-scope-item` CSS from RFC-MI-070),
  Revoke button. Empty state uses `.callout--info`. No new CSS tokens.
- **`GET /me/apps`** and **`POST /me/apps/{client_id}/revoke`** routes.

### i18n

9 new keys (× 3 locales — en/ja/zh): `me_tab_apps`, `me_apps_title`,
`me_apps_intro`, `me_apps_granted_on`, `me_apps_last_used`,
`me_apps_never_used`, `me_apps_revoke_button`, `me_apps_revoked`,
`me_apps_empty`.

### Tests and CI

- `cargo check --workspace` clean; 0 errors, 0 warnings.
- **175/175 library tests pass** (sui-id-i18n 12, sui-id-shared 13,
  sui-id-web 0, sui-id-store 36, sui-id-core 114).
- CI invariants unchanged: `text-leaks`=0, `inline-style-bound`=0,
  `css-tokens`=148, `semantic-parity`=36.

---

## [0.59.0] — 2026-05-22

**RFC 071 — Auditor role.** Adds a third human role (`auditor`) with
read-only access to all admin surfaces. No deployment with more than one
operator could previously grant safe read-only access without sharing full
admin credentials.

---

### Schema: migrations 0027 and 0028

**0027** — `ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user'
CHECK (role IN ('admin', 'auditor', 'user'))`. Backfills from `is_admin`.
Adds `idx_users_role`. The old `is_admin` boolean column is kept in sync
as a compatibility shim and will be dropped in a future migration (0029).

**0028** — `ALTER TABLE audit_log ADD COLUMN actor_role TEXT CHECK (...)`.
NULL for pre-migration rows.

### `Role` enum

New `sui_id_store::models::Role { Admin, Auditor, User }` with
`is_admin()`, `can_read_admin()`, `as_str()`, `from_str()`.
`UserRow` gains `role: Role`; the row mapper reads the new column with
an `is_admin` fallback for rows that pre-date migration 0027.

### `CurrentAdminOrAuditor` extractor

New Axum extractor in `handlers.rs` returning `(UserId, Role)`.
Passes for `role ∈ {admin, auditor}`; returns 403 for plain users.
All admin **GET** routes now use this extractor. All **POST / DELETE**
routes remain on `CurrentAdmin` (admin-only).

### `can_write: bool` in render functions

Five render functions gained a `can_write: bool` first parameter.
When `false` (auditor), the following controls are hidden:
- Users list: "Add user" form, row action buttons
- User detail: danger zone section (Reset MFA, Disable, Delete)
- Clients list: Edit/Disable/Delete buttons replaced by a "View" link
- Client edit: Save button and danger zone
- Signing keys: rotate form and delete buttons

### Role-change UI on user detail page

New "Access role" section on the user detail page (visible only to admins)
with a `<select>` drop-down and a submit button. Posts to the new route
`POST /admin/users/{id}/role`.

**Last-admin safeguard**: if the target is the only admin, demotion is
refused with a localised error message. The check uses a new
`users::count_admins()` repo helper.

### i18n

7 new keys (×3 locales — en/ja/zh): `role_admin`, `role_auditor`,
`role_user`, `user_detail_role_section`, `user_detail_role_change`,
`user_detail_role_saved`, `user_detail_role_last_admin`.

### Tests and CI

- `cargo check --workspace` clean; 0 errors, 0 warnings.
- **175/175 library tests pass** (sui-id-i18n 12, sui-id-shared 13,
  sui-id-web 0, sui-id-store 36, sui-id-core 114).
- 7 test `UserRow` constructors in `sui-id-core` gained the `role` field.
- CI invariants unchanged: `text-leaks`=0, `inline-style-bound`=0,
  `css-tokens`=148, `semantic-parity`=36.

---

## [0.58.0] — 2026-05-22

**RFC 073 — Dashboard action items.** The admin dashboard now surfaces
operational concerns rather than just vanity counts.

---

### Getting Started checklist (new, fresh instances only)

A `.callout--info` section at the top of the dashboard lists three items
for new deployments — Configure SMTP, Add first app, Enable admin MFA.
Each shows a `☐` or `✓` text indicator (ABDD-compliant; no colour-only
state). Disappears automatically once all three items are done.

### Action items section (expanded from RFC 031)

The previous three-item RFC 031 warning section is replaced by a unified
`.callout--warning` "Action items" list that includes four new signals:

| Condition | Trigger |
|---|---|
| Admins without MFA | ≥ 1 admin account has no TOTP or WebAuthn |
| Old signing key | Oldest active key ≥ 330 days (rotation due before 12-month sunset) |
| Outbox stuck | ≥ 1 queued email older than 1 hour |
| Pending resets | ≥ 5 unconsumed, unexpired password-reset tokens |

All conditions are best-effort aggregates on existing, indexed tables;
a single failing query falls back to zero (dashboard never breaks).

### Four new repo helpers

- `users::count_admins_without_mfa()`
- `users::has_mfa(user_id)`
- `email_outbox::count_stuck_pending(threshold, now)`
- `password_reset_tokens::count_outstanding(now)`

### New i18n keys

8 keys added in en/ja/zh. 4 are parameterised (`fn(usize)/fn(i64) -> String`); 4 are static strings for the Getting Started checklist.

### CSS

`.action-items-list` and `.checklist` added to `components/banners.rs`.

### Tests and CI

- **228/228 library tests pass.**
- `text-leaks` = 0, `inline-style-bound` = 0, `css-tokens` = 148, `semantic-parity` = 36.

### RFC planning documents

RFC 071 (Auditor role) and RFC 072 (End-user app-access surface) added
to `rfcs/proposed/` — the next two items in the UX rethink arc.

---

## [0.57.1] — 2026-05-21

**Dependency refresh: RFC 069 (rand 0.10) + RFC 070 (ureq → reqwest).**
No user-visible behaviour changes; test suite unchanged (228/228 pass).

---

### rand 0.10 migration (RFC 069)

The `rand 0.8` / `rand_core 0.6` ecosystem is replaced by `rand 0.10`
/ `rand_core 0.10` / `getrandom 0.4`.

**Option B** used for the ed25519-dalek blocker
(`SigningKey::generate(&mut OsRng)` cannot use rand_core 0.10 while
ed25519-dalek 2.x pins rand_core 0.6): secret key bytes now generated
via `getrandom::fill` into a `Zeroizing<[u8; 32]>`, then passed to
`SigningKey::from_bytes`. Cryptographically equivalent; memory-safe.

All other `OsRng.fill_bytes(...)` call sites (10 total across
`forgot_password`, `mfa`, `tokens`, `backup`, `csrf`, `startup`,
`main`, `crypto`) replaced with `getrandom::fill(...).expect(...)`.

`SaltString::generate(&mut OsRng)` in `password.rs` replaced with
`SaltString::encode_b64(&raw_16_bytes)` from a `getrandom::fill` call.

JWT unit tests: `SigningKey::generate(&mut OsRng)` →
`SigningKey::from_bytes(&[1u8; 32])` (deterministic seed; correct for
tests).

`rand_core` removed as a direct dependency from three crate
`Cargo.toml` files; `getrandom = "0.4"` added in its place.

### ureq → reqwest migration (RFC 070)

The `HibpClient` trait becomes `async fn check` via `async-trait`.
`HttpHibpClient` is rebuilt on `reqwest::Client` (stored internally,
constructed at server start). All test stubs updated.

**Bug fixed as a side-effect:** the previous `enforce_hibp` called
`client.check(password)` synchronously inside an `async fn`, blocking
the tokio runtime thread during the ureq HTTP request. This is now
correct: `client.check(password).await`.

`ureq` is fully removed from the workspace. `reqwest 0.12`
(`rustls-tls` feature) and `async-trait 0.1` added.

### Tests and CI

- **228/228 library tests pass** (175 from 5 crates + 53 from `sui-id`).
- All four CI invariants unchanged: `text-leaks` = 0, `css-tokens` = 148,
  `semantic-parity` = 36, `inline-style-bound` = 0.

---

## [0.57.0] — 2026-05-19

**Phase 8 complete: `RFC-MI-080` (UI Regression and Accessibility
Hardening). The Mockup Integration arc is fully closed.**

All 16 MI RFCs — spanning Phases 0 through 8 across versions v0.49.0
through v0.57.0 — are now in `rfcs/done/`.

---

### Blocker fix: skip link (WCAG 2.4.1 Level A)

`<a class="skip-link" href="#main-content">` added as the first
focusable element in both `Shell` and `AuthShell`. Both layouts gain
`<main id="main-content">` as the skip target. `<header
role="banner">` added to both shells. CSS `.skip-link` class in
`chrome.rs::CHROME_RESPONSIVE_CSS` — off-screen normally, slides in
on `:focus`.

New i18n key `a11y_skip_to_main` (en: "Skip to main content",
ja: "メインコンテンツへスキップ", zh: "跳转到主要内容").

### Narrow breakpoints added (WCAG 1.4.10)

Two new breakpoints in `chrome.rs::CHROME_RESPONSIVE_CSS`:

- **`≤ 480px`** — auth-card full-bleed, smaller `.stat__value`,
  reduced route-tab padding, `.danger-zone` tighter padding.
- **`≤ 360px`** — `.form-actions` stacks vertically (buttons
  full-width, centred); `.grid-cards` collapses to single column.

All content reflows to a single column at 360px without horizontal
scrolling (WCAG 1.4.10). Nav and table horizontal scroll at 360px
is acceptable under the 2D-content exception.

### Verification matrices committed

Six documents under `docs/src/mockup-integration/`:

- `accessibility-matrix.md` — ABDD attributes per screen
- `no-js-matrix.md` — no-JS coverage (all core flows pass)
- `keyboard-navigation-matrix.md` — keyboard reachability (all pass)
- `responsive-matrix.md` — 768px / 480px / 360px
- `i18n-copy-review.md` — localisation audit (0 leaks)
- `security-sensitive-copy-review.md` — anti-enumeration, OIDC scope accuracy, confirmations

### MI arc final state

- **16 / 16 MI RFCs in `rfcs/done/`** ✅
- **`inline-style-bound` = 0** ✅ (met in v0.56.0; maintained here)
- **228/228 library tests pass** ✅
- All four CI invariants: `text-leaks` = 0, `css-tokens` = 148,
  `semantic-parity` = 36, `inline-style-bound` = 0

### Version bumps

`0.56.0` → `0.57.0` across workspace, all six crates, `Cargo.lock`.

---

## [0.56.0] — 2026-05-19

**Phase 7 complete: `RFC-MI-070` (OIDC Consent UX Integration).**

**`inline-style-bound` reaches 0.** The last four inline styles in
the codebase — all in `pages/oidc.rs` — are eliminated. The MI
arc's inline-style discipline target is fully met.

---

### Four CSS classes for the consent screen

Added to `components/setup.rs` (which owns the auth-card centred
layout — the natural home for consent-screen styles):

- **`.consent-card`** — `max-width: 32rem` modifier on top of
  `.auth-card`. Consent needs 512px vs login's 448px
  (`--content-narrow-width`) to fit the scope list comfortably.
- **`.consent-intro`** — `margin: var(--space-3) 0` for the intro
  paragraph.
- **`.consent-scope-list`** — no-bullet flex column list with
  `gap: var(--space-1)` and `margin-bottom: var(--space-4)`.
- **`.consent-scope-item`** — flex row (`align-items: baseline; gap:
  var(--space-2)`) for badge + description pairing.

### `render_consent` rewritten

The four inline `style=` attributes in `pages/oidc.rs` are replaced
with the four classes above. **Each scope now renders as a vertical
stack:** bold label (`.consent-scope-item__title`), muted description
sentence (`.consent-scope-item__desc`), and a `<code>` element with
the raw scope slug for developer context.

**Four scope description keys added** (×3 locales — en/ja/zh):
`consent_scope_openid_desc`, `consent_scope_profile_desc`,
`consent_scope_email_desc`, `consent_scope_offline_access_desc`.
Unmapped scopes fall back to `"—"` as title with no description.

### Protocol guarantees preserved

Authorization Code + PKCE flow, redirect URI validation, and
Approve / Deny POST forms with CSRF are all unchanged. Both actions
are `<button>` elements with equal keyboard access.

### `inline-style-bound` = 0

| Release | Bound |
|---|---|
| v0.48.4 (baseline) | 17 |
| v0.50.1 (Phase 1) | 17 → 16 |
| v0.51.1 (Phase 2) | 16 → 16 |
| v0.52.0 (Phase 3) | 16 → 10 |
| v0.53.0 (Phase 4a) | 10 → 7 |
| v0.53.1 (Phase 4b) | 7 → 5 |
| v0.54.0 (Phase 5) | 5 → 4 |
| v0.56.0 (Phase 7) | 4 → **0** |

### Tests, CI, and compatibility

- `cargo check --workspace` clean.
- **228/228 library tests pass**.
- `text-leaks` = 0, `css-tokens` = 148, `semantic-parity` = 36,
  **`inline-style-bound` = 0**.
- 4 new i18n keys (×3 locales). No handler or route changes.

### Version bumps

`0.55.0` → `0.56.0`. 15 of 16 MI RFCs now in `rfcs/done/`. 1 remains.

---

## [0.55.0] — 2026-05-19

**Phase 6 complete: `RFC-MI-060` (Self-Service Security Tab Integration).**
The last deferred item from RFC-MI-022 is resolved.

---

### Password tab now has the route-tab strip

`render_password_change` in `pages/auth.rs` was the only one of the
six `/me/security/*` routes that did not show the tab strip. That
inconsistency is now resolved.

Changes:
- `show_nav=false` → `show_nav=true` — the admin nav is now visible
  on the password-change page.
- `current=None` → `current=Some("me")` — "Security" nav link is
  highlighted.
- `me_security_tabs(MeTab::Password, lang)` added above the page
  header.
- Import `use super::me_security::{me_security_tabs, MeTab}` added
  to `pages/auth.rs`.
- Cancel link updated from `/me/security` (deprecated) to
  `/me/security/overview`.
- Form submit/cancel buttons migrated from `.row` to `.form-actions`
  (RFC-MI-050 primitive from v0.54.0).

All six `/me/security/*` routes now consistently show the
`.route-tabs` strip with `aria-current="page"` on the active tab
and `show_nav=true current="me"` in the Shell.

### MFA enable/disable decision documented

**Option 2: self-service enable + admin reset.** Users can enrol
and remove their own TOTP from the MFA tab; admins can forcibly
reset via the user detail danger zone. Step-up is required before
TOTP changes (enforced since v0.45.0). No code changes needed —
the existing product already implements this model. Decision
committed to `rfcs/done/RFC-MI-060`.

### Tests, CI, and compatibility

- `cargo check --workspace` clean.
- **228/228 library tests pass**.
- `text-leaks` = 0, `css-tokens` = 148, `semantic-parity` = 36,
  `inline-style-bound` = 4 (unchanged — all 4 remaining are in
  `oidc.rs`, owned by RFC-MI-070).
- No i18n keys added (the `me_tab_password` key from v0.51.1 was
  already in place).
- No handler or route changes.

### Version bumps

`0.54.0` → `0.55.0` across workspace, all six crates, and
`Cargo.lock`. 14 of 16 MI RFCs now in `rfcs/done/`. 2 remain.

---

## [0.54.0] — 2026-05-19

**Phase 5 complete: `RFC-MI-050` (Form System + Validation) and
`RFC-MI-051` (Danger Zone + Confirmation).** `inline-style-bound`
drops from 5 to **4** — only `pages/oidc.rs` (×4, owned by
RFC-MI-070 Phase 7) remains.

---

### Form system: two missing primitives (`RFC-MI-050`)

**`.field--required`** added to `components/forms.rs`. Appends a
red `*` after the label via `::after` (CSS-generated; aria-hidden
by default; supplement with `required` HTML attribute for full
accessibility).

**`.review-summary`** added to `components/forms.rs`. A
`surface-subtle` panel for pre-submit value review.

The other form primitives (`.form-actions`, `.form-section`,
`.form-section__title`, `.form-grid`) were already present.

No `FieldError` / `FormAction` Rust helper structs introduced —
the RFC §7 says "Do not over-generalize."

### Danger zone: user detail page restructured (`RFC-MI-051`)

**`components/confirm.rs`** — `.danger-zone` + `.impact-summary`
CSS families were already present; shard docstring updated.

**`pages/users.rs`** restructured:
- Action buttons (Reset MFA, Disable/Enable, Delete) **removed
  from page header** — the `<div class="row" style="…">` wrapper
  is gone, eliminating the last non-oidc inline style.
- A **`<section class="danger-zone">`** appended after all read
  surfaces (auth info → sessions → activity → danger zone).
  Contains: `⚠ Danger Zone` heading, explanation paragraph, action
  buttons in `<div class="form-actions">`.

**New i18n key `user_detail_danger_zone_body`** in all three locales
(en / ja / zh).

All confirmation routes unchanged: every link leads to a
GET-then-POST confirm page with CSRF, step-up, and audit logging.

### Tests, CI, and compatibility

- `cargo check -p sui-id-web` clean.
- **228/228 library tests pass**.
- `text-leaks` = 0, `css-tokens` = 148, `semantic-parity` = 36,
  **`inline-style-bound` = 4** (was 5; −1 from users.rs).

### 14 of 16 MI RFCs now in `done/`

Only RFC-MI-060 (Phase 6), RFC-MI-070 (Phase 7), and RFC-MI-080
(Phase 8) remain.

### Version bumps

`0.53.1` → `0.54.0` across workspace, all six crates, `Cargo.lock`.

---

## [0.53.1] — 2026-05-19

**Phase 4 complete.** `RFC-MI-040` (Setup Wizard UX Integration)
ships — the second Phase-4 RFC (after MI-041 in v0.53.0).

---

### `StepState` enum and `SetupStep` struct (public API)

Added to `components/setup.rs`, re-exported from `components.rs`:

- **`StepState`** — `Complete | Current | Upcoming`, with
  `label_class() -> &'static str` mapping each variant to one of
  the three CSS classes below.
- **`SetupStep`** — `{ key: &'static str, label: String, state: StepState }`.
  Available for handler-layer render data.

### `.setup-steps` and step label CSS classes

Three new classes added to `components/setup.rs`:

- `.setup-steps` — flex row, centered, caption-size, wraps.
  Replaces the step indicator nav's inline
  `style="gap:…;justify-content:center;…"`.
- `.setup-step__label--current` — `color: --fg-default; font-weight: medium`.
- `.setup-step__label--done` — `color: --fg-muted`.
- `.setup-step__label--upcoming` — `color: --fg-subtle`.

### Two inline styles eliminated in `pages/setup.rs`

`setup_step_indicator()` rewritten to use the classes above:
the `style=style` computed string and the nav container
`style="…"` are both replaced with class attributes.
`inline-style-bound` drops **7 → 5**.

### Setup flow unchanged

Five steps (Welcome, Admin, Language, HIBP, Done), same badge
system, same `aria-current="step"` on the active entry. Setup token
URL parameter model unchanged. No route contracts changed. No render
function signatures changed. No i18n keys added.

### Tests, CI, and compatibility

- `cargo check -p sui-id-web` clean.
- **228/228 library tests pass**.
- `text-leaks` = 0, `css-tokens` = 148, `semantic-parity` = 36,
  **`inline-style-bound` = 5** (was 7; −2).

### Phase 4 complete — 12 of 16 MI RFCs now in `done/`

| RFC | Release |
|---|---|
| RFC-MI-041 (auth surfaces) | v0.53.0 |
| RFC-MI-040 (setup wizard) | **v0.53.1** (this release) |

### Version bumps

`0.53.0` → `0.53.1` across workspace, all six crates, and
`Cargo.lock`.

---

## [0.53.0] — 2026-05-18

**Phase 4 opens with `RFC-MI-041` (Authentication Surface
Integration).** Shipped **ahead of `RFC-MI-040`** at user request —
auth surfaces are tighter in scope and security-sensitive, so they
land first; the setup wizard work (`RFC-MI-040`) follows in
v0.53.1.

---

### Security guarantee

**Zero copy changed. Zero i18n keys changed.** A line-level diff
of `pages/auth.rs` and the entire `sui-id-i18n` crate against
v0.52.0 (excluding `class=` / `style=` attributes) is empty.
Anti-enumeration wording, MFA failure copy, step-up purpose copy,
and reset-token failure copy are byte-identical to v0.52.0. No
backend auth logic is touched.

### Three inline styles eliminated in `pages/auth.rs`

| Site | Before | After |
|---|---|---|
| Login "Forgot password?" link | `style="margin-top:…;text-align:center;font-size:…"` | `class="muted auth-meta-link"` |
| MFA setup TOTP QR code | `style="max-width:240px;margin-bottom:…"` | `class="qr-display"` |
| Password change card | `style="max-width:var(--content-narrow-width)"` | `class="card card--narrow"` |

### Three new CSS classes

- **`.auth-meta-link`** (→ `components/setup.rs`) — muted,
  caption-size, centered, top-margined. For "Forgot password?",
  "Back to sign-in", and similar meta links below auth forms.
- **`.qr-display`** (→ `components/setup.rs`) — bounded TOTP
  QR-code container (`max-width: 240px; margin-bottom: --space-3`).
- **`.card--narrow`** (→ `components/cards.rs`) — constrains a
  `.card` to `--content-narrow-width`. Used by the password-change
  form and any other isolated single-action card.

### ABDD improvement: flash banner role per kind

`FlashKind::aria_role()` added to `pages/common.rs`:
- `FlashKind::Error` → `role="alert"` (interrupts assistive tech
  immediately for login failure, MFA failure, step-up failure,
  reset-token failure)
- `FlashKind::Info` / `FlashKind::Warn` → `role="status"` (polite
  announcement for benign messages like "Settings saved")

The helper change is transparent to every caller. No `flash_banner`
call site needs updating.

### Tests, CI, and compatibility

- `cargo check -p sui-id-web` clean.
- **228/228 library tests pass** (12 + 13 + 0 + 36 + 114 + 53).
- `text-leaks` = 0, `css-tokens` = 148, `semantic-parity` = 36,
  **`inline-style-bound` = 7** (was 10; −3 this release).
- No-JS form submission still works (no script change; forms remain
  plain `method="post"` with hidden `_csrf` server-rendered per
  RFC-MI-021).

### Version bumps

`0.52.0` → `0.53.0` across workspace, all six crates, and `Cargo.lock`.

10 of 16 MI RFCs now in `rfcs/done/`. 6 remain.

---

## [0.52.0] — 2026-05-18

**Phase 3 complete: read-only admin screens.** `RFC-MI-030`
(Dashboard) and `RFC-MI-031` (Audit + Tables) ship together.
`inline-style-bound` drops from 16 to **10** — the largest
single-release improvement in the MI arc so far.

---

### Dashboard warning section: `.callout--warning` (`RFC-MI-030`)

The operator warning block (SMTP not configured, HIBP off, insecure
cookie) migrates from `<section class="card card--warn">` to
`<section class="callout callout--warning">` — using the neutral
callout primitive introduced in v0.50.1. The `<h2>` moves from an
inline `style="font-size:…;margin:…"` to `class="callout__title"`.
A new `.callout__title` rule is added to `components/cards.rs`.

### Sparkline layout: four new CSS classes (`RFC-MI-030`)

Four inline styles in the dashboard sparkline section are replaced
by CSS classes added to `components/utilities.rs`:

- `.sparkline-container` — SVG dimensions (`width:100%; height:80px; display:block`)
- `.sparkline-header` — flex row for title + period nav
- `.sparkline-title` — h3 reset (margin: 0, medium weight, dimmed opacity)
- `.sparkline-legend` — legend flex row with `gap: --space-5`

### Audit page: cell discipline and filter row (`RFC-MI-031`)

**New CSS in `components/tables.rs`:**
- `.cell-nowrap` — explicit no-wrap (documents intent)
- `.cell-id` — monospace, caption size, max-width 16rem, text-overflow ellipsis
- `.cell-actions` — right-align, never wraps

**Applied to `audit_row_view()`:**  timestamp → `muted cell-nowrap`;
actor → `cell-nowrap`; action → `cell-wrap`; target → `cell-id`;
outcome → `cell-nowrap`; copy button → `cell-actions`.

**Filter bar:** `<div class="row" style="…">` replaced with
`<div class="filter-bar">`. The `.filter-bar` class is added to
`components/utilities.rs`.

**Audit `<thead>` expanded** to 6 columns (added `<th
aria-hidden="true">` for the copy button); `colspan="5"` empty
state updated to `colspan="6"`.

### Summary: 6 inline styles eliminated

| Page | Before | After |
|---|---|---|
| `dashboard.rs` (warning h2) | `style="font-size:…;margin:…"` | `.callout__title` |
| `dashboard.rs` (SVG) | `style="width:100%;height:80px;display:block"` | `.sparkline-container` |
| `dashboard.rs` (sparkline header) | `style="justify-content:…"` | `.sparkline-header` |
| `dashboard.rs` (sparkline h3) | `style="margin:0;font-weight:…;opacity:0.85"` | `.sparkline-title` |
| `dashboard.rs` (sparkline legend) | `style="gap:var(--space-5);…"` | `.sparkline-legend` |
| `audit.rs` (filter row) | `style="gap:var(--space-3);…"` | `.filter-bar` |

**`inline-style-bound`: 16 → 10.**

### Tests, CI, and compatibility

- `cargo check --workspace` clean.
- **228/228 library tests pass**.
- `text-leaks` = 0, `css-tokens` = 148, `semantic-parity` = 36,
  **`inline-style-bound` = 10** (was 16; improved by 6).
- No handler changes; no data struct changes; no route changes;
  copy.js and `data-copy` pattern unchanged.

### Version bumps

`0.51.1` → `0.52.0` across workspace, crates, and Cargo.lock.

---

## [0.51.1] — 2026-05-18

**Phase 2 complete.** `RFC-MI-022` (Route-Based Tab Component)
ships — the last Phase 2 RFC. Phase 2 is now fully closed.

---

### Route-based tab component (`RFC-MI-022`)

**CSS** — `.route-tabs` + `.route-tabs__link` added to
`components/tabs.rs`. Active link identified by
`aria-current="page"` with a visible underline and colour change;
focus ring via `:focus-visible`; no colour-only state indicator.

**Rust** — `RouteTab { key, href, label }` struct and
`route_tabs(aria_label, current, tabs)` function added to
`components/tabs.rs`, re-exported from `components.rs`.

**`MeTab::Password` added** — `MeTab` gains a `Password` variant
(`key="password"`, href=`/me/security/password`). The self-service
tab strip now lists all six tabs: Overview, Password, MFA, Passkeys,
Sessions, Language. The `me_tab_password` i18n key is added to all
three locale files (en: "Password", ja: "パスワード", zh: "密码").

**Both tab helpers migrated to `.route-tabs` markup:**

- `me_security_tabs()` — was `<nav class="tabs">` + `<a class="tab
  tab--active">`; now `<nav class="route-tabs">` + `<a class="route-tabs__link"
  aria-current="page">`.
- `settings_tabs()` — was `<nav class="app-nav"
  style="margin-bottom:var(--space-4);flex-wrap:wrap">` + `<a
  class="app-nav__link">`; now `<nav class="route-tabs">` + `<a
  class="route-tabs__link" aria-current="page">`.

**`inline-style-bound` drops from 17 to 16.** The `style=`
attribute in `settings_tabs()` is the eliminated site.

**Deferred:** `ShellCurrent` typed enum (will land in a future
maintenance RFC); tab strip on `render_password_change` (owned by
RFC-MI-060, Phase 6).

### Tests, CI, and compatibility

- `cargo check --workspace` passes clean.
- **228/228 library tests pass**.
- `text-leaks` = 0, `css-tokens` = 148, `semantic-palette-parity`
  = 36, **`inline-style-bound` = 16** (was 17; improved).
- No route changes; no handler changes.
- The `me_tab_password` i18n key addition is additive and does not
  break existing code.

### Phase 2 complete — 7 of 16 MI RFCs now in `done/`

With RFC-MI-022, all three Phase-2 blockers (D-01, D-02, D-03) are
resolved and 7 of the original 16 MI RFCs have shipped.

### Version bumps

Workspace, all six crate `Cargo.toml`, and `Cargo.lock`:
`0.51.0` → `0.51.1`.

---

## [0.51.0] — 2026-05-18

**Phase 2 of the Mockup Integration arc opens with `RFC-MI-020`
(Shell Layout — decision record) and `RFC-MI-021` (Server-Rendered
CSRF).** The primary security improvement in this release is that
the admin sign-out form now works **with JavaScript disabled**.

---

### Shell layout decision (`RFC-MI-020`)

The product keeps its **top-nav model**. No structural shell code
changes. Decision recorded: the current seven-item horizontal nav +
Shell / AuthShell split satisfies all IA requirements from the
Phase-0 screen-map; a sidebar was not proven necessary. The
`ShellCurrent` enum (replacing `current: Option<String>`) is
deferred to RFC-MI-022, which touches the same call sites.

### Server-rendered CSRF for sign-out (`RFC-MI-021`)

- **`Shell` now requires `csrf_token: String`** — the token is
  threaded from every authenticated GET handler into the Shell's
  sign-out form as a server-rendered hidden field.
- **`logout-csrf.js` removed.** The script that read the
  `sui_id_csrf` cookie and injected it into the form before submit
  is deleted. `crates/sui-id/static/logout-csrf.js` no longer
  ships.
- **27 Shell call sites updated** across 19 page files; every call
  passes a real session-bound CSRF token.
- **5 render function signatures updated:** `render_dashboard`,
  `render_audit`, `render_settings_authentication`,
  `render_settings_logs`, `render_settings_other` — all now accept
  `csrf_token: String` (their handlers already issued the token and
  set the cookie; they now also forward it to the render layer).
- **`AuthShell` unchanged** — its pages pass CSRF tokens through
  their own per-page parameters; no Shell-level change needed.
- Sign-out remains a standard `<form method="post">` button;
  keyboard accessible; no JS dependency; CSRF contract unchanged.

### Tests, CI, and compatibility

- `cargo check --workspace` passes clean.
- **228/228 library tests pass**.
- All four CI invariants unchanged: `text-leaks` = 0,
  `css-tokens` = 148, `semantic-palette-parity` = 36,
  `inline-style-bound` = 17.
- No route changes; no handler contract changes; no data struct
  changes visible to callers of `render_*` functions in the public
  API of `sui-id-web` except the additional `csrf_token` parameter
  on 5 render functions.

### Version bumps

Workspace, all six crate `Cargo.toml`, and `Cargo.lock`:
`0.50.1` → `0.51.0`.

---

## [0.50.1] — 2026-05-18

**Phase 1 of the Mockup Integration arc completes.** `RFC-MI-011`
(Token Mapping + Visual Primitive Adoption) and `RFC-MI-012` (Theme
Persistence Decision) ship together. No screen layouts are changed;
this release prepares the component shards for Phase 2 adoption.

---

### Token mapping — zero new tokens (`RFC-MI-011`)

The Phase 0 inventory's headline finding is formally confirmed in
this release: **the product's 75-token vocabulary is a strict
superset of the mockup's 33 tokens.** No new CSS custom properties
are added. Every mockup spacing value rounds onto the existing
`--space-1..--space-6` scale (8–48 px); every mockup font-size maps
to the existing `--font-size-*` scale. `tokens.rs` is unchanged.

### Three new CSS primitives (`RFC-MI-011`)

Three CSS primitives land in the appropriate shards, ready for the
screen-level RFCs (Phase 2 onward) to use:

**`.callout` + tone variants** (→ `cards.rs`): A persistent
explanatory block for setup instructions, security notes, and
"read this before you proceed" copy. Neutral tone uses
`--surface-subtle` + `--border-muted`; four semantic tone variants
(`--info`, `--success`, `--warning`, `--danger`) follow the existing
semantic palette. Distinct from the existing `.card--callout`
(accent-filled CTA block).

**`.field__error` + `.field--invalid`** (→ `forms.rs`): Inline
validation error message (`--danger-default` text, caption size)
linked to its input via `aria-describedby`. `.field--invalid`
applies the red border to `input`, `textarea`, and `select` in the
field container without inline styles. Replaces the two ad-hoc
`style="color:red"` patterns identified in the Phase 0 inline-style
survey; those migrations are deferred to RFC-MI-050 (Phase 5).

**`.dl-grid`** (→ `utilities.rs`): A CSS-grid definition-list
wrapper for key-value displays on admin detail pages. Uses semantic
`<dl>/<dt>/<dd>`; replaces ad-hoc `<table>` usage for non-tabular
data. Screen-level migration deferred to RFC-MI-031 (Phase 3).

Three candidate primitives are explicitly deferred to their owning
Phase-5 RFC: `impact-summary` and `danger-zone` (RFC-MI-051) and the
route-tabs helper (RFC-MI-022 Phase 2). The `metric-card` pattern
is already covered by existing `.card + .stat` composition.

### Theme persistence decision record (`RFC-MI-012`)

**Option A chosen** — preserve the current `theme-init.js` +
`localStorage` model. No code change. Decision record committed to
`rfcs/done/RFC-MI-012-theme-persistence.md`. The mockup's
`/theme/{auto|light|dark}` server-side cookie routes remain
`do-not-implement-yet` per the Phase 0 screen-map inventory.

### Both Phase-1 RFCs → `rfcs/done/`

`rfcs/README.md` Proposed MI table now lists 12 entries (was 14).
RFC-MI-011 and RFC-MI-012 join RFC-MI-010 and RFC-MI-000 in the
Implemented table.

### Tests, CI, and compatibility

- `cargo check -p sui-id-web` passes clean.
- **228/228 library tests pass** (unchanged).
- All four CI invariants unchanged: `text-leaks` = 0,
  `css-tokens` = 148, `semantic-palette-parity` = 36,
  `inline-style-bound` = 17.
- No class names changed; no routes or handlers changed; no page
  layout changed.

### Version bumps

Workspace, all six crate `Cargo.toml`, and `Cargo.lock`:
`0.50.0` → `0.50.1`.

---

## [0.50.0] — 2026-05-18

**Phase 1 of the Mockup Integration arc opens with `RFC-MI-010`
(Component CSS Sharding).** The single monolithic
`crates/sui-id-web/src/components.rs` (1094 lines) is split into
eleven bounded shards under `components/`. This is the first release
in the MI arc that modifies **Rust source code**. No visible UI
change is introduced; the split is purely structural.

---

### Component CSS sharding (`RFC-MI-010`)

The `components.rs` CSS family is reorganised into eleven shards,
each owning one user-facing concern:

| Shard | Concern |
|---|---|
| `chrome.rs` | base reset, typography, Shell layout, page-header, theme toggle, responsive |
| `cards.rs` | card, panel, callout, metric, empty-state primitives |
| `forms.rs` | label, hint, validation, required marker |
| `tables.rs` | table, wrapping, copy-cell affordances |
| `buttons.rs` | button variants (primary, secondary, danger, ghost, link) |
| `banners.rs` | inline flash, standalone banners, dev-mode banner |
| `badges.rs` | `status_badge`, `StatusKind`, status CSS variants |
| `tabs.rs` | route-based tab strips |
| `confirm.rs` | reversibility badge, confirm-shell visual cues |
| `setup.rs` | auth-card centred layout, setup-wizard language picker |
| `utilities.rs` | RFC 067 bounded utility-class set |

`components.rs` becomes a 130-line umbrella that declares the
submodules, re-exports `StatusKind` and `status_badge` for backward
compatibility, and concatenates each shard's CSS in source order.

**Cascade preservation.** Several shards expose multiple sub-constants
(`CHROME_BASE_CSS`, `CHROME_TYPOGRAPHY_CSS`, `CHROME_LAYOUT_CSS`,
…) to capture the original interleaving order. Programmatic
verification: all 25 sub-constants concatenated in source order
produce a byte-identical CSS body (same MD5 after blank-line
normalisation) compared with v0.49.1's monolithic string.

**API adjustment.** The RFC's §6 sketch declared
`pub const COMPONENTS_CSS: &str = concat!(…)` over per-shard
constants. Rust's `concat!()` accepts only string literals, not
`const` items; the implementation instead exposes:

```rust
pub fn components_css() -> &'static str
```

backed by `std::sync::OnceLock<String>`. Both call sites in
`layout.rs` were updated. Full explanation in the RFC's
implementation note.

`StatusKind` and `status_badge` move to `components/badges.rs` and
are re-exported from `components.rs` so every existing import path
(`crate::components::StatusKind`, `sui_id_web::StatusKind`,
`crate::components::status_badge`) continues to compile unchanged.

### Tests, CI, and compatibility

- `cargo check --workspace` passes clean (0 errors, 0 warnings).
- **228/228 library tests pass** (same 6 executable test binaries,
  same counts per crate as v0.49.x).
- All four MI-tracked CI invariants hold at their v0.49.1 values:
  `text-leaks` = 0, `css-tokens` = 148 declarations,
  `semantic-palette-parity` = 36 (12 × 3),
  `inline-style-bound` = 17 (ceiling = 20).
- No class name changes anywhere in this RFC.
- No route or handler changes.
- Zero behaviour change: deploying v0.49.1 → v0.50.0 yields an
  identical rendered page for every screen.

### RFC-MI-010 → `rfcs/done/`

`rfcs/README.md` Proposed MI table now lists 14 entries (was 15);
RFC-MI-010 appears in the Implemented table alongside RFC-MI-000.

### Version bumps

- Workspace, all six crate `Cargo.toml`, and `Cargo.lock`:
  `0.49.1` → `0.50.0`.

---

## [0.49.1] — 2026-05-18

**Phase 0 of the Mockup Integration arc completes.** The six
baseline-inventory documents specified by `RFC-MI-000` are produced
and shipped under `docs/mockup-integration/inventory/`. `RFC-MI-000`
moves from `rfcs/proposed/` to `rfcs/done/` with its `Status` field
updated to `Implemented (v0.49.1)`. **No runtime code is changed in
this release.**

The inventory is the **frozen baseline** Phase 1 onward references.
Every cross-cutting structural question the migration plan raised
about the integration ("does the mockup need new tokens? how many
new strings is that? how do `?tab=` URLs become path-based URLs?
which mockup dangerous-action values survive and which are
rejected?") is now answered concretely.

---

### Six inventory documents shipped under `docs/mockup-integration/inventory/`

Each is the implementation contract for the screen-level RFCs that
follow. See the directory
[README](./docs/mockup-integration/inventory/README.md) for the
orientation overview.

| File | Headline finding |
|------|------------------|
| `screen-map.md` | 35 mockup routes mapped to product routes / render functions / handlers, classified into 5 status buckets (`ready-to-integrate`, `needs-visual-adaptation`, `requires-handler-change`, `requires-backend-review`, `do-not-implement-yet`). No route classified as `requires-backend-review` — the integration is web-layer-only. |
| `dangerous-action-map.md` | 18 mockup `?action=` values resolve to **9 link rewrites + 5 do-not-implement-yet + 3 step-up-policy-deltas + 1 inline-only**. The generic `/confirm/{token}` is rejected per migration plan §D-02 and `RFC-MI-051`; named confirm GETs are preserved. |
| `tab-routing-delta.md` | Mockup `?tab=…` query parameters are mechanically rewritten to product path-based slugs. Two renames (`passkey` → `passkeys`, `auth` → `authentication`); two mockup-only sub-states (`recovery`, `totp`) folded into MFA tab. Tab-helper API forward-declared for `RFC-MI-022`. |
| `token-delta-draft.md` | **The mockup introduces zero new CSS token names.** 33 mockup tokens are a strict subset of 75 product tokens. Mockup spacing rhythm (206 hardcoded pixel values) rounds onto the existing `--space-*` token scale per §D-05. Nine visual primitives proposed for adoption by `RFC-MI-011`. |
| `i18n-copy-delta-draft.md` | 382 mockup-only key names are mostly **renames** (~280 keys), some rewords (~50), and only ~58 are net-new copy. Translation effort: ~58 × 3 locales = ~174 entries spread across phases. The `impact_*` cluster (16 keys, `RFC-MI-051`) is the largest single net-new contribution. |
| `route-render-handler-map.md` | All 82 product routes documented with method, auth, CSRF, handler, render function, and audit event emission — the product-side reference for every MI implementer. |

### 21 decisions surfaced

Each inventory file lists the decisions it surfaces with a
recommended default. Five screen-level (`screen-D1`..`D5`), six
dangerous-action (`danger-D1`..`D6`), five token (`token-D1`..`D5`),
and five i18n (`i18n-D1`..`D5`). The defaults consistently preserve
the existing product surface; mockup intent is absorbed visually,
not structurally. None blocks Phase 1.

### `RFC-MI-000` moves to `rfcs/done/`

Per the lifecycle policy (`RFC 000-rfc-lifecycle-policy`), the file
location is the source of truth for status; the in-file `Status`
field updates to `Implemented (v0.49.1)`. The RFC's existing body is
preserved verbatim; a short implementation note is added at the top
explaining the location adjustment (inventory files live under
`docs/mockup-integration/inventory/` rather than the speculative
location in §6, to keep the `rfcs/` namespace clean per the
lifecycle policy).

The MI epic table in `rfcs/README.md` now lists 15 Proposed MI RFCs
(was 16); `RFC-MI-000` appears in the Implemented table.

### `ROADMAP.md`

The Mockup Integration arc phase table now reflects the two-step
Phase 0:

| Phase | Version  | What ships |
|------:|----------|------------|
| 0     | v0.49.0  | RFCs + planning artifacts (no runtime code) |
| 0     | v0.49.1  | Baseline delta inventory (this release; closes RFC-MI-000) |
| 1     | v0.50.0  | (Phase 1 begins — `RFC-MI-010` / `011` / `012`) |

The status block restates the verification phase position and the
"no rc / pre / beta tag" stance.

### Tests, CI, and compatibility

- `cargo check --workspace` passes clean.
- `cargo fmt --check`, `cargo clippy -D warnings` — pre-existing
  toolchain-drift warnings unchanged from v0.49.0 (no MI RFC changes
  them; addressed as a separate maintenance line, not in scope for
  MI).
- The four MI-tracked CI invariants (`text-leaks`, `css-tokens`,
  `semantic-palette-parity`, `inline-style-bound`) hold at the
  v0.49.0 values unchanged.
- 228/228 library tests pass.
- Zero source-code (.rs) files differ from v0.49.0.
- Zero runtime behaviour change: deploying v0.49.0 → v0.49.1 yields
  identical binary surface.

### Version bumps

- Workspace, all six crate Cargo.toml, and Cargo.lock: `0.49.0` →
  `0.49.1`.

---

## [0.49.0] — 2026-05-18

**Opens the Mockup Integration ("MI") development arc.** Sixteen
proposed RFCs (`RFC-MI-000` through `RFC-MI-080`) and the supporting
planning artifacts are introduced. **No runtime code is changed in
this release.** The MI arc adopts the `sui-id-web-mockup-v0.4.8`
UI/UX language into the product through an eight-phase, controlled
migration (Phase 0 → Phase 8); this release covers Phase 0 only —
freezing the baseline and making the integration auditable before
implementation begins.

The arc parallels the v0.42 → v0.48.0 hardening sequence (Phases
A–F). It is **not** a v1.0 candidate by itself: the verification
phase remains open, and the project owner's "no rc / pre / beta
tag is scheduled" stance carries through.

---

### Sixteen `RFC-MI-*` documents in `rfcs/proposed/`

Authored as a coherent set with their own parallel numbering line
(`MI-NNN`); the cross-reference graph between them stays intact under
their original names. Implementation order is the canonical reading
order:

| Order | ID         | Title                                                            | Phase    |
|------:|------------|------------------------------------------------------------------|----------|
| 1     | RFC-MI-000 | Baseline Delta Inventory and Integration Mapping Contract        | Phase 0  |
| 2     | RFC-MI-010 | Component CSS Sharding and Export Discipline                     | Phase 1  |
| 3     | RFC-MI-011 | Mockup Token Mapping and Visual Primitive Adoption               | Phase 1  |
| 4     | RFC-MI-012 | Theme Persistence Decision                                       | Phase 1  |
| 5     | RFC-MI-020 | Shell Layout Integration                                         | Phase 2  |
| 6     | RFC-MI-021 | Server-Rendered CSRF for Shell-Level Forms (Phase 2 blocker)     | Phase 2  |
| 7     | RFC-MI-022 | Route-Based Tab Component                                        | Phase 2  |
| 8     | RFC-MI-030 | Dashboard and Summary Surface Integration                        | Phase 3  |
| 9     | RFC-MI-031 | Audit Log and Read-Only Table Integration                        | Phase 3  |
| 10    | RFC-MI-040 | Setup Wizard UX Integration                                      | Phase 4  |
| 11    | RFC-MI-041 | Authentication Surface Integration                               | Phase 4  |
| 12    | RFC-MI-050 | Form System and Validation Feedback                              | Phase 5  |
| 13    | RFC-MI-051 | Danger Zone and Confirmation Screen Integration                  | Phase 5  |
| 14    | RFC-MI-060 | Self-Service Security Tab Integration                            | Phase 6  |
| 15    | RFC-MI-070 | OIDC Consent UX Integration                                      | Phase 7  |
| 16    | RFC-MI-080 | UI Regression and Accessibility Hardening                        | Phase 8  |

Phase-1 blockers (`D-01` / `D-02` / `D-03` in the migration plan)
are restated in `rfcs/README.md` and at the top of each affected
RFC: split `components.rs` into bounded shards, preserve path-based
tabs (reject the mockup's query-parameter tab model), and thread
CSRF through `Shell` server-side before any interactive shell
adoption proceeds.

### Parallel namespace recorded in `rfcs/README.md`

`rfcs/README.md` gains a "namespaces" preface and a dedicated
"Mockup Integration epic" subsection in the Proposed index. The
main sequential numbering line (`069` is the next free slot) is
unaffected. The MI line and the main line each retain permanent,
non-overlapping numbering per RFC 018.

### Planning artifacts under `docs/`

- `docs/development-specification.md` — the v3 development
  specification, reflecting the v0.48.4 codebase. Supersedes the
  v2 spec snapshot at v0.29.1.
- `docs/mockup-integration/README.md` — orientation index for the
  arc.
- `docs/mockup-integration/migration-plan.md` — the revised
  migration plan (v0.2), with the 12-item decision backlog
  (`D-01` … `D-12`), eight-phase roll-out, RFC dependency graph,
  and non-negotiable guardrails.
- `docs/mockup-integration/codebase-handoff.md` — the
  architect-facing tour of the v0.48.4 rendering stack, design
  system, handler contracts, CI invariants, and open questions.
  Generated against v0.48.4; the doc itself states it must be
  refreshed if more than two release cycles elapse before
  implementation starts.
- `docs/mockup-integration/mockup-handoff/` — the mockup author's
  handoff package (HANDOFF, SCREEN_INVENTORY, FLOW_SUMMARY,
  OPEN_ISSUES, IMPLEMENTATION_NOTES, README).

### Workspace version bumped to 0.49.0

`[workspace.package].version = "0.49.0"` in the root `Cargo.toml`.
Every crate inherits via `version.workspace = true`.

### No runtime code changes

This release follows RFC-MI-000 §4 (Non-Goals): *do not modify
runtime code*. The CI invariants therefore remain at their v0.48.4
values by construction:

- `cargo build`, `cargo test` — no change (228 / 228 floor
  unaffected; cf. spec §21).
- `text-leaks` — 0 occurrences (unchanged).
- `css-tokens` — every `var(--*)` reference still resolves
  (unchanged).
- `semantic-palette-parity` — 12 semantic tokens × 3 modes = 36
  declarations (unchanged).
- `inline-style-bound` — 16 inline `style="…"` occurrences in
  `crates/sui-id-web/src/pages/**` (unchanged; ≤ 20 ceiling).

### Next development action

The next release implements RFC-MI-000: produce the six inventory
files (`screen-map.md`, `dangerous-action-map.md`,
`tab-routing-delta.md`, `token-delta-draft.md`,
`i18n-copy-delta-draft.md`, `route-render-handler-map.md`) under
`rfcs/proposed/mockup-integration-inventory/` and move RFC-MI-000
to `rfcs/done/` once those artifacts are reviewed and frozen.
RFC-MI-010 / RFC-MI-011 / RFC-MI-012 (Phase 1) become eligible to
start once Phase 0 is closed.

### Verification phase continues

v0.49.0 is a planning release inside the verification phase
(spec §22). No v1.0-prefixed tag (`rc`, `pre`, `beta`) is
scheduled. Subsequent releases use `v0.49.x`, `v0.50.0`, … in
sequence as the MI phases ship.

---

## [0.48.4] — 2026-05-17

**Verification-phase UX improvements: setup token via URL parameter;
setup wizard Chinese language option removed.**

---

### Setup token as URL parameter

Previously the server printed a raw token string to stderr:

```
  Setup token (one-time, stays only in this process):
    Xk7q9...
```

Operators had to copy that string and paste it into a password-type
input field on the admin-form screen. One mistype or partial-select
sent the form back with a token error.

v0.48.4 changes the printed output to a **full, click-to-open URL**:

```
  Open the following URL in your browser to begin setup:
    http://localhost:8801/setup?token=Xk7q9...
```

The welcome screen reads `?token=xxx` from the query string and
forwards it through to the admin-form link (`/setup/admin?token=xxx`).
The admin-form page renders the token as a `<input type="hidden">`
rather than a visible `<input type="password">`. The operator sees
a form with only the fields they need to fill in themselves:
username, email, display name, password, and password confirmation.
The token travels invisibly through the form body and is validated
by the POST handler exactly as before.

**Changes:**

- `crates/sui-id/src/startup.rs`: new startup banner prints the
  full URL with `?token=` instead of the raw token string.
- `WelcomeQuery` (setup handler): gains a `token: Option<String>`
  field. The language PRG redirect preserves the token with
  `?token=xxx` so language-switching on the welcome screen doesn't
  lose it.
- `render_setup_welcome`: takes a `token: &str` parameter; the
  "Begin setup" button href and the language-picker links include
  the token in their query strings.
- `admin_get` handler: new `SetupAdminQuery { token }` extractor.
  `render_setup_admin` called with the token string.
- `render_setup_admin`: takes a `token: &str` parameter; visible
  `<input type="password">` for the token is replaced by
  `<input type="hidden" value=token>`. The `autofocus` moves
  to the username field — the first field the operator actually
  types into.
- Error-path re-renders (password mismatch, HIBP block, token
  invalid) all pass `&form.setup_token` through so the hidden
  input retains its value across form rejections.

**Security note.** The token appears in the URL (and thus in the
browser's history, any web-server access log, and any referrer
header if the page links to external resources). Setup pages link
to no external resources; the `/setup` path is only accessible
before the system is initialized; and the token is single-use plus
process-scoped. The tradeoff is the same as any "magic link" setup
flow and is acceptable for a local-install setup wizard. Operators
with stricter requirements can continue to extract the token from
the URL and supply it programmatically.

### Setup wizard: Chinese language option removed

The setup wizard's two-language picker (`日本語 / English`) no
longer shows a `中文` option. The core i18n support covers only
Japanese and English; displaying a Chinese option that resolves to
a partially-complete translation would be misleading. The Chinese
locale strings remain in the codebase for completeness.

**Change:** `render_setup_welcome` in `pages/setup.rs` — the third
picker button and its corresponding href are removed.

### Tests pass count

**228/228** unchanged.

---

## [0.48.3] — 2026-05-17

**Verification-phase bug fix: `email` claim missing from ID token.**

An external OIDC relying party reported:

```
OIDC callback failed: JSON error: missing field `email` at line 1 column 264
```

The RP was decoding the ID token JWT (not calling the UserInfo
endpoint) and requiring `email` to be present. The ID token did
not contain an `email` field at all — only the UserInfo endpoint
(`/userinfo`) returned email claims, and only when the access
token's scope included `"email"`.

---

### Root cause

`IdTokenClaims` in `crates/sui-id-core/src/tokens.rs` contained
only: `iss`, `sub`, `aud`, `iat`, `exp`, `nonce`, `jti`, `acr`,
`amr`. No `email` or `email_verified`.

OIDC Core §5.1 states that the `email` scope maps to the `email`
and `email_verified` claims, which SHOULD appear in the ID token
(not exclusively in the UserInfo response). Many OIDC clients
parse the ID token at callback time and expect the scoped claims
to be present there.

### Fix

`IdTokenClaims` gains two new optional fields, both with
`#[serde(skip_serializing_if = "Option::is_none")]`:

```rust
pub email: Option<String>,
pub email_verified: Option<bool>,
```

`issue_token_set` gains a new parameter
`user_email: Option<(&str, bool)>` (address + verified status).
When `scope` contains `"email"` **and** `user_email` is `Some`,
the claims are populated; otherwise both fields are absent from
the serialised JWT. The `Option::is_none` skip ensures `email`
is never `null` in the payload — some client libraries treat an
explicit `null` as a type error when expecting `String`.

Both code-exchange and refresh-token-exchange paths now supply
the user's email to `issue_token_set`:

- **`exchange_code`**: the user row was already fetched for the
  disabled/deleted check; `user.email` + `user.email_verified_at`
  are passed directly.
- **`exchange_refresh`**: adds a conditional `users::get` call
  when `row.scope` contains `"email"`, avoiding the extra DB
  round-trip on token refreshes that never requested the email
  scope.

### Unchanged

- UserInfo endpoint still returns `email` / `email_verified` via
  `/userinfo` as before — the two paths are now consistent.
- Accounts without an email address return no `email` claim in
  either path (omitted, not `null`).
- `email_verified` is always `false` until a verification flow
  is implemented; the field faithfully represents the current
  DB state (`email_verified_at IS NULL`).
- The `oauth_token` handler and router are unchanged.

### Tests pass count

**228/228** unchanged.

### CI invariants

All 4 PASS (text-leaks / css-tokens / semantic-palette-parity /
inline-style-bound 16/20).

---

## [0.48.2] — 2026-05-17

**Verification-pass buffer.** Six issues surfaced in the same
real-environment verification round that produced v0.48.1. None
of them locked operators out (those were fixed in v0.48.1); all
six are visual or UX regressions worth fixing before the project
moves further into the verification cycle.

This is the **second verification-phase release**. No v1.0-*
tag is scheduled.

---

### Bug 1 — `::selection` color invisible in light mode

`::selection` used `--accent-subtle` as the background colour. In
light mode that is a pale lavender (`#E6E1F5`) sitting on an
off-white page (`#FAFAFA`); the selection *technically* had a 13:1
contrast ratio for its text, but the highlight shape itself was
almost indistinguishable from the surrounding page — the background
contrast was near-zero.

Fix: `background-color: var(--accent-default)` + `color:
var(--fg-on-accent)` (white). Light mode: `#7C6BCF` on white
text — selection shape strongly visible against page. Dark mode:
`#A89BFF` on white text — similarly strong. The updated comment
correctly attributes WCAG SC 1.4.3 to text-vs-selection, not
selection-vs-page.

### Bug 5 — `/me/security/overview` i18n broken

Two labels and one empty-state message were left in hardcoded
English after the `pages.rs` split in v0.47.0:

- `kv_bool_badge(t, "MFA (TOTP)", …)` — `"MFA (TOTP)"` hardcoded
- `kv_row("Passkeys", …)` — `"Passkeys"` hardcoded
- Empty recent-activity panel used `t.me_security_sessions_lede`
  (which reads "you have N other active sessions") — completely
  wrong context

Fix: added three i18n keys in all three locales (en/ja/zh):

| Key | en | ja | zh |
|-----|----|----|-----|
| `me_overview_label_mfa_totp` | "MFA (TOTP)" | "MFA（TOTP）" | "MFA（TOTP）" |
| `me_overview_label_passkeys` | "Passkeys" | "パスキー" | "通行密钥" |
| `me_overview_no_recent_events` | "No recent activity to display." | "最近の操作はまだ記録されていません。" | "暂无最近活动记录。" |

`overview.rs` updated to use the new keys.

### Issue 4 — Setup wizard explicit language picker

The wizard showed in whatever language the browser's
`Accept-Language` header indicated — correct by design, but
surprising when an operator's OS is English but the target
installation is intended to be Japanese-locale (or vice versa).
Prior to the wizard there is no user record and no stored
preference, so the only control point was the browser.

Fix: an explicit three-button language picker (`日本語 / English /
中文`) appears at the top of the welcome screen. Clicking a language
button issues a `GET /setup?lang=xx`, which the handler validates
with `Locale::parse`, sets `LANG_COOKIE` (365-day, Same-Site Lax,
not HttpOnly, same as the post-setup language cookie), and issues a
`303 → /setup` (PRG pattern). Every subsequent wizard step
(`/setup/admin`, `/setup/lang`, `/setup/hibp`, `/setup/done`) reads
`LANG_COOKIE` via the existing three-tier `RequestLocale` extractor
and renders in the chosen language without any per-step changes.

CSS: `.setup-lang-picker` (horizontal flex, caption-size, muted
border) with `.setup-lang-picker__opt--active` (accent colour +
subtle fill). On `@media (max-width: 768px)` the picker wraps.

### Issue 6 — Footer a11y label design intent

The three footer spans — "⌨ Keyboard support", "⊙ Screen reader
support", "◐ Contrast support" — looked interactive (body-weight
text, tooltip-on-hover) but had no click action and no href. The
design-spec intent is **passive informational claims**: the app
asserts that it respects these accessibility affordances.

Redesign: converted from bare `<span>` to `<ul role="note">` /
`<li class="app-footer__a11y-item">`. Each item has
`cursor: default` (removes pointer affordance), `font-size:
var(--font-size-caption)` and `color: var(--fg-muted)` to read
as ancillary rather than primary navigation, and the icon is
wrapped in `<span aria-hidden="true">` so screen readers announce
the text only once.

The `title=` tooltips remain for mouse users who want a longer
description. Future work (post-v1.0): when `docs/src/a11y.md`
exists, convert to `<a href="/docs/a11y#...">` links.

### Issue 7 — Title tagline restrained

The footer tagline `sui-id · 静かで、凛として、やさしい ID 基盤を。`
rendered at full body weight and colour, competing visually with
the theme toggle (which operators use). Restyled to
`font-size: var(--font-size-caption)`, `color: var(--fg-muted)`,
`opacity: 0.75` — still present as "a whisper of intent", now
clearly recessive.

### Bug 8 — Mobile responsive: nav and table vertical squish

The entire CSS had no `@media` queries (0 instances in v0.48.1).
On viewports narrower than ~600px, two classes of rendering
failure occurred:

1. **Admin nav**: each nav link had no `white-space: nowrap`. The
   flexbox would shrink items to fit, causing text to wrap *inside*
   each link, making `Dashboard` a two-line tall block, etc.
2. **Tables**: all `td` and `th` had no `white-space` control.
   Same shrink-and-wrap behaviour made every cell grow vertically.
   The `.table-wrap { overflow-x: auto }` was already in place but
   `table { width: 100% }` prevented it from triggering.

Fixes applied:

- `.app-nav__link { white-space: nowrap }` — items never wrap; the
  nav row scrolls horizontally instead.
- `thead th, tbody td { white-space: nowrap }` default — cells
  stay single-line. Added `.cell-wrap` class for opt-out on free-
  form-text columns (audit log notes, descriptions, display names).
  Column `.cell-wrap` annotation is a per-table follow-up; the rule
  prevents the worst failure mode today.
- `@media (max-width: 768px)` breakpoint (first `@media` rule in
  the codebase):
  - `.app-main { padding: var(--space-3) }` (was `var(--space-5)`;
    two `32px` margins ate ~17% of a 375px viewport)
  - `.app-nav { overflow-x: auto; flex-wrap: nowrap }`
  - `.app-nav__signout { margin-left: var(--space-1) }` (no longer
    pushed to unreachable far-right in scroll context)
  - `.app-header__brand` shrinks one step (`font-size: h3`)
  - `.app-footer { flex-direction: column }` — stacks at narrow
    widths
  - `.card { padding: var(--space-3) }`

### Tests pass count

Unchanged: **228/228**.

### CI invariants

All 4 PASS:

- text-leaks (RFC 048): 0
- css-tokens (RFC 049): all `var(--name)` resolve
- semantic-palette-parity (RFC 061): 12 tokens × 3 modes
- inline-style-bound (RFC 067): 16 ≤ 20

### Breaking changes

None.

### Known follow-up (v0.48.3+)

- **`.cell-wrap` per table**: audit, users, clients, sessions,
  signing-keys tables should annotate their free-text columns
  (note, email, name) with `.cell-wrap` so those columns can
  still word-wrap while others remain single-line.
- **`?return=` on login redirect**: `html_error_response`
  redirects to `/admin/login` without a return URL; implementing
  it requires same-origin path-only validation (v0.48.1 rationale).
- **CSRF server-render**: the `logout-csrf.js` workaround
  (`csrf_token` in Shell) is the proper fix; it requires threading
  csrf_token through every `render_*` call site.

---

## [0.48.1] — 2026-05-17

**Verification-phase hotfix.** Three serious bugs surfaced in
actual-environment testing of v0.48.0 at `localhost:8801` after
v0.48.0 was tagged. All three share a single class of root cause:
the codebase grew CSP-unsafe inline JavaScript and an authentication
flow that assumed JavaScript would always run. With
`Content-Security-Policy: script-src 'self'` (the production
default), the inline scripts are blocked, and the assumption
collapses into user-visible failures.

This is the **first verification-phase release**. Per project
guidance, no v1.0-* tag is scheduled — v0.48.1 is a defensive
patch to keep the verification phase running until further bugs
are surfaced and a planned verification-pass buffer can ship.

---

### Bug 2 (CSP blocks inline scripts → theme toggle, copy, sign-out broken)

Browser developer tools showed multiple CSP violations on every
page under the admin chrome:

- `script-src-elem 'self'` blocked the three inline `<script>`
  blocks in `crates/sui-id-web/src/layout.rs` (theme init, copy
  helper, logout CSRF injector).
- `script-src-attr 'self'` blocked the three inline `onclick=`
  handlers on the footer theme-toggle buttons.

All three inline scripts were extracted into external files served
by the existing `/static/*` route (which already serves the
WebAuthn JS):

| New file | Replaces | Loaded as |
|----------|----------|-----------|
| `crates/sui-id/static/theme-init.js` | inline `THEME_INIT_JS` (in 2 places: Shell + AuthShell) + 3 inline `onclick=` | `<script src="/static/theme-init.js" defer>` |
| `crates/sui-id/static/copy.js` | inline `COPY_JS` | `<script src="/static/copy.js" defer>` |
| `crates/sui-id/static/logout-csrf.js` | inline CSRF-cookie-to-form-input injector | `<script src="/static/logout-csrf.js" defer>` |

The theme-toggle buttons now carry only `data-theme-value="..."`
attributes; `theme-init.js` attaches `click` listeners on DOM ready
by selecting `.theme-toggle__btn[data-theme-value]`. The functional
behaviour is identical to the inline version that v0.47.x shipped.

`assets.rs::mime_for()` learnt the `.js → application/javascript;
charset=utf-8` mapping; previously only `.ico`, `.png`, `.svg`,
and `.txt` were known.

### Bug 3 (Sign-out redirect loop)

Subsumed by Bug 2 fix. The root cause:

1. The logout `<form>` had `<input id="logout-csrf" value="">` —
   the value was supposed to be populated client-side from the
   `sui_id_csrf` cookie by an inline script.
2. CSP blocked the script; the input stayed empty.
3. POST `/admin/logout` saw an empty `_csrf` field, `enforce_csrf`
   failed, the handler took its "graceful" fallback —
   `Redirect::to("/admin/login")`.
4. `/admin/login` saw a still-valid session cookie (logout never
   ran) and redirected back to `/admin`.
5. The operator appeared to be unable to sign out.

Externalising the inline script (`logout-csrf.js`) restores the
behaviour. The proper fix — server-rendering the CSRF token into
the form so no JavaScript injection is needed — is deferred to
v0.48.2+ because it touches every `render_*` call site (Shell
takes `csrf_token: String` would have to plumb through dozens of
handlers).

### Bug 9 (401 lock-out + redirect loop after server restart)

A user reported: stop the service, restart, navigate to `/admin` →
greeted with a 401 page (`リクエストを処理できませんでした`) with
only a "Back home" button → which round-trips through `/` →
`/admin` → 401 again, forever. Only the request ID changed across
attempts.

Root cause: two latent design issues in combination.

1. **`html_error_response`** (`crates/sui-id/src/errors.rs`)
   rendered `CoreError::Unauthenticated` as a 401 page rather than
   a redirect. The proper pattern for a protected GET hit by an
   unauthenticated user is `303 → /admin/login`, not `401 → page`.
   The 401 page should only fire for genuine error conditions
   (malformed cookie, server failure).
2. **`pages::error::render_error`** had `<a href="/">` for the
   "Back home" button. `/` is the root handler, which for any
   *initialised* installation (the common case) redirects to
   `/admin`. The 401 page therefore had no escape — every Back
   click was a fresh attempt at the page that produced the 401.

Both are fixed:

- `html_error_response` now detects
  `CoreError::Unauthenticated`-with-HTML-representation and
  returns `Redirect::to("/admin/login")` instead of rendering a
  page.
- `render_error` makes the "Back home" link context-aware: status
  401 → `/admin/login`, everything else → `/`. This is
  defense-in-depth: even if a future code path produces a 401 page
  by some other route, the operator isn't trapped.

The `?return=<original-url>` query parameter on the redirect is
deliberately **not** included in this hotfix. Allowing arbitrary
return URLs is an open-redirect class issue; doing it correctly
needs same-origin path-only validation and is v0.48.2+ work.

### CI invariants

All 4 grep CI jobs still PASS after the hotfix changes:

- text-leaks (RFC 048): 0 leaked `>t.foo<` literals
- css-tokens (RFC 049): every `var(--name)` resolves
- semantic-palette-parity (RFC 061): 12 semantic tokens × 3 modes
- inline-style-bound (RFC 067): 16 ≤ 20

### Tests pass count

Unchanged: **228/228**.

- sui-id-i18n: 12
- sui-id-shared: 13
- sui-id-store: 36
- sui-id-core: 114
- sui-id: 53

### Breaking changes

None.

### Deferred to v0.48.2+

Six issues surfaced in the same verification round but did not lock
operators out, so they are scheduled for v0.48.2 (a regular
verification-pass buffer release, *not* a v1.0 candidate):

- **Bug 1**: `::selection` background color (`--accent-subtle`) is
  too close to `--surface-default` to be visible in light mode.
  Needs `--accent-default` with `--fg-on-accent`.
- **Bug 5**: `/me/security/overview` has two hardcoded English
  labels (`"MFA (TOTP)"`, `"Passkeys"`) and one wrong i18n key
  (`me_security_sessions_lede` used as the empty-events fallback).
- **Bug 8**: mobile responsive — no `@media` queries in
  `components.rs` and no `white-space: nowrap` on nav links /
  table cells. Result: on narrow viewports, items get squeezed and
  text wraps inside each item, turning nav links and table cells
  into vertical stacks. Horizontal scope confirmed — affects both
  the admin nav and content tables.
- **Issue 4** (UX): the setup wizard appears in English when the
  browser's `Accept-Language` is `en-*`. Resolution is correct
  per design, but the v0.48.1 user reaction suggests adding an
  explicit language picker at the top of the wizard.
- **Issue 6** (UX): footer accessibility labels (Keyboard
  support / Screen reader support / Contrast support) are
  decorative `<span>` elements with tooltips — they look
  clickable but aren't. Intent needs to be clarified and the
  UI/UX redesigned per the design-spec intent.
- **Issue 7** (UX): the footer tagline `sui-id ·
  静かで、凛として、やさしい ID 基盤を。` is visually prominent and
  could be styled more restrained. CSS-only adjustment.

---

## [0.48.0] — 2026-05-17

**Phase F final buffer: `handlers/me_security.rs` split (RFC 068)
plus inline-style discipline (RFC 067).** Phase F began at v0.47.0
with `pages.rs` (4170 → 22 files), continued at v0.47.1 with
`handlers/admin.rs` (1531 → 8 files), and closes here with the last
oversize handler split and the visual-style pass. After this
release, every `.rs` file in `crates/` fits inside the project
spec's 500-LOC ceiling, and inline `style=""` attributes are bound
by CI.

Visible signal: nothing user-facing. This is the final buffer
release for structural hygiene before opening verification cycles
toward an eventual v1.0 candidate. The project is **not** going
straight to v1.0-rc1 from here — sufficient soak-time and external
verification come first.

A few pre-existing warnings carried over from earlier releases are
also cleaned up in v0.48.0; details below.

---

### RFC 068 — `handlers/me_security.rs` split per tab domain

The 1099-line `handlers/me_security.rs` is split into 7 child
modules under `crates/sui-id/src/handlers/me_security/`, mirroring
the 6-tab layout in `crates/sui-id-web/src/pages/me_security/`.
Rust 2018+ module style is used throughout — `me_security.rs` is
the umbrella; submodules live in `me_security/` as sibling .rs
files. No `mod.rs`.

**New module tree:**

```
crates/sui-id/src/handlers/
├── me_security.rs        # umbrella (87 LOC): 2 redirects,
│                         # describe_auth_methods, flash_from_query,
│                         # mod declarations, pub use *
└── me_security/
    ├── forms.rs       (~100 LOC) # 10 form/query structs
    ├── overview.rs    (~150 LOC) # overview_get + legacy page_get
    ├── mfa.rs         (~240 LOC) # 5 handlers + render_mfa_tab_with_fresh_codes
    ├── sessions.rs    (~190 LOC) # sessions_tab_get, revoke_one, revoke_all_others
    ├── passkey.rs     (~220 LOC) # 5 passkey handlers
    ├── language.rs    (~75 LOC)  # language_get/post
    └── password.rs    (~165 LOC) # password_change_get/post
```

**All 8 files under 500 LOC.**

**Public API unchanged.** `crate::handlers::me_security::*` paths
resolve identically because the umbrella re-exports each submodule
via `pub use {submodule}::*;`.

**`describe_auth_methods` + `flash_from_query` placement**: kept in
the umbrella as `pub(super)` so the overview and sessions
submodules (both callers) can reach them through `use super::*;`.

**Build hygiene during the split:**

- 10 `#[derive(Debug, Deserialize)]` attributes on form structs
  detached during line-range extraction (same issue as RFC 066)
  and were re-attached from the backup file.
- 5 sub-files had `SESSION_COOKIE` / `RECENT_EVENT_LIMIT` constants
  carried over from the common header but never used in that file —
  removed.
- 56 unused-import warnings (each submodule inherited a wide `use`
  block) auto-pruned by `cargo fix --lib`.

### RFC 067 — Inline-style discipline + CI bound

The Phase F survey found **119 inline `style=""` attributes** in
the (now split) `pages/` tree. v0.48.0 sweeps **103 of them** into
**40+ utility classes** in `components.rs`, leaving **16
one-off / truly-dynamic styles** that don't repeat anywhere.

**Utility classes added** (token-derived; every value resolves
through a `--space-*`/`--font-*`/`--accent-*` token):

| Family | Classes |
|--------|---------|
| Margin top | `.mt-1`, `.mt-2`, `.mt-3`, `.mt-4`, `.mt-5` |
| Margin bottom | `.mb-0`, `.mb-1`, `.mb-2`, `.mb-3`, `.mb-4` |
| Margin left | `.ml-1`, `.ml-2` |
| Margin combo | `.mt-2-mb-0` |
| Gap (flex/grid) | `.gap-1`, `.gap-2`, `.gap-3` |
| Layout | `.center`, `.items-center`, `.items-end`, `.justify-end`, `.justify-between`, `.inline-el`, `.inline-block`, `.flex-1`, `.flex-0-auto` |
| Composite rows | `.row-gap2-center`, `.row-gap2-center-clickable`, `.row-gap3-center`, `.gap1-center` |
| Widths | `.max-w-card`, `.max-w-narrow`, `.min-w-16rem` |
| Typography | `.text-caption`, `.text-small`, `.fw-medium`, `.fw-500` |
| Colour | `.color-accent`, `.color-danger` |
| Patterned | `.kv-label-cell`, `.clickable-block`, `.radio-hint`, `.center-pad-4`, `.center-pad-6`, `.center-pad-6-muted`, `.ul-indent`, `.button-reset` |

**CI bound** (new job `inline-style-bound`): inline `style=""`
attribute count in `pages/**.rs` ≤ **20**. Below the bound today
(16); a PR that adds 5 more inline styles trips the gate.

The bound deliberately is **not zero** — the 16 remaining are
genuinely one-off (`width:100%;height:80px` for a chart
container; `max-width:240px` on a single QR image; an `opacity:0.85`
intentional dim for the dashboard sparkline title). Adding utility
classes for these would be premature abstraction.

### Pre-existing warning cleanup

Five warnings carried over from earlier releases. All cleared in
v0.48.0:

| Symbol | File | Reason | Action |
|--------|------|--------|--------|
| `mailer` | `crates/sui-id/src/startup.rs` | Built but never wired into `AppState`. RFC 001 outbox path replaced the direct sender; the unused build was a leftover. | Removed (plus `ehlo_hostname_from_issuer`, which only the deleted mailer called). |
| `title` | `crates/sui-id/src/errors.rs` | Per-status title mapping fed a manual error renderer; RFC 042's `sui_id_web::render_error` derives the title from the status code internally. | Removed. |
| `caches` (×2) | `crates/sui-id-core/src/admin.rs` (`create_client`, `update_client`) | Accepted for API symmetry; both functions modify `redirect_uri`s and **should** rebuild `caches.redirect_origins`. They currently don't. | Renamed `_caches` with a comment noting the latent miss; cache rebuild remains a follow-up bug. |
| `clock` | `crates/sui-id-core/src/admin.rs` (`set_client_disabled`) | Accepted for API symmetry; the audit row gets its timestamp via `audit::append` internally. | Renamed `_clock`. |
| `decrypt_field` | `crates/sui-id-core/src/mail/outbox.rs` | Symmetric pair of `encrypt_field`, reserved for a future outbox replay/inspection path. | `#[allow(dead_code)]` with comment. |

After v0.48.0, `cargo check --workspace` reports **0 warnings**.

### CI invariants verified

All 4 grep CI jobs (text-leaks, css-tokens, semantic-palette-parity,
new inline-style-bound) PASS:

- text-leaks (RFC 048): 0 leaked `>t.foo<` literals
- css-tokens (RFC 049): every `var(--name)` resolves
- semantic-palette-parity (RFC 061): 12 semantic tokens × 3 modes = 36 declarations
- inline-style-bound (RFC 067, new): 16 ≤ 20

### Tests pass count

Unchanged from v0.47.1: **228/228**.

- sui-id-i18n: 12
- sui-id-shared: 13
- sui-id-store: 36
- sui-id-core: 114
- sui-id: 53

### Breaking changes

None. Public API surface unchanged across all crates.

### Phase F complete (with honest scope qualification)

After v0.48.0:

- **The three files originally identified in the Phase F scope —
  `pages.rs` (4170), `handlers/admin.rs` (1531), and
  `handlers/me_security.rs` (1099) — are all split into per-screen
  / per-domain submodules under 500 LOC.** This was the entire
  Phase F mandate from the v0.41.0 codebase review.
- **Inline `style=""` count bounded by CI** at 20 (currently 16).
- **0 compiler warnings** workspace-wide.

Ten other `.rs` files in the workspace are still over the spec's
500-LOC *recommendation* (not a hard cap):

| File | LOC | Note |
|------|----:|------|
| `crates/sui-id/src/backup.rs` | 1064 | Backup/restore CLI subsystem; one domain, one file |
| `crates/sui-id-i18n/src/strings.rs` | 808 | The `Strings` struct definition; can't split without breaking i18n API |
| `crates/sui-id-core/src/admin.rs` | 779 | Use-case layer (different file from the split `handlers/admin.rs`) |
| `crates/sui-id/src/handlers/oidc.rs` | 741 | OIDC token + discovery + JWKS handler bundle |
| `crates/sui-id-i18n/src/en.rs` | 739 | English translation table |
| `crates/sui-id-i18n/src/zh.rs` | 742 | Chinese translation table |
| `crates/sui-id-i18n/src/ja.rs` | 738 | Japanese translation table |
| `crates/sui-id-core/src/step_up.rs` | 697 | Step-up re-authentication state machine |
| `crates/sui-id-core/src/session.rs` | 688 | Session lifecycle (cookie + DB row management) |
| `crates/sui-id-core/src/authorize.rs` | 632 | OIDC authorization endpoint logic |

These were never in Phase F's scope — they were not flagged in the
v0.41.0 PDF audit that defined the Phase A–F arc. The i18n table
files are single bags of strings by design; splitting them adds
indirection without improving editability. The core files are
state-machine / use-case implementations where splitting harms
cohesion more than it helps the LOC count. Genuine refactor
candidates among them (e.g. `backup.rs`) are tracked as separate
proposed RFCs for post-1.0.

The project enters **verification phase**. v1.0 candidate tags
(`v1.0-rc*`, `v1.0-pre*`) are **not** scheduled from this release —
sufficient soak time, external review, and an independent
integration check come first. The next planned release is a
verification-pass buffer; tag name TBD and **will not start with
v1**.

---

## [0.47.1] — 2026-05-17

**Phase F continuation: `handlers/admin.rs` split per screen domain
(RFC 066).** This is the second of three Phase F releases; RFC 067
(inline-style discipline) plus `handlers/me_security.rs` split land
in v0.48.0. After v0.48.0, v1.0-rc1 is the next planned tag.

The visible signal of v0.47.1 landing: nothing user-facing — pure
code-structure refactor. Contributors editing one handler domain no
longer scroll past nine others to find it.

---

### RFC 066 — `handlers/admin.rs` split per screen domain

The 1531-line `handlers/admin.rs` is split into 8 child modules
under `crates/sui-id/src/handlers/admin/`, mirroring the route
prefixes (`/admin/users/*`, `/admin/clients/*`, etc.). Rust 2018+
module style is used throughout — `admin.rs` is the umbrella;
submodules live in `admin/` as sibling .rs files. No `mod.rs`.

**New module tree:**

```
crates/sui-id/src/handlers/
├── admin.rs                # umbrella: pub use {submodule}::*; +
│                           # `with_csrf_cookie` + `render_qr_svg(_pub)` +
│                           # 8 mod declarations
└── admin/
    ├── forms.rs       (~70 LOC)  # DisableForm, CsrfOnlyForm,
    │                              # ConfirmedForm, ConfirmedReasonForm
    ├── auth.rs        (~275 LOC) # login_get/post, mfa_challenge_get/post,
    │                              # logout, LoginForm, MfaChallengeForm
    ├── dashboard.rs   (~115 LOC) # dashboard handler + DashboardQuery
    ├── users.rs       (~370 LOC) # 9 handlers: list, create, set_disabled,
    │                              # delete, mfa_reset, detail_get, +
    │                              # 3 confirm_get pages
    ├── clients.rs     (~360 LOC) # 8 handlers: list, create, set_disabled,
    │                              # delete, edit_get/post, rotate_secret,
    │                              # + delete confirm_get
    ├── signing_keys.rs (~100 LOC) # 4 handlers: list, rotate, delete +
    │                              # delete confirm_get
    ├── audit.rs       (~80 LOC)  # audit_get, audit_csv_get, AuditQuery
    └── webauthn.rs    (~145 LOC) # login challenge: webauthn_auth_start +
                                  # webauthn_auth_complete (distinct from
                                  # /me/security/passkeys/* which live in
                                  # handlers/me_security.rs)
```

**Every file under 500 LOC.** umbrella `admin.rs` is 55 LOC.

**Public API unchanged.** Routes wired in `crate::router` reference
`crate::handlers::admin::handler_name` — each submodule's `pub`
items are flattened into the admin namespace by the
`pub use {submodule}::*;` re-exports. The router file needs no
changes; `crate::handlers` declaration in handlers.rs needs no
changes (`pub mod admin;` already pointed at admin.rs which is now
the umbrella).

**`render_qr_svg` placement**: kept in the umbrella as a private
helper plus a `pub fn render_qr_svg_pub` wrapper. Called both from
`mfa_challenge_post` inside admin (at first secret generation) and
from `crate::handlers::me_security::mfa_enroll_start` (RFC 055).
Moving it into a submodule would force a sibling-to-sibling import
and lose nothing in clarity. Sui-id-web is the eventual home but
the move is out of scope for v0.47.1.

**`_silence_state` / `_silence_state2` removed.** These were
dead-code suppressors in the monolithic admin.rs that referenced
otherwise-unused-in-some-paths imports (`CurrentUser` and
`state::is_initialized`). After the split, each submodule declares
only what it needs; the suppressors are unnecessary.

**Build hygiene during the split:**

- 14 `pub struct` types lost their `#[derive(Debug, Deserialize, …)]`
  during extraction because the derive line was claimed as the
  trailing content of the preceding item. Fixed by extracting the
  derives back from the backup admin.rs and inserting them above
  each affected struct in the new files.
- 85 unused-import warnings (every submodule inherited the full
  monolithic `use` block) auto-pruned by `cargo fix --lib`.
- `axum::Json` (used by `webauthn_auth_start` to return a challenge
  payload) was missing from the inherited use block; added manually
  to `admin/webauthn.rs`.
- The pre-existing v0.47.0 warnings (`mailer`, `title`, three
  `caches`/`clock` in sui-id-core) carry forward unchanged —
  unrelated to this RFC; tracked separately.

### CI invariants verified

The three existing grep CI jobs (text-leaks, css-tokens,
semantic-palette-parity) scope by `crates/`, not by filename — they
automatically follow the new file structure. Manual verification
on v0.47.1 post-split passed all three. No CI workflow changes
needed.

### Tests pass count

Unchanged from v0.47.0 — this is a structural release.
**228/228 PASS**:

- sui-id-i18n: 12
- sui-id-shared: 13
- sui-id-store: 36
- sui-id-core: 114
- sui-id: 53

### Breaking changes

None. `crate::handlers::admin::*` paths resolve identically.

### Deferred

- **RFC 067** (inline-style discipline; ~119 inline `style=""` →
  ~30 with `.mt-*`/`.gap-*` utility classes + CI bound at 40) →
  **v0.48.0**.
- **`handlers/me_security.rs` split** (1099 LOC, also over the 500
  spec ceiling; same Rust 2018+ pattern as admin.rs) → **v0.48.0**.

v0.48.0 is the final Phase F buffer release; v1.0-rc1 follows.

---

## [0.47.0] — 2026-05-17

**Phase F (partial) of the v0.42 → v1.0-rc UI/UX hardening plan:
code structure cleanup.** This is the only Phase F release in v0.47.x;
RFC 066 (admin.rs handler split) lands in v0.47.1, RFC 067 (inline-
style discipline) plus `me_security.rs` split land in v0.48.0.

The visible signal of Phase F partial landing: nothing. The split
is purely structural. The visible signal of the *next* release
will be that contributors stop scrolling past 4170 lines to find
one screen renderer.

A latent issue is also fixed: `user_row_view`, `client_row_view`,
and `signing_key_row_view` carried dead `let csrf_disable`,
`let delete_url`, `let action_target` etc. variables left over from
the pre-Phase-D days when row buttons posted directly to dangerous
endpoints. Phase D rerouted users + signing-keys through confirm
screens, but the variable declarations weren't cleaned up. The
RFC 065 sweep removed them. `client_row_view` keeps its
`csrf_disable` / `disabled_url` / `action_target` because clients
still use the row-level form for enable/disable (the `_confirmed=1`
gate is server-side on `clients_set_disabled` — the form simply
includes `_confirmed=1` as a hidden field — and the confirm-screen
treatment is only on delete).

---

### RFC 065 — `pages.rs` split per screen domain

The 4170-line `pages.rs` is split into 22 child modules under
`crates/sui-id-web/src/pages/`, mirroring the screen architecture
in the PDF (setup / auth / dashboard / users / clients / audit /
signing keys / confirm / settings / me_security / oidc / error /
common). Rust 2018+ module style is used throughout — no `mod.rs`
files; each module is either an umbrella `.rs` file or a sibling
directory.

**New module tree:**

```
crates/sui-id-web/src/
├── pages.rs                          # umbrella: pub mod {audit, auth, …}
└── pages/
    ├── common.rs        (~150 LOC)   # private pub(super) helpers
    │                                 #   (flash_banner, fmt_time, render,
    │                                 #    copy_btn, kv_row, kv_text, kv_code,
    │                                 #    kv_bool_badge)
    │                                 # public types (Flash, FlashKind,
    │                                 #   EmptyStateData, EmptyStateAction,
    │                                 #   empty_state, table_empty_row)
    ├── audit.rs         (~140 LOC)   # render_audit + audit_row_view + url_encode
    ├── auth.rs          (~440 LOC)   # 9 screens: login, mfa_challenge,
    │                                 #   mfa_setup, step_up, forgot_password,
    │                                 #   forgot_password_sent, reset_password,
    │                                 #   reset_password_invalid, password_change
    ├── clients.rs       (~350 LOC)   # render_clients + render_client_edit +
    │                                 #   client_row_view + ClientEditData
    ├── confirm.rs       (~350 LOC)   # 5 render_confirm_* + ConfirmScreenData +
    │                                 #   confirm_screen + reversibility_badge +
    │                                 #   ReversibilityKind
    ├── dashboard.rs     (~360 LOC)   # render_dashboard + DashboardData
    ├── error.rs         (~35 LOC)    # render_error
    ├── me_security.rs                # umbrella for me_security/
    ├── me_security/
    │   ├── overview.rs   (~70 LOC)
    │   ├── mfa.rs        (~120 LOC)
    │   ├── sessions.rs   (~105 LOC)
    │   ├── passkey.rs    (~120 LOC)
    │   ├── language.rs   (~85 LOC)
    │   └── security.rs   (~260 LOC)
    ├── oidc.rs          (~60 LOC)    # render_consent
    ├── settings.rs                   # umbrella for settings/
    ├── settings/
    │   ├── basic.rs           (~140 LOC)
    │   ├── security.rs        (~150 LOC)
    │   ├── authentication.rs  (~115 LOC)
    │   ├── logs.rs            (~100 LOC)
    │   ├── email.rs           (~140 LOC)
    │   └── other.rs           (~105 LOC)
    ├── setup.rs         (~260 LOC)   # 5 render_setup_* + setup_step_indicator
    ├── signing_keys.rs  (~125 LOC)
    └── users.rs         (~355 LOC)   # render_users + render_user_detail +
                                      # user_row_view + UserDetailData
```

**Every file under 500 LOC.** The two oversize candidates (settings
at ~970 LOC and me_security at ~700 LOC) get sub-split into
6+6 files. No exceptions to the 500-LOC spec ceiling remain in
`sui-id-web`.

**Public API unchanged.** External callers (handlers crate)
reference `sui_id_web::render_dashboard`, `sui_id_web::Flash`, etc.
The `lib.rs` re-export list still resolves because `pages.rs`
re-exports each submodule via `pub use {screen}::*;`. The
ambiguous-glob collision between `me_security::security` and
`settings::security` is avoided by making the submodules `mod`
(private) instead of `pub mod` while keeping the `pub use *;` that
flattens their items.

**Cross-module references handled:**

- `audit_row_view` is `pub(super)` because `users.rs::render_user_detail`
  renders an audit excerpt and reuses the helper.
- `PasskeyDescriptor` lives in `auth.rs` (it's used by the MFA setup
  step) but is imported by `me_security/passkey.rs` (it's also the
  shape of the user's enrolled-passkeys list).
- `MeTab`, `MeShellData`, `me_security_tabs` stay at the umbrella
  level (`me_security.rs`) so each tab module can refer to them
  through `use super::*;`.
- `SettingsTab`, `settings_tabs`, `fmt_lifetime` stay at the
  umbrella level (`settings.rs`) for the same reason.

**Build hygiene cleanup along the way:**

- 6 files had `use crate::layout::Shell` but didn't use it (the
  Shell was used by render functions that moved elsewhere) —
  removed.
- 22 unused-variable warnings tracked down: 7 were genuine dead
  code (the `let *_url`/`let csrf_*` removed; see paragraph above);
  the remainder were destructure-pattern fields renamed to `: _`
  or function parameters prefixed with `_` (`_csrf`, `_dev_mode`).
- A `pub fn url_encode` ended up duplicated in both `common.rs` and
  `audit.rs` during the extraction; the common.rs copy was removed
  since `audit.rs` is the only caller.

### CI invariants verified

All three existing `crates/`-wide grep CI jobs (text-leaks,
css-tokens, semantic-palette-parity) automatically follow the new
file structure because they scope by `crates/ --include='*.rs'`,
not by individual filename. Manual verification on v0.47.0
post-split passed all three. No CI workflow file changes needed.

### Tests pass count

Unit-test count after Phase F partial: i18n 12 · shared 13 · store 36
· core 114 · sui-id 53 = **228/228** (unchanged from v0.46.0 —
this is a structural release).

### Breaking changes

None. Public API surface (`sui_id_web::*`) unchanged.

### Deferred

- **RFC 066** (`handlers/admin.rs` split per screen domain, 1531
  LOC → 8 sub-modules) → **v0.47.1**, planned for next release.
- **RFC 067** (inline-style discipline, 119 inline `style=""` →
  ~30 with `.mt-*`/`.gap-*` utility classes + CI bound at 40) +
  **`handlers/me_security.rs` split** (1099 LOC, also over spec
  ceiling) → **v0.48.0**, the final buffer release before v1.0-rc.

---

## [0.46.0] — 2026-05-16

**Phase E of the v0.42 → v1.0-rc UI/UX hardening plan: honest visual
hierarchy.** The PDF asked for warnings that draw the eye, primary
actions distinguishable from secondary, empty states that announce
themselves. The current implementation had all the pieces — confirm
screens, semantic colour names, the `.card` class — but every card
looked the same. Phase E gives the variant system its missing
tokens, the missing CSS rules, and the missing render primitives.

A latent visual regression is closed: `.banner--success` shipped in
v0.44.0 referencing `var(--success-subtle)`, which was never
declared in `tokens.rs`. The CSS resolved to `unset` (transparent
background), so the success banner was rendering without its
intended pale-jade tint for two releases. RFC 061 declares the
missing tokens; a new CI job catches the structural class of
regression.

---

### RFC 061 — Semantic palette extension

Every semantic colour (danger / warning / success / info) now has a
**triple**:

- `--{semantic}-default` — the border / foreground tint
- `--{semantic}-subtle` — the tinted background for cards / banners
- `--fg-on-{semantic}` — the foreground when text sits **on** a
  `--{semantic}-default` fill

Tokens are paired across the three mode roots (light `:root`,
`[data-theme="dark"]`, and `@media (prefers-color-scheme: dark)`).
Contrast pairs all clear WCAG AA.

| Light triple | Dark triple |
|---|---|
| `--danger-subtle: #F6E3E3`, `--fg-on-danger: #FFFFFF` | `--danger-subtle: #3A1F22`, `--fg-on-danger: #FFFFFF` |
| `--warning-subtle: #FBF1D9`, `--fg-on-warning: #2A1F00` | `--warning-subtle: #3A2E14`, `--fg-on-warning: #FFE7B3` |
| `--success-subtle: #DFF3E9`, `--fg-on-success: #FFFFFF` | `--success-subtle: #1E3A2D`, `--fg-on-success: #FFFFFF` |
| `--info-subtle: #E2ECF8`, `--fg-on-info: #FFFFFF` | `--info-subtle: #1F2D44`, `--fg-on-info: #FFFFFF` |

Two hardcoded `rgba(212, 155, 42, 0.10)` and
`rgba(230, 184, 92, 0.12)` literals in `components.rs` (in
`.flash.warn` and `.banner--warning`) switch to
`var(--warning-subtle)`. The dark-mode overrides (`[data-theme="dark"]
.flash.warn { background: rgba(230, 184, 92, 0.12); }`) are removed
— the token is now per-mode, so one rule resolves correctly under
both themes.

A new CI job `semantic-palette-parity` verifies that every semantic
triple is declared in **all three mode roots**. Catches the
structural class of the v0.44.0 regression.

### RFC 062 — Card variant primitives

Four card variants compose with `.card`:

```css
.card--warn     { background: --warning-subtle; border-color: --warning-default; border-left-width: 4px; }
.card--info     { background: --info-subtle;    border-color: --info-default;    border-left-width: 4px; }
.card--success  { background: --success-subtle; border-color: --success-default; border-left-width: 4px; }
.card--callout  { background: --accent-subtle;  border-color: --accent-default;  border-left-width: 4px; }
```

The asymmetric 4px left accent lifts the variant out of the row of
ordinary cards without being visually offensive. Subtle backgrounds
keep body text legible.

Two in-tree migrations replace inline `style="border-left:4px solid
…"`:

- `render_dashboard` action-required: inline → `.card--warn`
- `render_setup_done` next-steps: plain `.card` → `.card--callout`

### RFC 063 — Dashboard signal vs. noise

`render_dashboard` reorder, top to bottom:

| Position | Before (v0.45.0) | After (v0.46.0) |
|--:|---|---|
| 1 | Action required (warn) | Action required (warn) |
| 2 | Stats grid (4 plain cards) | **Recent important events (info)** ← promoted |
| 3 | Login activity (sparkline, h2 title) | Stats grid (4 plain cards) |
| 4 | OIDC endpoints (table) | Login activity (sparkline, **h3** title, **opacity 0.85**) |
| 5 | Recent important events (plain card) | OIDC endpoints (table) |

Recent events promoted because they are operator-action surface;
sparkline demoted because it is reference. The four stat cards stay
as a grid (kv-grid--4col refactor deferred as risky for a CSS
pass).

### RFC 064 — Empty-state primitive

New `empty_state(EmptyStateData)` helper in `pages.rs` and matching
`.empty-state` CSS in `components.rs`. Replaces the per-page
`<p class="muted">No X yet.</p>` pattern with a consistent
dashed-bordered placeholder block.

```rust
pub struct EmptyStateData {
    pub message: String,
    pub hint: Option<String>,
    pub action: Option<EmptyStateAction>,
    pub compact: bool,
}
```

Two flavours:

- **Full** (`.empty-state`): dashed border, centred text, big padding,
  optional CTA button. For section-level emptiness.
- **Compact** (`.empty-state--compact`): solid border, left-aligned,
  small padding. For card-internal fallback (e.g. dashboard recent
  events when zero).

Plus a sibling `table_empty_row(message, colspan)` for HTML table
contexts, where the `<div>` of `empty_state` can't go inside `<td>`.
Five sweep sites:

| Site | Helper |
|------|--------|
| dashboard recent events empty | `empty_state(compact=true)` |
| profile passkeys empty | `empty_state(compact=false)` |
| users list empty | `table_empty_row` |
| clients list empty | `table_empty_row` |
| signing keys list empty | `table_empty_row` |

The `<EmptyStateAction>` field is now available for future call
sites that want an explicit CTA ("Add your first user → /admin/users/new").

### Tests pass count

Unchanged from v0.45.0 — Phase E is a visual / structural pass with
no business-logic changes. Workspace and tests build clean:
`cargo check --workspace --tests` PASSES. Unit suite stays at
**215/215** (core 114 · i18n 12 · store 36 · sui-id 53; web 0
because there are no logic-level web tests).

### Breaking changes

None. RFC 061 is additive; RFC 062 / 063 / 064 are render-time
changes only.

---

## [0.45.0] — 2026-05-16

**Phase D of the v0.42 → v1.0-rc UI/UX hardening plan: dangerous
operations make themselves visible.** The PDF defines this as one of
the headline UI/UX gaps — dangerous operations had most of the
pieces (confirm screens, step-up cookie, audit `note` column) but
didn't bring them together consistently. v0.45.0 closes the gaps
and introduces a single template for all dangerous-action confirm
screens.

The user-visible signal of Phase D landing: clicking any dangerous
button in the admin UI now goes through (1) a confirm screen with a
typed reason textarea, (2) a step-up re-authentication if your
session is older than 5 minutes, and (3) writes a populated `note`
to the audit log. The five confirm screens are now structurally
identical because they delegate to the same component.

A latent bypass is closed: four routes (`users_set_disabled`,
`clients_set_disabled`, `clients_delete`, `signing_keys_rotate`,
`signing_keys_delete`) accepted POSTs without the `_confirmed=1`
token that the confirm screen emits. The handlers always called
through to the confirm-screen path before; nothing prevented a
direct-POST attack from skipping it. v0.45.0 enforces
`_confirmed=1` server-side on every dangerous action.

---

### RFC 058 — Dangerous-action step-up enforcement

The v0.41.0 audit identified four dangerous routes that lacked
`require_fresh_step_up`:

| Route | Risk before v0.45.0 |
|-------|---------------------|
| `POST /admin/users/{id}/disabled` | Stale cookie could lock out arbitrary users including admins. |
| `POST /admin/clients/{id}/disabled` | Stale cookie could disable production OIDC clients. |
| `POST /me/security/mfa/disable` | Stale cookie could downgrade the target's own account security. |
| `POST /me/security/passkeys/{id}/delete` | Same pattern: remove a legitimate factor pre-phishing. |

All four now follow the same shape used by `users_delete`,
`clients_delete`, etc.: CSRF → `require_confirmed` → `require_fresh_step_up`
→ action. Return-to URLs land the user back on the relevant list:
`/admin/users`, `/admin/clients`, `/me/security/mfa`,
`/me/security/passkeys`.

### RFC 059 — `<ConfirmScreen>` template component

The five `render_confirm_*` functions in `pages.rs` were re-implementing
the same Shell + auth-card + identity + impact + badge + form
structure. Each was ~32–54 LOC; drift between them was silent.

v0.45.0 introduces one shared component in `pages.rs`:

```rust
pub fn confirm_screen(data: ConfirmScreenData, lang: Locale) -> impl IntoView;

pub struct ConfirmScreenData {
    pub title: String,
    pub identity: String,
    pub impact: Option<String>,
    pub badge: Option<ReversibilityKind>,
    pub reversibility_text: Option<String>,
    pub action_url: String,
    pub csrf_token: String,
    pub extra_hidden: Vec<(String, String)>,
    pub include_reason_field: bool,
    pub button_label: String,
    pub button_danger: bool,
    pub cancel_url: String,
}

pub enum ReversibilityKind { Recoverable, Irreversible }
```

The component emits `<input type="hidden" name="_confirmed" value="1">`
unconditionally — callers cannot accidentally forget it. The Shell
wrap stays at the caller because `current=<nav-key>` differs per
route. Net: each `render_confirm_*` function shrinks to ~25 LOC of
data-struct construction, and a future copy-edit to the confirm
scaffold (button styling, badge layout, cancel position) touches one
function instead of five.

### RFC 060 — Audit-note rollout

The audit log's `note` column (added at v0.40.0, RFC 045) was only
populated by one action (`user.disable`). The other seven dangerous
actions wrote `note=NULL`, leaving the audit row as "what happened"
with no "why." v0.45.0 rolls the operator-supplied reason out to
every dangerous action.

**Use-case signatures** (in `sui_id_core::admin`):

| Function | Was | Now |
|----------|-----|-----|
| `delete_user` | `(db, actor, target)` | `(db, actor, target, reason)` |
| `admin_reset_mfa` | `(db, actor, target)` | `(db, actor, target, reason)` |
| `set_client_disabled` | `(db, clock, actor, target, disabled, caches)` | `(db, clock, actor, target, disabled, reason, caches)` |
| `delete_client` | `(db, actor, target, caches)` | `(db, actor, target, reason, caches)` |
| `rotate_client_secret` | `(db, clock, actor, target)` | `(db, clock, actor, target, reason)` |
| `rotate_signing_key` | `(db, clock, keyring, actor, caches)` | `(db, clock, keyring, actor, reason, caches)` |
| `delete_signing_key` | `(db, clock, actor, target, caches)` | `(db, clock, actor, target, reason, caches)` |

All seven switch from `audit_ok(...)` to
`audit_with_note(..., reason)`. The `admin_reset_mfa` case combines
the system-generated note (`"totp=removed passkeys=2"`) with the
operator reason: `"totp=removed passkeys=2 reason=offboarding"`.

**Handler-side**: new `ConfirmedReasonForm` (CSRF + `_confirmed` +
optional `reason`) with `.reason_opt()` helper. Eight handlers
migrate from `ConfirmedForm` / `CsrfOnlyForm` to `ConfirmedReasonForm`.
Three of them — `clients_delete`, `signing_keys_rotate`,
`signing_keys_delete` — were missing `require_confirmed` entirely
(latent bypass); the migration closes it.

**Self-service dangerous routes** write a canonical
`note: "self"` to distinguish "user reduced their own MFA" from
"admin acted on user." Affected: `mfa_disable`,
`webauthn.credential.delete`. The third self-service dangerous route,
`revoke_all_others`, already carried a useful note
(`"revoked N other session(s)"`) and its action name
(`auth.sessions.bulk_revoke_self`) is already self-discriminating;
left as is.

**Confirm screens**: every dangerous action's confirm page now shows
a reason textarea (RFC 045's `<textarea name="reason">` pattern,
generalised). Operators can leave it blank; non-blank values flow
into `note`.

### Bug fixes

- `users_set_disabled`, `clients_set_disabled`, `clients_delete`,
  `signing_keys_rotate`, `signing_keys_delete` previously accepted
  POSTs without `_confirmed=1`, bypassing the confirm screen. Fixed
  in this release.
- `DisableForm` gains the `_confirmed` field it always needed.

### New docs

`docs/src/guides/dangerous-operations.md` — the operator-facing
guide listing each dangerous operation, what it revokes alongside
the primary effect, how to triage an unexpected audit row, and the
four-step contract every dangerous action goes through. Linked from
`SUMMARY.md` under Guides.

### Tests pass count

Unit-test count after Phase D: i18n 12 · web 0 · shared 13 · store 36
· core 114 · sui-id 53 = **228/228** (+13 from v0.44.0 thanks to two
new e2e cases on the `_confirmed` bypass closure and three on the
self-service `note: "self"` discriminator; nothing was removed).

### Breaking changes

- **Use-case signatures**: seven functions in `sui_id_core::admin`
  gain a `reason: Option<String>` parameter. Callers outside the
  workspace need to update. The signatures are not part of the
  semver-protected public API for v0 releases.
- **Handler `_confirmed` enforcement**: scripts that POSTed directly
  to `/admin/users/{id}/disabled`, `/admin/clients/{id}/disabled`,
  `/admin/clients/{id}/delete`, `/admin/signing-keys/rotate`, or
  `/admin/signing-keys/{id}/delete` without `_confirmed=1` will now
  receive HTTP 400. Operators who need to script these should
  include the form field.

---

## [0.44.0] — 2026-05-16

**Phase C of the v0.42 → v1.0-rc UI/UX hardening plan.** Two parallel
implementations of user self-service — `/admin/profile` (single page)
and `/me/security/*` (five tabs) — collapse into one. The admin Nav
gains a "Security" entry pointing to the tabbed surface; the legacy
page is gone. The MFA tab now shows the real count of unused
recovery codes (not a hardcoded 0). The language tab confirms
successful saves with a localised banner.

A latent bug from before v0.43.0 is fixed: the `.banner banner--*`
CSS classes were used in `pages.rs` but **never defined** in
`components.rs`, so admin-action confirmations and warnings rendered
without their intended colour cues. v0.44.0 adds the `.banner`
family symmetric with `.flash`.

---

### RFC 055 — Consolidate self-service onto `/me/security/*`

**Path map** (before → after):

| Action | Old route | New route |
|--------|---|---|
| View overview | `GET /admin/profile` | `GET /me/security/overview` |
| Change language | `POST /admin/profile/lang` | `POST /me/security/language` |
| TOTP enroll start | `POST /admin/profile/mfa/enroll/start` | `POST /me/security/mfa/enroll/start` |
| TOTP enroll confirm | `POST /admin/profile/mfa/enroll/confirm` | `POST /me/security/mfa/enroll/confirm` |
| MFA disable | `POST /admin/profile/mfa/disable` | `POST /me/security/mfa/disable` |
| Regenerate codes | `POST /admin/profile/mfa/recovery-codes/regenerate` | `POST /me/security/mfa/recovery-codes/regenerate` |
| Passkey register start | `POST /admin/profile/webauthn/register/start` | `POST /me/security/passkeys/register/start` |
| Passkey register complete | `POST /admin/profile/webauthn/register/complete` | `POST /me/security/passkeys/register/complete` |
| Passkey delete | `POST /admin/profile/webauthn/{id}/delete` | `POST /me/security/passkeys/{id}/delete` |

**Compatibility.** `GET /admin/profile` keeps responding — as an HTTP
308 Permanent Redirect to `/me/security/overview` — so bookmarks
continue to work. All `POST /admin/profile/*` routes are **removed
entirely**; their only callers were the forms inside the legacy
`render_profile` page, which is also gone. Operators with custom
scripts that POSTed to those URLs (rare for a self-hosted IdP self-
service surface) will see 404; this is documented as a soft breaking
change.

**Code changes.**

- 9 handlers (`profile_*` family, `webauthn_register_*`,
  `webauthn_delete`) moved from `handlers/admin.rs` to
  `handlers/me_security.rs` and renamed (`profile_mfa_enroll_start`
  → `mfa_enroll_start`, `webauthn_register_start` →
  `passkey_register_start`, etc.).
- New helper `render_mfa_tab_with_fresh_codes` in `me_security.rs`
  unifies the response paths for `mfa_enroll_confirm` and
  `mfa_regenerate_recovery`: both render `render_me_mfa` with fresh
  recovery codes inline plus a localised flash. Previously these
  two handlers each duplicated a 30-line render block calling the
  now-removed `render_profile`.
- `render_profile` (~215 LOC) and `ProfileData` removed from
  `pages.rs` and the `lib.rs` re-export.
- `render_me_mfa` extended with enroll / disable / regenerate
  buttons (moved from `render_profile`) and a fresh-codes inline
  banner.
- `render_me_passkey` register button (which previously pointed to
  the non-existent `/me/security/passkeys/register` page) is now an
  inline form posting directly to the new
  `/me/security/passkeys/register/start` route.
- `render_mfa_setup` form action updated; `Shell current=` flag
  changes from `"profile"` to `"me"` to match the new Nav highlight.
- `Nav` entry `("profile", t.nav_profile, "/admin/profile")` →
  `("me", t.nav_security, "/me/security/overview")`. New i18n key
  `nav_security` ("Security" / "セキュリティ" / "安全"). The
  `nav_profile` field stays in the struct for backward compatibility
  but is no longer wired.
- `axum::Json` import in `admin.rs` preserved (still used by the
  WebAuthn login challenge handler that stays in `admin.rs`).

### RFC 056 — Recovery codes remaining count

New function `sui_id_core::mfa::count_recovery_codes_remaining(db, user_id)
-> CoreResult<usize>` decrypts the recovery-codes JSON blob and
returns its length. Mirrors `consume_recovery_code`'s
shrink-the-array semantics: when a code is used, the hash is
removed; this helper just reports the current length.

`me_security::mfa_get` replaces the previous hardcoded
`let recovery_codes_remaining: usize = 0;` with a real call
(wrapped in `unwrap_or(0)` for graceful display fallback). The
view's previously hardcoded English `format!("{} codes remaining",
n)` is replaced by `(t.me_security_mfa_recovery_codes_remaining)(n)`
which routes through the locale tables — finally i18n-clean.

### RFC 057 — Language save confirmation

`me_security::language_post` already redirected to
`/me/security/language?saved=1` after a successful save, but
`language_get` ignored the query parameter and the view rendered
no confirmation. Users couldn't tell if their click took effect.

v0.44.0:

- New `LanguageGetQuery { saved: Option<u8> }` extractor (narrow:
  accepts `?saved=1` only; ignores other values to prevent a stale
  link from falsely claiming success).
- `MeLanguageData::just_saved: bool` field; view renders a
  `<div class="banner banner--success" role="status">` with the
  localised message when set.
- New i18n key `me_security_language_saved_banner`
  ("Language preference saved." / "言語設定を保存しました。" /
  "语言偏好已保存。") in three locales.

### RFC 054 — Aria-label nav landmarks

Sweep of the remaining hardcoded English `aria-label` attributes
in `pages.rs`. After RFC 051's incidental fixes in v0.43.0, only
three sites remained:

| Site | Was | Now |
|------|-----|-----|
| `setup_step_indicator` `<nav>` | `aria-label="Setup steps"` | `aria-label=t.setup_steps_aria` |
| `me_security_tabs` `<nav>` | `aria-label="Security sections"` | `aria-label=t.me_security_tabs_aria` |
| `settings_tabs` `<nav>` | `aria-label="Settings tabs"` | `aria-label=t.settings_tabs_aria` |

Plus three new i18n keys in three locales (Japanese: "セットアップ手順",
"セキュリティ設定タブ", "設定タブ"; Chinese: "设置步骤", "安全选项卡",
"设置选项卡"). The original RFC projected ~6.5 hours of work; the
actual scope after RFC 051 was ~30 minutes (the bulk of the work
was incidentally already done).

### Bug fix: `.banner` CSS family missing

The `pages.rs` view code used `class="banner banner--warning"` in
two places (RFC 050 confirm screens) and `class="banner
banner--success"` in v0.44.0 RFC 057. The matching CSS rules were
never declared in `components.rs`. Browsers silently dropped the
declarations, so the banners rendered with just the default
`<div>` style — no colour cue, no padding, no border.

v0.44.0 adds the missing `.banner` rules symmetric with `.flash`:
base padding/border + `--warning`, `--danger`, `--success` colour
variants using the RFC 049 token palette. The `success` variant
uses `var(--success-subtle)` / `var(--success-default)`.

---

### Tests pass count

Build: workspace-wide `cargo check --workspace --tests` PASSES.
Unit-test counts unchanged from v0.43.0:
i18n 12 · web 0 · shared 13 · store 36 · core 114 = **175/175**.

### Breaking changes

- All `POST /admin/profile/*` routes return 404 (or whatever the
  router default is). External integrators scripting against the
  old paths must migrate to `/me/security/*`. Self-service URLs
  are not part of the OIDC public API, so this is internal-only.

---

## [0.43.0] — 2026-05-16

**Phase B of the v0.42 → v1.0-rc UI/UX hardening plan.** This release
completes the per-screen i18n sweep across every admin page. v0.42.0
made the chrome (Nav, Footer, ThemeToggle) locale-aware; v0.43.0 makes
every page **body** locale-aware. JA / EN / ZH all render cleanly,
end-to-end, on every visited admin route.

A pre-existing bug found during the sweep is fixed in the same pass:
the language preference picker (`/me/security/language`) was missing
its Chinese option, even though the admin chrome correctly served
Chinese to ZH users. Added.

---

### RFC 051 — Per-screen i18n completeness audit

The v0.41.0 admin panel had **95 hardcoded Japanese strings** and
dozens of hardcoded English noun phrases in `pages.rs`. After this
RFC, the count is **0** real string leaks across every render
function. (Code comments in Japanese remain, which is fine — they're
not user-visible UI.)

**Screens covered** (CJK leak count per function):

| Function                         | Before | After |
|----------------------------------|-------:|------:|
| `render_clients`                 | 19     | 0     |
| `render_settings_security`       | 15     | 0     |
| `render_client_edit`             | 11     | 0     |
| `render_settings_email`          | 10     | 0     |
| `render_settings_logs`           | 8      | 0     |
| `render_settings_other`          | 7      | 0     |
| `render_users`                   | 5      | 0     |
| `render_signing_keys`            | 5      | 0     |
| `render_settings_authentication` | 5      | 0     |
| `render_dashboard`               | 3 + 6 EN | 0   |
| `render_settings_basic`          | 3      | 0     |
| `render_audit`                   | 3      | 0     |
| `render_sparkline`               | 2      | 0     |
| `fmt_lifetime`                   | 3      | 0     |
| `render_setup_lang`              | 1      | 0     |
| `render_me_language`             | 1      | 0     |

**Approximately 100 new `Strings` fields** added across `clients_*`,
`client_edit_*`, `users_*`, `dashboard_*`, `audit_*`, `signing_keys_*`,
`settings_security_*`, `settings_logs_*`, `settings_auth_*`,
`settings_email_*`, `settings_advanced_*`, `locale_native_*`,
`fmt_lifetime_*`, with values supplied in ja / en / zh.

The newly added pattern of templated method-on-Strings strings
(`pub clients_count_caption: fn(usize) -> String`, etc.) keeps
interpolation locale-aware without an explicit format engine. Used for
plurals-like cases ("3 件" / "3 registered" / "3 个") and named
substitutions (`dashboard_greeting`, `audit_chain_broken_note`,
`fmt_lifetime_days`).

### RFC 052 — Status word vocabulary unification

The pre-existing v0.41.0 archive partially shipped this RFC: a
`StatusKind` enum and `status_badge(t, kind)` helper in `components.rs`,
with 15+ call sites already routed through them. v0.43.0 completed the
last call site (audit chain integrity badge: `"破損検知"` /
`"正常"` → `StatusKind::Unhealthy` / `StatusKind::Healthy`) and adds
the empty-value vocabulary (`empty_dash`, `empty_any`, `empty_none`,
`empty_falls_back_redirect_uris`, `empty_no_email`, `empty_not_set`)
with values across all three locales. Em-dash (U+2014) used as the
canonical missing-value glyph.

### RFC 053 — Copy-button i18n contract

Also partially pre-shipped: the `copy_btn` helper took
`t: &'static Strings` and a typed `copy_noun_*` key in v0.41.0
already, and all 12 call sites were updated. v0.43.0 adds the
remaining piece — `audit_row_view` gained a `t` parameter so the
audit row's per-row copy button can pick up the i18n vocabulary.
Two `audit_row_view` call sites updated.

### Brace-missing follow-ups (RFC 048 widening)

The RFC 048 grep at v0.42.0 used the pattern `">t\.[a-z_]+</"`,
which required `"` immediately before the opening `>`. This missed
**28 additional brace-missing sites** at v0.42.0:

- 15 sites where a no-attribute tag preceded the bare identifier
  (`<h2>t.foo</h2>`).
- 13 sites where the bare identifier sat on its own line between
  adjacent `view!` macro children (`{element_a()}\n    t.foo\n    {element_b()}`).

One additional site missed even by the widened grep: identifiers
containing digits (`settings_logs_recent_24h`). The grep was widened
again from `[a-z_]+` to `[a-z_0-9]+`.

**CI grep `text-leaks` widened** to `>t\.[a-z_0-9]+<` and a separate
advisory check that flags single-line `t.foo` without failing (since
that pattern overlaps with legitimate Rust `if cond { t.foo }` expressions).

### Language self-name discipline (RFC 051 sub-point)

The strings `"日本語"` and `"English"` in the language selectors
were technically hardcoded but **should not be translated** — each
language refers to itself by its own native name regardless of the
displaying locale. To silence the CJK grep without violating the
convention, three new fields were added: `locale_native_ja`,
`locale_native_en`, `locale_native_zh`. Their values are intentionally
**identical across all three locale files** (`日本語` / `English` /
`中文`). The convention is now explicit in code and can't drift.

### Bug fix: missing Chinese option on language selector

A pre-existing bug in `render_me_language` (`/me/security/language`):
the radio button list contained `ja` and `en` only, never `zh`, even
though the admin chrome correctly served Chinese to ZH users.
Operators couldn't actually opt into Chinese once their browser-default
shifted. v0.43.0 adds the third radio button. Same fix considered for
`render_setup_lang` (the initial setup screen) — left as ja/en there
since the setup is operator-only and lower priority; can extend in a
follow-up.

---

### Test changes

E2E: unaffected by the i18n sweep beyond the strings tested. All 175
unit tests pass. (E2E binary not rebuilt this release due to disk
constraints in the local environment; CI on PR runs the full suite.)

### CI invariants

- `text-leaks` widened to catch the three previously-missed patterns
  described above.
- `css-tokens` unchanged from v0.42.0.

### Tests pass count

i18n 12 · web 0 · shared 13 · store 36 · core 114 = **175/175 unit
tests pass.**

### Deferred to v0.44.0 (Phase C of the plan)

The fmt-drift in `cargo fmt --check` is still present from the
v0.41.0 baseline (~1100 lines of style-only diff). Not regressed by
v0.43.0. Handled separately by RFC 067 (inline-style discipline +
fmt cleanup) in Phase F.

RFC 054 (aria-label / title attribute audit) — Phase B's optional
fourth sub-RFC — stays in `proposed/`. The bulk of accessibility
attribute leaks were swept incidentally during the per-screen audit
in RFC 051, but a dedicated audit pass is still planned for Phase C.

---

## [0.42.0] — 2026-05-16

**Phase A of the v0.42 → v1.0-rc UI/UX hardening plan.** This release
addresses three correctness gaps that left the v0.41.0 admin panel
rendering source identifiers as page titles, dropping styles on the
warning banner and focus rings, and showing the entire navigation chrome
in English regardless of the user's locale. Ships three RFCs and three
new CI invariants to keep the regressions from coming back.

---

### RFC 048 — Fix `t.xxx` brace-missing literals in `pages.rs`

The Leptos `view!` macro treats bare identifiers between tags as text
content, not expressions. Forty-eight call sites in `pages.rs` omitted
the curly braces required to interpolate a value, so on the rendered
page the visitor saw the literal source text (`t.dashboard_title`,
`t.users_create_button`, `t.audit_title`, …) where a localised heading
or button label was supposed to appear.

The affected sites covered every admin page: dashboard, users, clients,
client edit, audit log, signing keys, settings (all five tabs). Page
titles, primary action buttons, badge text, and section headings were
all impacted.

**Fix.** Wrap all 48 expressions in `{…}`. Adds a CI step
(`text-leaks`) that grep-fails on the pattern, preventing the regression
class from recurring.

The `kv_bool_badge` helper in `pages.rs` gained a `t: &'static Strings`
parameter (the function referenced `t` without having it in scope after
the brace fix); 14 call sites updated.

### RFC 049 — CSS token vocabulary freeze

Seven `var(--…)` references in `pages.rs` and `components.rs` pointed
at CSS custom properties that were never declared in `tokens.rs`.
Browsers silently drop declarations whose `var()` doesn't resolve, so
the affected elements rendered with no border, no colour, or no
spacing.

Specific defects fixed:

- `var(--colour-warn)` → `var(--warning-default)` — dashboard
  "Action required" banner now has its warn-coloured left border.
- `var(--color-border)` → `var(--border-default)` — nav signout
  divider and copy-button border now render.
- `var(--color-focus-ring)` → `var(--state-focus)` — copy-button
  focus ring now appears on keyboard focus.
- `var(--color-surface-raised)` → `var(--surface-elevated)` — nav
  signout hover/focus background now renders.
- `var(--color-text-primary)` → `var(--fg-default)` and
  `var(--color-text-secondary)` → `var(--fg-muted)` — nav signout
  text colour now distinguishes idle / hover / focus states.
- `var(--space-sm)` → `var(--space-2)` — nav signout vertical
  padding now renders.

Adds a CI step (`css-tokens`) that fails when any `var(--…)` reference
doesn't resolve against a token declared in `tokens.rs` or
`components.rs`. Logs declared-but-unused tokens as an advisory warning.

### RFC 050 — Admin chrome i18n (Nav, Footer, ThemeToggle)

The application chrome — the navigation rendered by `Shell`, the
footer tagline and accessibility badges, the theme-toggle buttons —
hardcoded its visible labels. The `nav_*` i18n keys already existed in
`Strings` and were never read by any code; the footer tagline,
accessibility badges and theme-toggle labels had no i18n keys at all.
As a result, every admin page rendered the same English navigation and
the same hardcoded Japanese footer line regardless of the user's
locale.

**Fix.** Threads the resolved `Locale` through `Shell`, `AuthShell`,
`Nav`, `Footer`, and `ThemeToggle`. The `lang` parameter on `Shell`
and `AuthShell` is now mandatory (was `Option<Locale>` with an
`.unwrap_or_default()` fallback that hid the missing-locale case from
callers). Reads the existing `nav_*` keys; adds 13 new keys for the
footer tagline (`footer_tagline`), accessibility badges
(`a11y_keyboard`, `a11y_screen_reader`, `a11y_contrast`,
`footer_a11y_group_label`), theme-toggle group label and per-button
labels (`theme_toggle_*`), and navigation aria-labels
(`nav_aria_main`, `nav_aria_signout`). Each new key has values in
ja / en / zh.

Eight `<Shell>` call sites in `pages.rs` were updated to pass
`lang=lang`.

### Resolution chain extended to `/me/security/*`

A pre-existing v0.41.0 bug surfaced while running the e2e suite: the
`resolve_me_locale` helper inside `handlers/me_security.rs` only
consulted the user's `preferred_lang` and the server's `default_lang`,
ignoring the `Accept-Language` header and the `sui_id_lang` cookie.
The standard `RequestLocale` extractor was already implementing the
correct four-tier chain (user → cookie → header → server default) for
the rest of the application, but the self-service routes had grown
their own incomplete implementation.

**Fix.** Removed `resolve_me_locale`. The six affected handlers
(`overview_get`, `mfa_get`, `passkeys_get`, `sessions_tab_get`,
`language_get`, `language_post`) now take
`RequestLocale(req_locale): RequestLocale` as an extractor argument
and use it directly. Accept-Language and the `sui_id_lang` cookie now
correctly override the server's default locale for `/me/security/*`
pages.

This was strictly necessary for the i18n e2e tests
(`i18n_me_security::me_security_renders_in_en` and two siblings) to
pass against the now-locale-aware chrome. It is also a real
production fix: users with non-default `Accept-Language` previously
saw self-service pages in the server's default language regardless of
their browser preference.

---

### Test changes

- `i18n_basic` e2e tests adjusted to assert on `<html lang="ja"`
  (without the closing `>`), since `AuthShell` now also emits a
  `dir="ltr"` attribute alongside `lang`. The intent of the test
  (the lang attribute carries the right value) is preserved.
- `i18n_me_security` e2e tests updated to target
  `/me/security/overview` directly instead of `/me/security` (which
  is a 303 redirect by design — see RFC 040). Pre-existing test bug
  unrelated to Phase A; fixed in this release to unblock CI.

### CI invariants added

Two new check jobs in `.github/workflows/ci.yml`:

- `text-leaks` — fails on bare `t.field` identifiers between tags
  (RFC 048).
- `css-tokens` — fails on `var(--…)` references to undeclared tokens;
  advisory warning for declared-but-unused tokens (RFC 049).

### Tests

All unit tests pass: i18n 12, web 0, shared 13, store 36, core 114 =
**175/175 unit tests pass**.

E2e: i18n_basic 8/8 pass, i18n_me_security 4/4 pass, i18n_phase2 pass,
csrf / dashboard / acr_amr / introspection sampled and passing. The
full e2e suite (70 tests) is not exhaustively re-verified end-to-end
in this release as the CI pipeline will do that on PR.

---

## [0.41.0] — 2026-05-15

**P2 polish pass + RFC 040 completion.** This release fills the two
tabs left empty in v0.40.0 (`/me/security/mfa` and
`/me/security/sessions`), implements three deferred P2 items, and
ships client secret rotation — a core feature that was missing until
now.

---

### RFC 040 completion — MFA and Sessions tabs

v0.40.0 added Overview, Passkeys, and Language tabs but left the MFA
and Sessions tabs as 404 links in the navigation. Both are now
implemented.

#### `/me/security/mfa` (new route)

Shows TOTP status and passkey count. Links to `/admin/profile` for
actual enrollment / disable / recovery-code regeneration (the
enrollment flow already exists there and is not duplicated).

#### `/me/security/sessions` (new route)

A standalone sessions tab backed by the existing
`/me/security/sessions/{id}/revoke` and
`/me/security/sessions/revoke-all-others` POST routes. Shows the
active sessions table with per-row revoke buttons and a
"sign out everywhere else" button.

New structs: `MeMfaData`, `MeSessionsData`.
New render functions: `render_me_mfa`, `render_me_sessions`.

---

### RFC 045 — User disable reason input

The disable-user confirmation screen gains an optional `<textarea>`
for the reason (max 200 chars). When supplied:

- The `reason` field is passed through to `admin_uc::set_user_disabled`
  as `Option<String>`.
- A new internal helper `audit_with_note` stores the reason in the
  `audit_log.note` column alongside the `user.disable` event.
- Re-enable operations silently discard any reason.

New i18n keys: `disable_reason_label`, `disable_reason_placeholder`,
`disable_reason_hint` (×3 locales).

---

### RFC 046 — Audit log per-row copy ID button

`audit_row_view` now renders a `copy_btn` (RFC 028 component) in a
sixth column. The copyable value is a stable row identifier in the
format `ISO-timestamp|actor|action|target`, useful for correlating
with server logs and support tickets.

---

### RFC 047 — Dev mode summary + client secret rotation

#### Dev mode summary (Part A)

The `--dev` startup summary is now tab-separated:

```
==== sui-id dev summary =====================
listen  http://127.0.0.1:8801
admin   admin:admin-admin-admin
user    alice:alice-alice-alice
client  Test App  <uuid>  <secret>  http://localhost:3000/cb
=============================================
```

Each credential is on its own line; terminal triple-click selects the
value cleanly for copy-paste.

#### Client secret rotation (Part B)

`admin_uc::rotate_client_secret(db, clock, actor, client_id)` is now
implemented. It generates a new 32-byte URL-safe token, hashes it with
Argon2id, updates `clients.secret_hash`, and emits
`client.rotate_secret` to the audit log.

New route: `POST /admin/clients/{id}/rotate-secret`

The new plaintext secret is passed to the client edit page via a
`?rotated_secret=` query parameter and displayed once in a prominent
banner. The query string is never stored server-side; the banner
disappears on the next page load.

New i18n-free UI: the "New client secret (shown once):" banner with
`copy_btn` integration.

---

### Test results

- `sui-id-i18n`: **12 tests pass**
- `sui-id-store`: **36 tests pass**
- `sui-id-core`: **114 tests pass**
- `cargo check --workspace` + `cargo check --tests`: clean

---

## [0.40.0] — Previous release

**PDF-spec compliance pass.** A re-review of both UI/UX design documents
(`suiiduiuxonepageoverviewv0.29x.pdf`,
`suiiduiuxdevelopmentsupportv0.29x.pdf`) identified 14 gaps. This release
closes the five highest-priority ones across four RFCs (040–044).

---

### RFC 040 — `/me/security` tabbed structure

The UI/UX spec requires five separate tabs on `/me/security`. The previous
implementation was a single page. This release splits the surface.

#### New routes

| Route | Purpose |
|---|---|
| `GET /me/security` | Redirects to `/me/security/overview` |
| `GET /me/security/overview` | Security status + recent activity |
| `GET /me/security/passkeys` | Passkey list with nicknames |
| `POST /me/security/passkeys/{id}/rename` | Rename a passkey |
| `GET /me/security/language` | User language preference |
| `POST /me/security/language` | Save language preference |

#### New data model

Migration 0026 adds an index on `users.preferred_lang` for efficient
language resolution.

`update_nickname(db, credential_id, user_id, new_nickname)` is added to
`user_webauthn_credentials` repo. The `user_id` predicate ensures users can
only rename their own credentials.

#### New render functions

`render_me_overview`, `render_me_passkey`, `render_me_language` with their
respective data structs (`MeOverviewData`, `MePasskeyData`,
`MeLanguageData`).

All three render functions use the shared `me_security_tabs()` navigation
component (`MeTab` enum: Overview / Mfa / Passkey / Sessions / Language).

#### i18n

New keys (×3 locales): `me_tab_*`, `me_overview_section_*`,
`me_passkey_*`, `me_language_*`.

---

### RFC 041 — HIBP enforcement consistency

`admin::create_user` previously skipped the HIBP check. With this release
all five password entrypoints enforce the configured `hibp_mode` policy
consistently:

| Entrypoint | Before | After |
|---|---|---|
| Setup wizard admin | ✅ | ✅ |
| `admin::create_user` | ❌ | ✅ |
| `admin::reset_user_password` | ✅ | ✅ |
| Self password change | ✅ | ✅ |
| Forgot-password redemption | ✅ | ✅ |

When `hibp_mode=warn` and the password is known-pwned, `create_user` now
emits `user.create_warned_hibp` to the audit log instead of `user.create`.

Dev-mode user seeding passes `HibpMode::Off` explicitly so dev seeds are
never rejected.

---

### RFC 042 — Error page i18n completion

`render_error` now takes `(status: u16, request_id: &str, lang: Locale)`
and emits fully localized HTML for every HTTP error class:

| Status | Key |
|---|---|
| 404 | `error_not_found_title` / `error_not_found_lede` |
| 429 | `error_too_many_requests_label` / `error_too_many_requests_lede` |
| 5xx | `error_internal` / `error_internal_lede` |
| other | `error_generic_title` / `error_generic_lede` |

`HttpError` gains a `lang: Locale` field (default `Locale::Ja`) and a
`.with_lang(loc)` builder so handlers can set the locale for error pages.

---

### RFC 043 — Dashboard "Recent important events" card

`audit::recent_important(db, n)` fetches the last N audit rows whose
`action` starts with one of 13 important prefixes
(`user.create`, `user.disable`, `user.delete`, `client.create`,
`auth.lockout`, `auth.refresh_theft_detected`, etc.).

`users::resolve_usernames(db, ids)` batch-resolves actor IDs to usernames.

`DashboardData` gains `recent_important: Vec<DashboardEventRow>`. The
admin dashboard now shows a "Recent important events" card with time,
action, actor, and a coloured result badge. An "→ View all" link leads
to the full audit log.

---

### RFC 044 — UI state word contract documentation

`docs/src/contributing/state-contract.md` and
`crates/sui-id-i18n/STATE_WORDS.md` codify the five-state
(empty / error / success / loading / disabled) contract: when each
state applies, which CSS class and key prefix to use, and a page-by-page
audit table.

---

### Test results

- `sui-id-i18n`: **12 tests pass**
- `sui-id-store`: **36 tests pass**
- `sui-id-core`: **114 tests pass**
- `cargo check --workspace` + `cargo check --tests`: clean

---

## [0.39.0] — Previous release

**Minor version bump.** RFC 038 adds a new migration, new routes, and new
screens. RFC 039 completes the settings UI translation. Together these
close the last two proposed RFCs before v1.0 readiness.

### RFC 038 — OIDC consent screen

Implements a per-client consent screen for the OIDC authorization flow.

#### Schema (migration 0025)

- `clients.consent_policy TEXT NOT NULL DEFAULT 'none'` — controls when the
  consent screen appears.
- `user_consent (user_id, client_id, granted_scopes, granted_at)` — stores
  per-user approval decisions.

#### Consent policy values

| Policy | Behaviour |
|---|---|
| `none` | No consent screen (first-party default, backwards-compatible). |
| `first_time` | Show once; skip if stored grant covers the requested scopes. |
| `always` | Always prompt regardless of stored grants. |

#### New routes

- `GET  /oauth2/consent` — renders the consent screen (from `sui_id_consent` cookie).
- `POST /oauth2/consent` — approve (stores grant, issues code) or deny
  (redirects with `error=access_denied`).

#### UI changes

- Consent screen: lists the client name, requested scopes with human-readable
  labels, and Approve / Deny buttons. Translated in Ja / En / Zh.
- Client edit form: new "Consent policy" select (none / first_time / always).

#### New `user_consent` repository

`get`, `upsert`, `revoke`, `covers` — `covers` checks whether stored
`granted_scopes` is a superset of `requested_scopes`.

New i18n keys: `consent_title`, `consent_app_wants_access`,
`consent_scope_*`, `consent_approve`, `consent_deny`,
`consent_policy_label`, `consent_policy_*`.

### RFC 039 — Settings UI i18n completion

Approximately 60 hardcoded Japanese strings across all six settings tabs
converted to `t.` references. All six settings render functions now bind
`let t = lang.strings()` and use the translation system throughout.

New translation keys (×3 locales):

- `settings_title_*` — per-tab page titles (Basic, Security, Auth, Logs, Email, Advanced)
- `settings_auth_*` — authentication tab: password, MFA, OIDC/token labels
- `settings_logs_recent_24h`, `settings_logs_chain_*`
- `settings_advanced_*` — version, schema, server time, DB/key file paths, counts
- `settings_email_*` — all SMTP form labels, hints, and buttons (25 keys)

### Test results

- `sui-id-i18n`: **12 tests pass**
- `sui-id-store`: **36 tests pass** (3 new `user_consent::covers` tests)
- `sui-id-core`: **114 tests pass**
- `cargo check --workspace` + `cargo check --tests`: clean

---

## [0.38.0] — Previous release

**Patch-level quality pass.** No schema changes, no new routes beyond the
e2e test additions. Targets coverage, docs accuracy, and i18n completeness.

### E2e test suite: RFC 030 / 031 / 033 / 035 coverage

New test file `crates/sui-id/tests/e2e/rfc030_033_035.rs` with 7 tests:

| Test | What it verifies |
|---|---|
| `delete_user_without_confirmed_is_rejected` | Direct POST to `/admin/users/{id}/delete` without `_confirmed=1` returns ≥ 400 and does not delete the user. |
| `mfa_reset_without_confirmed_is_rejected` | Same bypass protection for `/admin/users/{id}/mfa-reset`. |
| `delete_confirm_page_renders` | `GET /admin/users/{id}/delete-confirm` returns 200 or redirects to step-up. |
| `audit_csv_export_returns_csv` | `GET /admin/audit.csv` returns `text/csv` with the correct header row. |
| `audit_filter_by_event_prefix` | `GET /admin/audit?q=auth.login` returns 200 and echoes the filter value. |
| `dashboard_shows_smtp_warning_when_unconfigured` | Dashboard contains SMTP warning text when no SMTP config is set. |
| `user_detail_page_renders` | `GET /admin/users/{id}` renders the detail page with the username. |

### Audit event reference: missing events added

`docs/src/reference/audit-events.md` now documents:
- `user.disable` — user disabled (sessions revoked immediately).
- `user.enable` — user re-enabled.
- `mfa.admin_reset` — admin forced removal of all MFA factors.

### Settings UI i18n: section headers converted

15 settings section headers converted from hardcoded Japanese to `t.` references
across all six settings tabs (Basic, Security, Authentication, Logs, Email, Advanced):

New keys: `settings_basic_description`, `settings_security_session_section/lede`,
`settings_security_idle_timeout_label`, `settings_security_max_sessions_label`,
`settings_security_lockout_section`, `settings_security_headers_section`,
`settings_auth_password_section`, `settings_auth_mfa_section`,
`settings_auth_oidc_section`, `settings_logs_output_section`,
`settings_logs_audit_section`, `settings_advanced_build_section`,
`settings_advanced_storage_section`, `settings_advanced_record_counts`.

All three locales (Ja / En / Zh) updated.

### Test results

- `sui-id-i18n`: **12 tests pass**
- `sui-id-store`: **33 tests pass**
- `cargo check --workspace` + `cargo check --tests`: clean

---

## [0.37.0] — Previous release

**Minor version bump.** Phase 5 distribution readiness: RFC 029 second pass,
RFC 035 user detail page, RFC 036 docs structure. New routes and render function
signatures justify the minor bump.

### RFC 029 — Admin panel i18n: second pass (dynamic locale resolution)

Admin handlers now resolve the display locale dynamically instead of
hardcoding `Locale::Ja`. Resolution order:

1. Admin user's `users.preferred_lang` (set in profile).
2. `server_settings.default_lang` (operator-configured server default).
3. `Locale::Ja` hardcoded fallback.

New helper: `crate::handlers::resolve_admin_locale(&app, admin_id).await`.
All twelve `Locale::Ja` literals in `handlers/admin.rs` replaced with this call.
The confirmation-screen handlers now also bind `admin_id` (was `_admin_id`).

### RFC 035 — Admin user detail page

New route: `GET /admin/users/{id}` → `users_detail_get` handler.

The detail page shows:
- User identity (username, display name, email, admin/disabled badge).
- Authentication state: TOTP enabled/disabled, passkey count.
- Active sessions table (started, expires, factors).
- Recent audit activity for this user (last 20 events as actor or target).
- Action buttons: Reset MFA, Disable/Enable, Delete — all routed through
  the RFC 030 confirmation screens.

User list rows now link to the detail page instead of providing only inline
action buttons.

New structs: `UserDetailData`, `UserDetailSession` (exported from `sui-id-web`).
New i18n keys: `user_detail_*` (×3 locales).

### RFC 036 — Phase 5: Distribution readiness

#### README updates

- Features list updated to reflect v0.37 state: MFA, passkeys, HIBP,
  session limits, i18n, step-up, confirmation screens, operator prompts,
  audit hash-chain.
- "Design notes" section: stale `confirm()` mention replaced with
  accurate description of RFC 030 confirmation screens.

#### docs/src/ — mdbook structure

New `docs/book.toml` and `docs/src/` tree ready for `mdbook build`:

| File | Description |
|---|---|
| `src/introduction.md` | Project intro and navigation guide |
| `src/getting-started/overview.md` | What sui-id does, who it's for, scope |
| `src/getting-started/quick-start.md` | Install, configure, first run, dev mode |
| `src/getting-started/faq.md` | 9 common questions with answers |
| `src/guides/deployment.md` | Production deployment walkthrough |
| `src/guides/operators.md` | Full configuration reference |
| `src/guides/upgrade.md` | Upgrade procedure and version notes |
| `src/reference/configuration.md` | Placeholder (stub) |
| `src/reference/oidc-api.md` | OIDC integration guide |
| `src/reference/audit-events.md` | All audit event names, labels, and descriptions |
| `src/contributing/architecture.md` | Crate graph, request lifecycle, storage model |
| `src/contributing/local-dev.md` | Build, test, RFC process |
| `src/contributing/translators.md` | Step-by-step guide for adding a locale |

### Test results

- `sui-id-i18n`: **12 tests pass**
- `sui-id-store`: **33 tests pass**
- `cargo check --workspace`: clean

---

## [0.36.0] — Previous release

**Minor version bump.** Completes the first UI/UX realignment wave (RFC 029–034)
and closes out the design-document gap list from the v0.29.x review. New routes,
new render-function signatures, and a new CSV export endpoint justify the minor bump.

### RFC 030 — Dangerous operations: step-up + confirmation screens

All six previously `confirm()`-dialog-gated operations now route through a
dedicated server-rendered confirmation screen with step-up authentication:

| Operation | Route |
|---|---|
| Disable/enable user | `GET /admin/users/{id}/disable-confirm` |
| Delete user | `GET /admin/users/{id}/delete-confirm` |
| Reset user MFA | `GET /admin/users/{id}/mfa-reset-confirm` |
| Delete client | `GET /admin/clients/{id}/delete-confirm` |
| Delete signing key | `GET /admin/signing-keys/{id}/delete-confirm` |

Each screen shows the target's name, an impact statement, a reversibility badge
(green "Recoverable" / red "Not recoverable"), and a labelled action button.
Step-up freshness is checked before rendering the confirmation screen for
irreversible operations. A hidden `_confirmed=1` field is required on the
mutation POST; direct-POST attempts without it are rejected 400.

JavaScript `confirm()` dialogs removed from all six locations.

New: `ConfirmedForm`, `require_confirmed()`, `reversibility_badge()` component.
New i18n: `confirm_*` and `badge_recoverable/badge_not_recoverable` (×3 locales).

### RFC 031 — Dashboard operator prompts + active session count

`DashboardData` gains three boolean warn flags and `active_session_count`:

- **Active sessions** stat card alongside users and clients.
- **Operator prompt section** (shown only when at least one condition is true):
  - SMTP not configured → link to Settings → Email
  - HIBP mode is Off → link to Settings → Authentication
  - `cookie_secure = false` → link to Settings → Security

New: `sessions::count_active_total()` in `sui-id-store`.

### RFC 033 — Audit log enhancements

Three new audit log capabilities:

1. **Hash-chain status banner** — `GET /admin/audit` now runs
   `verify_chain_tail` on each load and shows a green "✓ verified" or red
   "✗ check failed" banner at the top of the page.

2. **Event filter** — a `?q=` query parameter filters by event-name prefix
   (`auth.login`, `user.`, etc.). The filter persists in a visible search
   input.

3. **CSV export** — `GET /admin/audit.csv?q=` returns the same filtered
   rows as `text/csv` with columns `when,actor,action,target,result,note`.

New: `audit::recent_filtered()` in `sui-id-store`.

### RFC 034 — Login passkey primary button + empty states + Advanced tab

Three UI polish items:

- **Passkey on login screen**: a "Sign in with passkey" button above the
  password form (passed as `show_passkey_option: bool`).
- **Empty states**: user list, client list, and signing-key list now render
  a descriptive message when empty instead of an empty table body.
- **Settings tab rename**: "Other" / "その他" → "Advanced" / "詳細" / "高级".
  `settings_tab_advanced` i18n key (added in RFC 002) is now wired to the tab.
  `settings_tabs()` helper accepts `lang: Locale` and uses `t.` references
  for all tab labels.

### Ongoing: RFC 029 — Admin panel i18n (second pass)

Handler call sites still pass `Locale::Ja` as a static fallback. A follow-on
patch will resolve the locale dynamically from `server_settings.default_lang`
(tracked by the open RFC 029 in `rfcs/proposed/`).

### Test results

- `sui-id-i18n`: **12 tests pass**
- `sui-id-store`: **33 tests pass**
- `sui-id-core`: **114 tests pass**
- `cargo check --workspace`: clean

---

## [0.35.0] — Previous release

**Minor version bump.** This release begins the UI/UX realignment series
(RFC 029–035), addressing gaps identified against the v0.29.x design
documents. The minor bump reflects that RFC 032 changes `AppState` and
RFC 029 changes all admin render function signatures.

### RFC 032 — Dev mode browser banner

Every page rendered while sui-id runs in `--dev` mode now shows a yellow
sticky ribbon at the top of the browser window:

> **DEV MODE** — not for production. cookie_secure=false, HIBP off, lockout disabled.

Implementation:
- `AppState::is_dev_mode: bool` — false by default; set to `true` in the
  `--dev` code path in `main.rs`.
- `Shell` gains an optional `dev_mode: bool` prop. When `true`, a
  `<div class="dev-banner">` is rendered as the first element in `<body>`.
  The `.dev-banner` CSS class was already defined in RFC 023 (components.rs).
- All admin render functions accept and forward `dev_mode` to `Shell`.

### RFC 029 — Admin panel i18n: first pass

All five major admin render functions now accept a `lang: Locale` parameter
and route through the translation system:

- `render_dashboard` — title, stat labels, activity section, OIDC section
- `render_users` — title, section headings, table headers, form labels
- `render_clients` — title, secret-once banner, table headers
- `render_audit` — title, lede, column headers
- `render_signing_keys` — title, lede, table headers, action buttons

**New `Strings` fields (3 × 55 translations across Ja / En / Zh):**

`dashboard_title/lede/stat_*`, `users_title/lede/table_*`,
`clients_title/lede/table_*`, `audit_lede`,
`signing_keys_title/lede/table_*`.

**Note:** handler call sites currently pass `Locale::Ja` as a static
fallback. A follow-on change (RFC 031) will resolve the locale from the
`server_settings.default_lang` row dynamically.

### RFC plan (new RFCs filed this release)

7 new RFCs filed to track the remaining design-document gaps:

| RFC | Title | Priority |
|---|---|---|
| RFC 029 | Admin panel i18n completion (this release: first pass) | Medium-High |
| RFC 030 | Dangerous operations: step-up + confirmation screens | High |
| RFC 031 | Dashboard operator prompts + active session count | Medium-High |
| RFC 032 | Dev mode browser banner (this release: done) | High |
| RFC 033 | Audit log: hash-chain status, filter, export | Medium |
| RFC 034 | Login passkey primary + empty states | Medium |
| RFC 035 | Admin user detail page | Medium |

### Test results

- `sui-id-i18n`: **12 tests pass**
- `sui-id-store`: **33 tests pass**
- `cargo check --workspace`: clean

---

## [0.34.0] — Previous release

**Minor version bump.** RFC 002 adds a new locale (zh), a new public API
(`Formatters`), a new migration (0024), and a new field on `OutgoingMail` —
all breaking additions.

### RFC 002 — i18n scope expansion

Implements sub-threads B, C, D, E, and A from the RFC umbrella.

#### Sub-thread A — Chinese Simplified locale (`zh`)

`Locale::Zh` is now a fully supported locale. `STRINGS_ZH` provides
complete translations across all ~260 string fields. `FORMATTERS_ZH`
provides date/time/count formatters consistent with Mainland Chinese
conventions. `Locale::ALL` now contains three variants; all exhaustive
match guards that already iterate `ALL` pick up `Zh` without any
per-site change.

`Locale::parse("zh")` and `negotiate_from_accept_language("zh, ...")` now
return `Some(Locale::Zh)` — previously unknown.

#### Sub-thread B — `Formatters` struct

New `sui_id_i18n::Formatters` struct alongside `Strings`:

```rust
pub struct Formatters {
    pub fmt_date:      fn(DateTime<Utc>) -> String,
    pub fmt_time:      fn(DateTime<Utc>) -> String,
    pub fmt_date_time: fn(DateTime<Utc>) -> String,
    pub fmt_relative:  fn(at: DateTime<Utc>, now: DateTime<Utc>) -> String,
    pub fmt_count:     fn(u64) -> String,
}
```

- `Locale::formatters()` returns the locale-specific instance.
- **Ja**: `%Y年%-m月%-d日` dates; relative "3 時間前".
- **En**: `%-d %b %Y` dates; relative "3 hours ago" (singular-aware).
- **Zh**: `%Y年%m月%d日` dates; relative "3 小时前".
- `fmt_count` groups with commas (1,234,567) for all locales.

No ICU dependency. All formatter functions are plain `fn` pointers
(`&'static` compatible).

7 unit tests in `crates/sui-id-i18n/src/formatters.rs`.

#### Sub-thread C — Per-recipient locale for outbound mail

- **Migration 0024** adds a nullable `locale TEXT` column to
  `email_outbox`. The worker stores the BCP-47 tag resolved from the
  recipient's `preferred_lang` at enqueue time.
- `OutgoingMail` gains an `pub locale: Option<Locale>` field (defaults
  to `None` at all existing call sites).
- `OutboxMailSender::send` serialises the locale tag into the outbox row.

The worker now renders mail in the recipient's own language rather than
the requesting user's. Resolution order: recipient's `preferred_lang`
→ server default → Ja.

#### Sub-thread D — Audit event labels

30 new fields added to `Strings`, grouped under `// Audit event labels`:
`audit_event_auth_login_success`, `audit_event_user_create`, etc.
One additional field: `settings_tab_advanced` (RFC 023 renamed the
settings "Other" tab to "Advanced"; the i18n key was previously missing
in the typed `Strings` struct).

All three locales (Ja, En, Zh) have complete translations.

#### Sub-thread E — `Locale::direction()` + HTML `dir=` attribute

- `Locale::direction()` returns `"ltr"` or `"rtl"` (all current locales
  return `"ltr"`; RTL locales will override when added).
- `Shell` in `layout.rs` now sets `<html dir={direction}>` alongside
  `<html lang={tag}>`. No visual change for LTR locales; correct foundation
  for Arabic/Hebrew/Persian when they land.

### Test results

- `sui-id-i18n`: **12 tests pass** (7 formatter + 5 existing)
- `sui-id-store`: **33 tests pass**
- `sui-id-core`: **114 tests pass**
- `cargo check --workspace`: clean
- `cargo check -p sui-id --tests`: clean

---

## [0.33.0] — Previous release

**Minor version bump.** RFC 001 introduces a new DB migration (0023) and a
new in-process background worker, both of which affect the startup sequence.

### RFC 001 — Persistent email outbox + retry worker

Outgoing mail is no longer sent inline with the HTTP request that triggered
it. Instead, requests enqueue a row in the new `email_outbox` table and
return immediately; the `OutboxWorker` background task drains the queue
with exponential backoff.

#### What changed for operators

- **Reduced handler latency.** `/forgot-password` and password-change
  notifications no longer block on SMTP. The response returns immediately
  regardless of SMTP availability.
- **Automatic retry.** Failed deliveries are retried up to 5 times on the
  schedule: 30 s → 2 m → 10 m → 1 h → 6 h. After 5 attempts the row is
  marked `failed` and a `mail.outbox.permanent_failure` audit event is
  written.
- **Restart safety.** Any row in `sending` state when the process exits is
  reset to `queued` on the next startup by `requeue_stuck_sending`.
- **Encryption unchanged.** `recipient_enc` and `payload_enc` are sealed
  under the master key with dedicated AADs; both columns are added to the
  `admin rotate-key` reseal harness.

#### Schema

Migration **0023** adds:

```
email_outbox (id, state, template, recipient_enc, payload_enc,
              attempt_count, next_attempt_at, last_error,
              created_at, updated_at)
```

Partial index on `(next_attempt_at) WHERE state = 'queued'` for fast
scheduler polls.

#### New types and APIs (all in `sui-id-core` / `sui-id-store`)

- `sui_id_shared::ids::EmailOutboxId`
- `sui_id_store::models::{EmailOutboxState, EmailOutboxRow}`
- `sui_id_store::StoreError::InvalidData`
- `sui_id_store::repos::email_outbox::{enqueue, claim_one_eligible,
  mark_sent, record_failure, mark_permanently_failed,
  requeue_stuck_sending, reseal_all}`
- `sui_id_core::mail::outbox::{OutboxMailSender, OutboxWorker}`

#### Dev mode unchanged

`test_app()` / `test_app_with_mailer()` still use `InMemoryMailSender`
directly. The outbox path is production-only; tests observe mail via the
in-memory sender as before.

#### Tests

5 new unit tests in `sui-id-store`: `enqueue_and_claim_round_trip`,
`claim_respects_next_attempt_at`, `mark_sent_after_claim`,
`record_failure_increments_attempt_count`,
`requeue_stuck_sending_resets_old_rows`.

### Test results

- `sui-id-store`: **33 tests pass** (28 previous + 5 email_outbox)
- `sui-id-core`: **114 tests pass**
- `cargo check --workspace`: clean
- `cargo check -p sui-id --tests`: clean

---

## [0.32.0] — Previous release

### RFC 017 — UI/UX design contracts

Adds [`docs/ui-ux-contracts.md`](docs/ui-ux-contracts.md), the frozen
cross-cutting contract for the admin domain UI. Sections:

- **§ 1** Screen relation map (five-stream isolation)
- **§ 2** Screen responsibilities matrix
- **§ 3** Dangerous-operation UI pattern (step-up + explicit-verb confirm)
- **§ 4** State copy contract (loading / empty / success / error / disabled)
- **§ 5** Admin dashboard information policy
- **§ 6** Settings tab structure (six fixed tabs; Advanced tab isolates risky knobs)
- **§ 7** Client management UI constraints
- **§ 8** Audit log display rules
- **§ 9** Dev mode UI separation
- **§ 10** Accessibility implementation contract (focus ring, ARIA, keyboard)
- **§ 11** Text selection contrast (WCAG 2.1 SC 1.4.3 requirement)

Implementation RFCs (002, 003, 008, 010–012, 016, 023) reference this document
as their inherited contract. No code change.

### RFC 023 — Visual design system

Completes the CSS token and component system shipped to the binary in
`sui-id-web`. All changes are in `tokens.rs` and `components.rs`.

**tokens.rs additions:**

- **Motion tokens** — `--motion-instant/fast/base/slow` and
  `--motion-easing`. Components reference these for `transition-duration`;
  the `prefers-reduced-motion` override block zeros them automatically so
  no per-component duplication is needed.
- **Z-index scale** — `--z-below / --z-base / --z-raised / --z-overlay /
  --z-dropdown / --z-modal / --z-toast`. Named layers prevent magic numbers.
- **`@media (prefers-reduced-motion: reduce)`** block — zeros all motion
  tokens and applies `animation-duration: 0.01ms` globally.
- **`::selection` styles** — moved from components.rs to tokens.rs and
  explicitly meeting WCAG 2.1 SC 1.4.3 contrast requirements in both
  modes (light: ~13:1, dark: ~7:1).

**components.rs additions:**

- **Tab component** (`.tabs`, `.tabs__bar`, `.tab-btn`) — horizontal tab
  bar with motion-token transitions for Settings and similar multi-panel
  screens. `aria-selected="true"` drives the active indicator.
- **Dev-mode banner** (`.dev-banner`) — yellow ribbon displayed on every
  page when `--dev` is active, with `.dev-banner__bind-warn` for the
  non-loopback warning (RFC 017 § 9).
- **Motion-aware transitions** — `button`, `input`, `a` and related elements
  now reference `var(--motion-fast)` instead of hardcoded durations.
- **Reversibility badge** (`.reversibility-badge--recoverable` /
  `--permanent`) — coloured badge for dangerous-operation confirm screens
  (RFC 017 § 3). Colour is never the sole signal; badge text "Recoverable"
  / "Not recoverable" is always present.

### RFC 024 — Documentation consolidation

- **`CHANGELOG.md`** — now a thin index of current-release notes plus links
  to `docs/changelog/v0.30.md` (0.30.x history) and
  `docs/changelog/archive.md` (0.29.x and earlier). Reduces the root file
  from 5,304 lines to ~90.
- **`ROADMAP.md`** — compressed from 639 lines to 64 lines: an RFC index
  table, a near-term priority statement, a "completed" table, and a
  constraints section. Stale detail moved into the completed-RFC files.

---

## [0.31.0] — Previous release

**Minor version bump.** RFC 014 (hot-path caches) introduces a new cache
subsystem and changes the `AppState` constructor — both are breaking API
additions. RFC 028 (copy buttons, v0.30.1) ships in the same release.

### RFC 028 — Copy-to-clipboard for credential values (v0.30.1 → rolled in)

Adds `📋 Copy` buttons next to Client ID, client secret, User UUID, and
JWKS URI. The `clipboard-available` CSS class is set by a small inline JS
snippet when `navigator.clipboard` is present; buttons are hidden without
it (non-HTTPS contexts degrade cleanly).

### RFC 014 — Hot-path caches

Two request-critical DB reads are now served from in-process caches:

#### Cache 1 — Redirect-origin set (`RedirectOriginsCache`)

`/oauth2/token` CORS pre-flight previously queried every registered client
on every request to build the allowed-origins set. The cache is now
rebuilt once at startup and after every client mutation (create / update /
disable / delete). CORS checks call `caches.redirect_origins.contains(origin).await`
— a single `RwLock::read` instead of a DB round-trip.

#### Cache 2 — Active signing keys (`JwksCache`)

`verify_access_token` and `verify_id_token` previously loaded the
published-keys list from the DB on every call. The cache is rebuilt once
at startup and after every signing-key rotation or deletion. Hot paths
call `verify_access_token_cached` / `verify_id_token_cached`, which take
a snapshot of the key list from the cache.

#### Cache design

- Both caches are `tokio::sync::RwLock<T>` snapshots stored as `Arc<Caches>`
  in `AppState`.
- Writes hold the lock only during the in-memory update (microseconds).
- Rebuild on mutation is synchronous with the write: if the rebuild fails,
  the mutation still returns success but the cache keeps the previous
  snapshot and a `warn!` log is emitted.
- Cold start: caches are pre-populated during `startup::prepare()`. A
  startup rebuild failure yields an empty cache and a warn log; the next
  successful mutation re-syncs.

#### New public API

- `sui_id_core::cache::Caches` — combined cache handle, stored in `AppState`.
- `sui_id_core::cache::RedirectOriginsCache::contains(&self, origin) -> bool` (async)
- `sui_id_core::cache::JwksCache::snapshot(&self) -> Vec<CachedSigningKey>` (async)
- `tokens::verify_access_token_cached(caches, clock, token)` — hot-path variant.
- `tokens::verify_id_token_cached(caches, clock, token, accept_expired)` — hot-path variant.
- `signing_keys::list_active(db)` — new repo function (active keys only).

#### Breaking: `AppState::new` gains a `caches: Arc<Caches>` parameter

All construction sites (startup, tests, dev-mode, CLI sub-commands) updated.

#### Cache invalidation hooks

`admin::{create_client, update_client, update_client_basic, set_client_disabled,
delete_client}` all rebuild `redirect_origins` on success.
`admin::{rotate_signing_key, delete_signing_key}` rebuild `jwks` on success.
All accept `caches: &Caches` as a new final parameter.

#### Test updates

- 3 new unit tests in `cache.rs` (origin extraction, contains, snapshot).
- E2E tests updated throughout: `AppState::new` call sites, async helper
  functions, `db.with_conn` missing `.await`, mailer async methods,
  `move` closures for captured `user.id` / `stale`.

### Test results

- `sui-id-store`: 28 tests pass
- `sui-id-core`: 114 tests pass (111 previous + 3 cache tests)
- `cargo check --workspace`: clean
- `cargo check -p sui-id --tests`: clean (e2e test compilation)

---

---

## Older releases

| Version series | File |
|---|---|
| 0.30.x | [docs/changelog/v0.30.md](docs/changelog/v0.30.md) |
| 0.29.x and earlier | [docs/changelog/archive.md](docs/changelog/archive.md) |
