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

v0.48.0 ships Phase F's final buffer: `handlers/me_security.rs`
1099 → 7 sub-modules (RFC 068), inline-style discipline 119 → 16
plus a CI bound of 20 (RFC 067), and a cleanup of five pre-existing
warnings carried over from earlier releases.

**Phase F closes here.** The three files originally in the Phase F
mandate — `pages.rs` (4170), `handlers/admin.rs` (1531), and
`handlers/me_security.rs` (1099) — are all split into per-screen /
per-domain submodules under 500 LOC. Inline `style=""` count is
bounded by CI. The workspace compiles with 0 warnings.

Ten other `.rs` files (i18n string tables, sui-id-core state
machines, `backup.rs`, `handlers/oidc.rs`) are still over the
500-LOC *recommendation*. They were not in Phase F scope; some are
single-bag string tables where splitting harms cohesion, others
are state-machine implementations tracked as separate post-1.0
candidates.

The project enters **verification phase**. A v1.0 candidate
designation (rc, pre, beta, anything) is not on the immediate
horizon — sufficient soak, external review, and an integration
verification pass come first. The next planned release is a
verification-pass buffer; its tag is TBD and **will not start
with v1**.

The v0.42 → v0.48 hardening arc is complete: 21 RFCs (048–068)
landed across 7 releases, addressing every gap surfaced by the PDF
review.

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
