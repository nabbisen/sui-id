# Roadmap

This file is a loose sketch of direction — nothing here is a promise.
Completed work is tracked in [CHANGELOG.md](CHANGELOG.md) and the
[`rfcs/done/`](rfcs/done/) directory.

---

## Active proposals (proposed RFCs)

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

## Completed (recent)

| Version | What shipped |
|---|---|
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

v0.48.2 ships the **second verification-phase release**: six
non-lock-out issues from the same real-environment round that
produced the v0.48.1 hotfix. Text-selection colour, i18n
hardcoding on the security overview, the setup wizard language
picker, footer accessibility badge design intent, tagline
restraint, and the first `@media` breakpoint for mobile
responsive layout.

All are UX regressions or latent bugs that real testing surfaced.
None required changes to data structures, auth flows, or the
OIDC stack; all are CSS, i18n, and light handler/page code.

The project remains in **verification phase**. Three known
follow-up items are tracked in the v0.48.2 CHANGELOG (`.cell-wrap`
per-table annotations, `?return=` on login redirect, CSRF
server-render); they are not blocking and will land in v0.48.3+.
A v1.0 designation continues to be deferred until sufficient
soak and external review. **No release will start with v1
until that bar is met.**

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
