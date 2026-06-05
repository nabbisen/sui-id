# Changelog

All notable changes to sui-id will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.38.0] — Unreleased

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
