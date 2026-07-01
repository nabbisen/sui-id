# Roadmap

This file is a loose sketch of direction — nothing here is a promise.
Completed work is tracked in [CHANGELOG.md](CHANGELOG.md) and the
[`rfcs/done/`](rfcs/done/) directory.

---

## Active proposals (proposed RFCs)

**Security-assurance arc — RFCs 078–086 (v0.63.2).** Created by
the architect audit
(`docs/security-assurance-audit-v0.63.1.md`). Recommended
sequencing — each step independently shippable:

| Step | RFC | Theme | Suggested release |
|---|---|---|---|
| ✅ 1 | [078](rfcs/done/078-security-type-modeling-baseline.md) | Type modeling baseline (newtypes, secret redaction) | v0.64.0 |
| ✅ 2 | [080](rfcs/done/080-refresh-rotation-atomicity.md) | Refresh rotation atomicity + reuse detection | v0.66.0 |
| ✅ 3 | [079](rfcs/done/079-authorization-code-lifecycle-assurance.md) | Auth-code single-use by statement | v0.66.0 |
| 4 | [081](rfcs/proposed/081-actor-scope-boundary.md) | Actor scope boundary | v0.66.0 |
| 5 | [082](rfcs/proposed/082-authorization-decision-core.md) | Pure authorization core | v0.66.0 |
| 6 | [083](rfcs/proposed/083-security-state-machine-testing.md) | State-machine proptest harness | v0.67.0 |
| 7 | [085](rfcs/proposed/085-audit-event-completeness.md) | Audit completeness + atomicity | v0.67.0 |
| 8 | [084](rfcs/proposed/084-fuzzing-untrusted-input-boundaries.md) | Fuzzing harness | v0.68.0 |
| 9 | [086](rfcs/proposed/086-formal-model-checking-pilot.md) | Kani / TLA+ / Flux pilots (time-boxed) | evaluation only |

**Note — v0.65.0 took the token-foundation slot.** v0.65.0 shipped the
UI-security handoff's Unit 1 (WCAG AA contrast + explicit disabled tokens
+ a contrast CI test; see CHANGELOG). The auth-core suggested releases
above therefore shift forward by one (079 / 080 → v0.66.0, 081 / 082 →
v0.67.0, 083 / 085 → v0.68.0, 084 → v0.69.0); 086 stays evaluation-only.
Targets remain indicative, not commitments.

**UI-security contract — handoff units 1–6.** The approved v2.3 UI/UX
contract defines six units. Unit 1 (design tokens) is done (v0.65.0).
Units 2–6 are `[NEW CONTRACT]` items and will each be filed as an RFC
**behind the existing auth-core set** — RFC 087 onward, sequenced after
079–086 — because they build on those primitives: the actor-scope
boundary (081) underpins the auditor authorization matrix; the
authorization-decision core (082) underpins 403-before-step-up and
final-POST revalidation; audit completeness (085) underpins pending-change
audit events. RFC numbers are assigned at file creation, not pre-reserved.
Before drafting 087+, the v2.3 contract is reconciled against the
as-built 081 / 082 shapes.

_Known deferred prerequisite (→ unit 6)._ The `ThemeToggle` contract
(blocking theme-init in `<head>`, `no-js` / `js` root-class swap,
`<noscript>` fallback, `localStorage` try/catch) is held whole for unit 6
rather than split across releases. The current `defer`-loaded init can
flash an unthemed frame on first paint — a visual-only issue with no
security impact, lowest in the priority order.

**Mockup Integration epic — sixteen RFCs, Phase 0 → Phase 8.**
Introduced in v0.49.0. The full epic table and reading order live
in [`rfcs/README.md`](rfcs/README.md) ("Proposed — Mockup
Integration epic"); see also
[`docs/mockup-integration/`](docs/mockup-integration/).

| RFC | Title | Priority | Notes |
|---|---|---|---|
| [RFC 004](rfcs/proposed/004-federation.md) | OIDC/SAML federation (upstream IdP) | Low | Identity provider chaining |
| [RFC 005](rfcs/proposed/005-pluggable-user-backends.md) | Pluggable user backends | Low | LDAP/AD directory integration |
| [RFC 006](rfcs/proposed/006-metrics.md) | Metrics and observability | Low | Prometheus / OpenTelemetry |
| [RFC 008](rfcs/proposed/008-third-party-posture.md) | Third-party posture / consent screen | Low-Medium | Explicit consent for external RPs |
| [RFC 009](rfcs/proposed/009-sql-backends.md) | Alternative SQL backends | Low | PostgreSQL / MySQL support |
| [RFC 025](rfcs/proposed/025-multi-tenant-expansion.md) | Multi-tenant expansion | Low | Per-tenant namespaces (post-1.0) |

---

## Near-term (next 5–6 releases)

**The v0.42 → v1.0-rc UI/UX hardening plan** is the main near-term
direction. Six phases (A–F), each shipping in one release. The plan
addresses correctness gaps surfaced during a v0.41.0 implementation
review: the rendered UI was not matching the design contract the v0.40
HANDOFF claimed had been met.

| Phase | Version  | Theme                                              | RFCs (planned)       |
|-------|----------|----------------------------------------------------|----------------------|
| **A** | v0.42.0  | Stop the bleeding (this release)                   | 048, 049, 050        |
| **B** | v0.43.0  | i18n completeness sweep                            | 051, 052, 053, 054   |
| **C** | v0.44.0  | Self-service unification (`/me/security/*`)        | 055, 056, 057        |
| **D** | v0.45.0  | Dangerous operations contract                      | 058, 059, 060        |
| **E** | v0.46.0  | Visual hierarchy + palette extension               | 061, 062, 063, 064   |
| **F** | v0.47.0  | Code structure (split `pages.rs` and admin.rs)     | 065, 066, 067        |
| —     | v0.48.0  | Buffer + RFC index / docs reconciliation           | 068, 069             |

v1.0-rc follows once Phases A–F are clean.

The plan is intentionally correctness-first: visible polish (Phase E)
lands fifth, only after the underlying i18n, navigation, and
dangerous-operation contracts are honest. See
[`docs/src/contributing/`](docs/src/contributing/) and the individual
proposed RFCs once they enter the repository at each phase start.

---

## Mockup Integration arc (v0.49.0 → )

Following Phases A–F, the project enters the **Mockup Integration
("MI") arc**: a controlled migration that adopts the
`sui-id-web-mockup-v0.4.8` UI/UX language into the product. Eight
phases (0 → 8), each backed by one or more `RFC-MI-NNN` documents
in `rfcs/proposed/`. v0.49.0 opens the arc with the Phase 0 planning
artifacts only — **no runtime code changes** — so subsequent
implementation work has an auditable baseline. See
[`docs/mockup-integration/`](docs/mockup-integration/) for the
migration plan, codebase handoff, and mockup handoff package.

| Phase | Target version | Theme                                                   | RFCs                          |
|-------|----------------|---------------------------------------------------------|-------------------------------|
| **0** | v0.49.0–0.49.1 | Planning + baseline inventory                           | RFC-MI-000 → `done/`          |
| **1** | v0.50.0–0.50.1 | CSS sharding; token mapping; theme decision             | RFC-MI-010–012 → `done/`      |
| **2** | v0.51.0–0.51.1 | Shell decision; CSRF; route-based tabs                  | RFC-MI-020–022 → `done/`      |
| **3** | v0.52.0        | Dashboard + audit read-only screens                     | RFC-MI-030, 031 → `done/`     |
| **4** | v0.53.0–0.53.1 | Auth surfaces + setup wizard                            | RFC-MI-041, 040 → `done/`     |
| **5** | v0.54.0        | Form system + danger zone                               | RFC-MI-050, 051 → `done/`     |
| **6** | v0.55.0        | Self-service `/me/security/*`                           | RFC-MI-060 → `done/`          |
| **7** | **v0.56.0**    | **OIDC consent UX (this release; Phase 7 complete)**    | RFC-MI-070 → `done/`          |
| **8** | v0.57.0        | Responsive + a11y hardening (MI arc done)               | RFC-MI-080 → `done/`          |
| —     | v0.57.1        | Dependency refresh: rand 0.10 + reqwest                 | RFC 069, 070 → `done/`        |
| —     | v0.58.0        | Dashboard action items                                  | RFC 073 → `done/`             |
| —     | v0.59.0        | Auditor role                                            | RFC 071 → `done/`             |
| —     | v0.60.0        | End-user app-access surface| **End-user app-access surface (this release)**          | RFC 072 → `done/`             |

Phase-1 blockers (`D-01` / `D-02` / `D-03` in the migration plan)
must be resolved before any code-level visual replacement starts:
component CSS sharding, path-based-tab preservation, and CSRF
threaded through `Shell` server-side. Target versions above are
indicative — not commitments. v1.0 designation continues to be
deferred (verification phase, spec §22).

---

## Completed (recent)

| Version | What shipped |
|---|---|
| v0.67.0 | **RFC 081 + RFC 082 (actor scope boundary + authorization decision core).** `Actor`/`AdminActor`/`ReadOnlyAdminActor`/`SelfActor` capability types; pure `authorize(role, action) -> Decision` table with exhaustive tests (P1–P5). All admin mutations now require `&AdminActor`; self-service requires `&SelfActor`; a privileged call without proof of privilege is a compile error. Last-admin safeguard delegates to the authz table. `require_admin` deprecated. **90/90 tests; all CI invariants unchanged.** |
| v0.66.0 | **RFC 079 + RFC 080 (auth-code lifecycle assurance + refresh-token rotation atomicity).** `consume` enforced by SQL predicate + rows-affected guard. Typestate pipeline (`ConsumedCode`→`BoundCode`→`PkceVerifiedCode`→`IssuableGrant`) in `exchange_code`. `begin_rotation` closes the 3-closure TOCTOU race with a single-tx rows-affected arbitration; `RotationLookup` makes reuse-detection explicit. Migration 0031. **90/90 tests; all CI invariants unchanged.** |
| v0.65.1 | **RFC 087 (clippy/rustfmt baseline cleanup).** All four buildable crates clippy-clean (`--all-targets -D warnings`) and fmt-clean under Rust 1.96. Fixes across sui-id-web (16), sui-id-shared (2), sui-id-store (16 lib + test-target), sui-id-i18n (3). 31 files reformatted. No logic change. **78/78 tests; all CI invariants unchanged.** |
| v0.65.0 | **WCAG AA contrast correction — token foundation (UI/UX handoff unit 1).** Dark-mode AA defect fixed (all 5 colour pairs were failing, worst 1.5:1). Light-mode fills darkened to pass AA. Explicit `--fg-disabled`/`--bg-disabled` tokens; `button:disabled` wired to explicit tokens. Contrast CI test (`tokens/tests.rs`) validates all pairs in 3 modes. Dangling `--surface-overlay` reference fixed. **78/78 tests; all CI invariants unchanged.** |
| v0.62.0 | **RFC 075 + RFC 076 (soak cleanup).** Mechanical file splits: `admin.rs`→`admin/`, `backup.rs`→`backup/`, `main.rs`→`cli.rs`. Full `configuration.md` reference (10 fields, env vars, flags, examples). **175/175 tests; all CI invariants unchanged.** |
| v0.61.0 | **RFC 074 (Navigation + UX polish).** User-menu dropdown replaces flat Security link. "Apps" nav label. Settings: Basic→General, Other→Advanced. Migration 0030 (`last_login_at`); last-login anti-phishing line on `/me/security/overview`. 6 i18n keys. **175/175 tests PASS; all CI invariants unchanged.** |
| v0.60.1 | **v0.60.1 (Documentation).** CHANGELOG dated; README and docs updated for three-role model and UX-rethink arc; RFC 074 filed. No code changes. |
| v0.60.0 | **RFC 072 (End-user app-access surface).** Migration 0029 (`user_consent.last_used_at`). `list_for_user`, `revoke_with_tokens`, `touch_last_used` repo helpers. `TokenSet.user_id` for best-effort `last_used_at` update at token exchange. `MeTab::Apps` + `render_me_apps`. `GET /me/apps` + `POST /me/apps/{id}/revoke`. 9 i18n keys. **175/175 tests PASS; all CI invariants unchanged.** |
| v0.59.0 | **RFC 071 (Auditor role).** `users.role` column (migration 0027) + `audit_log.actor_role` (0028). `Role` enum with `is_admin()` / `can_read_admin()`. `CurrentAdminOrAuditor` extractor on all GET admin routes. `can_write: bool` in 5 render functions hides mutation controls from auditors. Role-change UI on user detail with last-admin safeguard. 7 new i18n keys. **175/175 tests PASS; all CI invariants unchanged.** |
| v0.58.0 | **RFC 073 (Dashboard action items).** Getting Started checklist (3 items, ☐/✓ ABDD-safe text indicators) + 4 new action items (admins without MFA, old signing key, stuck outbox, pending resets). 4 new read-only repo helpers. 8 i18n keys (×3 locales). `.action-items-list` and `.checklist` CSS. **228/228 tests PASS; all CI invariants unchanged.** |
| v0.57.1 | **Dependency refresh: RFC 069 (rand 0.10) + RFC 070 (ureq → reqwest).** rand 0.8→0.10 via getrandom; `OsRng.fill_bytes` (×10), `SaltString::generate`, `SigningKey::generate` (Option B: Zeroizing + from_bytes) all migrated. ureq removed; `HibpClient` trait made async via async-trait; `HttpHibpClient` rebuilt on reqwest 0.12. Bug fixed: enforce_hibp now properly awaits the check instead of blocking the tokio thread. **228/228 tests PASS; all CI invariants unchanged.** |
| v0.57.0 | **Phase 8 complete — MI arc fully closed: RFC-MI-080 (UI Regression + A11y Hardening).** Skip link added to Shell and AuthShell (WCAG 2.4.1). `<header role="banner">`, `<main id="main-content">`. `@media (max-width: 480px)` and `(max-width: 360px)` breakpoints added. New i18n key `a11y_skip_to_main`. Six verification matrices committed (`docs/src/mockup-integration/`). **16/16 MI RFCs in `done/`. `inline-style-bound` = 0. 228/228 tests PASS.** |
| v0.56.0 | **Phase 7 complete: RFC-MI-070 (OIDC Consent UX). `inline-style-bound` reaches 0.** Four inline styles in `pages/oidc.rs` eliminated via `.consent-card`, `.consent-intro`, `.consent-scope-list`, `.consent-scope-item` classes. Scope item structure improved. PKCE/redirect validation unchanged. 15/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.55.0 | **Phase 6 complete: RFC-MI-060 (Self-Service Security Tab Integration).** Password-change page (`render_password_change`) updated: `show_nav=true`, `current="me"`, tab strip added. All six `/me/security/*` routes now consistently render `.route-tabs` with `aria-current="page"`. MFA enable/disable decision documented (Option 2: self-service + admin reset). Cancel link updated to `/me/security/overview`. Form actions migrated to `.form-actions`. No i18n changes. `inline-style-bound` = 4 (unchanged). 14/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.54.0 | **Phase 5 complete: RFC-MI-050 (Form System) + RFC-MI-051 (Danger Zone).** Two new form CSS primitives (`.field--required`, `.review-summary`) added to `forms.rs`. User detail page restructured: action buttons moved from header into a `.danger-zone` section at the bottom. New i18n key `user_detail_danger_zone_body` (×3 locales). All confirmation routes unchanged. `inline-style-bound` **5 → 4**. 14/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.53.1 | **Phase 4 complete: RFC-MI-040 (Setup Wizard UX).** `StepState` enum + `SetupStep` struct added to `components/setup.rs` (re-exported from `components.rs`). `.setup-steps` nav container class and `.setup-step__label--{current,done,upcoming}` classes replace the two inline style= attributes in `setup_step_indicator()`. `inline-style-bound` **7 → 5**. 12/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.53.0 | **Phase 4 opens: RFC-MI-041 (Authentication Surfaces).** Ships ahead of MI-040 at user request. Three inline styles eliminated in `pages/auth.rs` (login forgot-password link, MFA QR code, password-change card). Three new CSS classes: `.auth-meta-link`, `.qr-display`, `.card--narrow`. ABDD: `FlashKind::aria_role()` added to `common.rs` — `Error` → `role="alert"`, `Info`/`Warn` → `role="status"`. **Zero copy / zero i18n changes** (security review confirms anti-enumeration wording byte-identical). `inline-style-bound` **10 → 7**. 10/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.52.0 | **Phase 3 complete: RFC-MI-030 (Dashboard) + RFC-MI-031 (Audit + Tables).** Dashboard: warning callout migrates to `.callout--warning`; 4 sparkline inline styles eliminated via `.sparkline-{container,header,title,legend}` classes. Audit: `.cell-id`, `.cell-nowrap`, `.cell-actions` added to `tables.rs`; applied to `audit_row_view`; filter row inline style eliminated via `.filter-bar` class. Total: **6 inline styles eliminated; `inline-style-bound` 16 → 10**. 9/16 MI RFCs in `done/`. **228/228 tests PASS.** |
| v0.51.1 | **Phase 2 complete: RFC-MI-022 (Route-Based Tab Component).** `.route-tabs` + `.route-tabs__link` CSS added. `RouteTab` + `route_tabs()` fn. `MeTab::Password` added. Both tab helpers migrated. `inline-style-bound` 17 → 16. **228/228 tests PASS.** |
| v0.51.0 | **Phase 2 opens: RFC-MI-020 (Shell Layout decision) + RFC-MI-021 (Server-Rendered CSRF).** Shell: keep top-nav decision recorded; no structural code change. CSRF: Shell now requires `csrf_token: String`; Nav renders token directly into sign-out form hidden field; `logout-csrf.js` removed. 27 Shell call sites updated; 5 render function signatures updated. Sign-out works with JS disabled. **228/228 tests PASS; 0 warnings; all 4 CI invariants unchanged.** |
| v0.50.1 | **Phase 1 complete: RFC-MI-011 (Token Mapping + Visual Primitives) + RFC-MI-012 (Theme Persistence).** Zero new CSS tokens (mockup vocabulary is a strict subset of the product's). Three CSS primitives adopted: `.callout` + tone variants (→ `cards.rs`), `.field__error` + `.field--invalid` (→ `forms.rs`), `.dl-grid` (→ `utilities.rs`). Theme persistence: **Option A chosen** (preserve `localStorage` model, no code change). Phase-1 blockers D-01/D-02/D-03 status: D-01 resolved (v0.50.0); D-02 and D-03 owned by Phase 2 (RFC-MI-022 and RFC-MI-021). **228/228 tests PASS; 0 warnings; all 4 CI invariants unchanged.** |
| v0.50.0 | **Phase 1 opens: RFC-MI-010 (Component CSS Sharding).** `components.rs` (1094 lines) split into 11 bounded shards under `components/` (badges, banners, buttons, cards, chrome, confirm, forms, setup, tables, tabs, utilities). `StatusKind` + `status_badge` moved to `badges.rs`; re-exported from `components.rs` for backward compatibility. `components_css()` fn (OnceLock-cached) replaces the former `COMPONENTS_CSS` const — produces a byte-identical CSS body to v0.49.x. Phase-1 blocker `D-01` (CSS sharding) resolved. **228/228 tests PASS; 0 warnings; all 4 CI invariants unchanged.** |
| v0.49.1 | **Phase 0 of the Mockup Integration arc completes.** The six baseline-inventory documents specified by `RFC-MI-000` (`screen-map.md`, `dangerous-action-map.md`, `tab-routing-delta.md`, `token-delta-draft.md`, `i18n-copy-delta-draft.md`, `route-render-handler-map.md` + a `README.md` index) ship under `docs/mockup-integration/inventory/`. Headline findings: zero new CSS tokens (mockup vocabulary is a strict subset of the product's), 18 dangerous-action values reduce to 9 link-rewrites + 5 do-not-implement + 3 step-up-policy-deltas + 1 inline-only, the 382 mockup-only i18n keys are mostly renames (~58 net-new keys × 3 locales = ~174 translation entries). `RFC-MI-000` moves to `rfcs/done/` with `Status = Implemented (v0.49.1)`. **No runtime code change**; CI invariants unchanged; 228/228 library tests PASS. |
| v0.49.0 | **Opens the Mockup Integration ("MI") arc.** Sixteen `RFC-MI-NNN` documents added to `rfcs/proposed/` (Phase 0 → Phase 8 plan); supporting planning artifacts placed under `docs/mockup-integration/` (migration plan, codebase handoff, mockup handoff package) and `docs/development-specification.md` (v3 spec). `rfcs/README.md` rewritten to surface the MI namespace and the eight-phase implementation order. Phase-1 blockers `D-01`/`D-02`/`D-03` restated. Workspace version → 0.49.0. **No runtime code changes**: CI invariants unchanged at their v0.48.4 values (228/228 floor unaffected; text-leaks 0; inline-style-bound 16; css-tokens green; semantic-palette-parity 12×3). |
| v0.48.4 | **Setup UX.** (1) Setup token moved from text-input to URL parameter: startup now prints a full URL (`/setup?token=xxx`), the welcome screen forwards it to `/setup/admin?token=xxx`, and the admin form holds it as `<input type="hidden">` — operators no longer copy-paste a raw token string. Token travels through language PRG redirects and error re-renders unchanged. (2) Chinese (`中文`) removed from setup wizard language picker — core i18n covers ja and en only; showing zh would be misleading. 228/228 PASS; 0 warnings. |
| v0.48.3 | **Verification-phase bug: `email` claim absent from ID token.** External RP reported `JSON error: missing field 'email'` at OIDC callback. `IdTokenClaims` had no `email`/`email_verified` fields; only the UserInfo endpoint returned them. OIDC Core §5.1: `email` scope SHOULD populate those claims in the ID token too. Fix: added `email: Option<String>` + `email_verified: Option<bool>` (both `skip_serializing_if = "Option::is_none"`) to `IdTokenClaims`; `issue_token_set` takes a new `user_email: Option<(&str, bool)>` param; `exchange_code` passes it from the already-fetched user row; `exchange_refresh` adds a conditional `users::get` only when scope includes `"email"`. Accounts without email → field omitted (not null). `email_verified` faithfully reflects `email_verified_at IS NULL`. 228/228 tests PASS; 0 warnings; CI PASS. |
| v0.48.2 | **Second verification-phase release (verification-pass buffer).** Six issues from the same real-environment round that produced v0.48.1. **Bug 1** (`::selection` invisible): `--accent-default` + `--fg-on-accent` replaces `--accent-subtle`. **Bug 5** (`/me/security/overview` i18n): 3 hardcoded/miskeyed strings replaced with 3 new keys (`me_overview_label_mfa_totp`, `me_overview_label_passkeys`, `me_overview_no_recent_events`) × en/ja/zh. **Issue 4** (setup wizard language): explicit 3-button picker on welcome screen, `?lang=xx` → LANG_COOKIE set (PRG) → all subsequent wizard steps auto-locale via existing RequestLocale. **Issue 6** (footer a11y labels): `<ul role="note">` / `<li class="app-footer__a11y-item">` with `cursor: default` and caption sizing — passive informational badges, not interactive. **Issue 7** (tagline prominence): caption-size + muted + opacity 0.75. **Bug 8** (mobile responsive): first `@media (max-width: 768px)` in codebase; `.app-nav__link { white-space: nowrap }` + `td/th { white-space: nowrap }` + `.cell-wrap` opt-out class; nav horizontal-scroll, main padding shrink, footer column stack. Tests stable at 228/228; 0 warnings; CI invariants PASS. |
| v0.48.1 | **First verification-phase hotfix.** Three lock-out / main-feature bugs surfaced during actual-environment testing of v0.48.0 at localhost:8801 — all CSP-related. **Bug 2** (CSP `script-src 'self'` blocking 3 inline `<script>` blocks + 3 inline `onclick=` handlers → theme toggle, clipboard copy, sign-out all silently failed): externalised the inline JS into `/static/theme-init.js`, `/static/copy.js`, `/static/logout-csrf.js`; theme buttons keep only `data-theme-value` attributes and `theme-init.js` attaches listeners on DOM-ready. **Bug 3** (sign-out → /admin redirect loop): subsumed by Bug 2 fix — CSRF token injection script now runs. **Bug 9** (401 lock-out after restart, "Back home" loops to /admin): `html_error_response` now redirects `CoreError::Unauthenticated`+HTML to `/admin/login` instead of rendering a 401 page; `pages/error.rs` "Back home" is context-aware (401 → /admin/login, else → /). Tests stable at 228/228; 0 workspace warnings; CI invariants all PASS. No new RFC consumed (hotfix scope). Six other v0.48.0 issues (`::selection` color, /me/security/overview i18n, mobile responsive on nav + tables, setup wizard language picker, footer a11y label intent, title tagline restraint) deferred to v0.48.2 — none of them lock operators out. |
| v0.48.0 | **Phase F (final buffer)** — RFC 068 (`handlers/me_security.rs` 1099 LOC → 7 sub-modules, Rust 2018+ style; all under 500 LOC) + RFC 067 (inline-style discipline: 119 → 16 with 40+ utility classes in `components.rs`; new CI bound `inline-style-bound` at 20). Pre-existing warning cleanup: 5 issues cleared (dead `mailer`/`title`, `_caches`/`_clock` rename for API symmetry, `decrypt_field` allow(dead_code)). 0 workspace warnings. Phase F closes; project enters verification phase. **No v1.0-rc/pre tag is scheduled from this release** — sufficient soak, external review, and integration verification precede any v1 designation. |
| v0.47.1 | **Phase F (continued)** of the UI/UX hardening plan — RFC 066 (`handlers/admin.rs` 1531 LOC → 8 sub-modules under `admin/`, Rust 2018+ module style; every file under spec's 500-LOC ceiling, umbrella 55 LOC; public route paths unchanged through `pub use {submodule}::*;` re-exports). Hygiene: 14 `#[derive(...)]` attributes lost during extraction were re-attached from the original; 85 unused-import warnings auto-pruned by `cargo fix`; `_silence_state*` dead-code suppressors removed (the split made them unnecessary). RFC 067 (inline-style discipline) + `handlers/me_security.rs` split deferred to v0.48.0, the final Phase F buffer release. |
| v0.47.0 | **Phase F (partial)** of the UI/UX hardening plan — RFC 065 (`pages.rs` 4170 LOC → 22 sub-modules under `pages/`, Rust 2018+ module style throughout; every file under spec's 500-LOC ceiling; sub-directory splits for `settings/` and `me_security/`; public API surface unchanged through `pub use {submodule}::*;` re-exports). Build hygiene: 22 unused-variable warnings cleared; 7 genuine dead code removals (`let csrf_*`/`let *_url` from pre-Phase-D row buttons). RFC 066 (admin.rs split) deferred to v0.47.1; RFC 067 (inline-style discipline) + `handlers/me_security.rs` split deferred to v0.48.0. |
| v0.46.0 | **Phase E** of the UI/UX hardening plan — RFC 061 (semantic palette extension: 12 new tokens completing `--{semantic}-subtle` + `--fg-on-{semantic}` triples for danger/warning/success/info × light/dark/auto-dark; closes v0.44.0 `.banner--success` regression where `--success-subtle` was used but undeclared; new CI job `semantic-palette-parity` enforces structural completeness), RFC 062 (card variants `.card--warn`, `.card--info`, `.card--success`, `.card--callout` over `.card` base; 2 inline `border-left` migrations), RFC 063 (dashboard signal/noise reorder: recent events promoted above stats with `.card--info`, sparkline demoted to h3+opacity), RFC 064 (`empty_state()` + `table_empty_row()` primitives replacing 5 ad-hoc `<p class="muted">No X yet.</p>` sites). |
| v0.45.0 | **Phase D** of the UI/UX hardening plan — RFC 058 (step-up enforcement on 4 dangerous routes: `users_set_disabled`, `clients_set_disabled`, `mfa_disable`, `passkey_delete`), RFC 059 (`<ConfirmScreen>` shared component; 5 `render_confirm_*` functions delegate to one template), RFC 060 (audit-note rollout: 7 use cases gain `reason` parameter, 8 handlers migrate to new `ConfirmedReasonForm`, self-service routes write canonical `"self"` note, reason textarea added to all 5 confirm screens). Latent bypass closed: 5 routes accepted POSTs without `_confirmed=1`; now enforced server-side. New docs page `guides/dangerous-operations.md`. |
| v0.44.0 | **Phase C** of the UI/UX hardening plan — RFC 055 (consolidate self-service onto `/me/security/*`: 9 handlers moved, `render_profile` removed, Nav "Profile" → "Security", 301 redirect for the GET endpoint, all old POST routes deleted), RFC 056 (recovery codes remaining: new `count_recovery_codes_remaining()` + i18n template replacing the hardcoded `0`), RFC 057 (language save confirmation banner via `?saved=1`), RFC 054 (aria-label sweep: 3 sites remaining after RFC 051's incidental fixes, now done). Bug fix: `.banner` CSS family was used in code but never declared — added in this release. |
| v0.43.0 | **Phase B** of the UI/UX hardening plan — RFC 051 (per-screen i18n completeness audit: 95 hardcoded JA strings → 0 across every render function in `pages.rs`; ~100 new typed Strings fields with ja/en/zh values), RFC 052 (status word + empty placeholder vocabulary unification, completing pre-existing partial work), RFC 053 (copy-button i18n contract, last call site `audit_row_view`). Bug fix: missing Chinese option on `/me/security/language`. Language self-name discipline (`locale_native_*`). RFC 048 grep widened to catch 28 additional brace-missing sites missed in v0.42.0. RFC 054 deferred to v0.44.0. |
| v0.42.0 | **Phase A** of the UI/UX hardening plan — RFC 048 (48 `t.xxx` literal-leak fixes), RFC 049 (CSS token freeze + 7 typo fixes), RFC 050 (admin chrome i18n: Nav, Footer, ThemeToggle). Plus the `/me/security/*` locale-resolution fix. Two new CI invariants (`text-leaks`, `css-tokens`). |
| v0.41.0 | RFC 040 completion (`/me/security/mfa`+`/sessions`), RFC 045 (user disable reason), RFC 046 (audit copy-ID), RFC 047 (dev summary + secret rotation) |
| v0.40.0 | RFC 040 (`/me/security` tabs initial), RFC 041 (HIBP consistency), RFC 042 (error i18n), RFC 043 (dashboard events), RFC 044 (state-word contract) |
| v0.39.0 | RFC 038 (consent screen), RFC 039 (settings i18n complete) |
| v0.38.0 | e2e coverage (RFC 030/033/035), audit-events doc, settings i18n section headers |
| v0.37.0 | RFC 029 pass 2 (dynamic locale), RFC 035 (user detail), RFC 036 (docs/Phase 5) |
| v0.36.0 | RFC 030 (dangerous ops confirm), RFC 031 (dashboard prompts), RFC 033 (audit), RFC 034 (passkey+empty) |
| v0.35.0 | RFC 032 (dev mode banner), RFC 029 first pass (admin i18n) |
| v0.34.0 | RFC 002 (i18n: zh locale, Formatters, audit labels, dir=, per-recipient locale) |
| v0.33.0 | RFC 001 (email outbox + retry worker) |
| v0.32.0 | RFC 017 (UI/UX contracts), RFC 023 (visual design system), RFC 024 (doc consolidation) |
| v0.31.0 | RFC 014 (hot-path caches), RFC 028 (copy buttons) |
| v0.30.0 | RFC 013 (async DB layer — full implementation + test fixes) |
| v0.29.13 | RFC 026 (admin logout), RFC 027 (client scope UX), dup-username bug fix |
| v0.29.12 | RFC 013 async DB layer initial |
| v0.29.10–11 | RFC 021/022 (schema invariants, boolean CHECKs, migration safety) |

Full history: [CHANGELOG.md](CHANGELOG.md)

---

## Status

**v0.60.0** completes the UX-rethink arc (RFCs 071, 072, 073) identified
in the post-MI-arc audit. All three targeted gaps are closed:

- RFC 073 (v0.58.0) — Dashboard action items
- RFC 071 (v0.59.0) — Auditor role (read-only admin access)
- RFC 072 (v0.60.0) — End-user app-access surface (`/me/apps`)

**v0.62.0** completes all verification-soak items identified during
the v0.61.0 audit (RFC 075: file-size refactor, RFC 076: configuration
reference). The project is now in **verification soak**.

The remaining open requirements for a v1.0 designation are external
to this repository:

1. **External OIDC integration verification** — run sui-id against a
   real relying party (e.g. a web app using `openid-client` or Passport).
2. **Optional second-party security review** — code review by a party
   other than the primary author.

All planned engineering work is complete. The remaining `rfcs/proposed/`
items (RFC 004, 005, 006, 008, 009, 025) are post-1.0 exploratory work;
none are scheduled.

All 16 MI RFCs across Phases 0–8 are implemented and in
`rfcs/done/`. The arc spanned v0.49.0 through v0.57.0.

Final metrics against v0.48.4 baseline:

| Metric | v0.48.4 baseline | v0.57.0 |
|---|---|---|
| `inline-style-bound` | 17 | **0** |
| MI RFCs completed | 0 | **16 / 16** |
| CSS shards | 1 monolith | **11 bounded shards** |
| Skip link | absent | **present (WCAG 2.4.1)** |
| No-JS sign-out | JS required | **server-rendered** |
| Responsive breakpoints | 768px only | **768 / 480 / 360px** |
| `inline-style-bound` ceiling | 119 (pre-v0.48.0) → 16 (v0.48.4) → 20 (MI arc limit) | **0** |

The project remains in **verification phase**. The MI arc
completion is a quality milestone, not a v1.0 gate.
**No release will start with v1 until sufficient soak, external
review, and integration verification have occurred.**

---

## Constraints and non-goals (pre-1.0)

- **Single realm.** All users share one namespace. Per-tenant isolation is
  RFC 025, post-1.0. See [docs/operators.md](docs/operators.md) §
  "User–client relationship".
- **SQLite only.** Alternative backends are RFC 009, low priority. The
  current SQLite implementation is production-grade for small deployments.
- **No user-facing theming API.** CSS tokens are for the maintainer, not
  operators.
- **No plugin system.** RFC 005 sketches one; it is not scheduled.
