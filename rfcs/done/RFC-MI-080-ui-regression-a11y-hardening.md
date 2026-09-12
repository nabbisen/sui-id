# RFC-MI-080: UI Regression and Accessibility Hardening

```toml
id = "RFC-MI-080"
title = "UI Regression and Accessibility Hardening"
status = "Implemented (v0.57.0)"
phase = "Phase 8"
created = "2026-05-18"
implemented = "2026-05-18"
project = "sui-id"
scope = "Mockup integration into sui-id v0.48.4"
language = "English"
```

**Companion artifacts retired (2026-09-12).** The verification matrices
under `docs/src/mockup-integration/` that this RFC describes were retired
to version-control history under RFC 098 step 5 (one topic, one document:
a completed arc's working papers are not product documentation). They are
not deleted, only unpublished — recover any of them with
`git show 600d184:<path>`, `600d184` being the last commit that carried
them. Their paths are named in the text below and are no longer links. The
RFC's own record is not rewritten; this note is the change.

## Implementation note (added on transition to `done/`)

Implemented in **v0.57.0** — the final release in the MI arc.

### Blocker fixes

**Skip link added** (WCAG 2.4.1 Level A) — `<a class="skip-link"
href="#main-content">` added as the first focusable element in both
`Shell` and `AuthShell`. The target `<main id="main-content">` was
added to both layouts. CSS `.skip-link` rule added to
`chrome.rs::CHROME_RESPONSIVE_CSS` (off-screen until focused, then
slides into view at `top: var(--space-2)`). Localised via new
`a11y_skip_to_main` i18n key (en: "Skip to main content",
ja: "メインコンテンツへスキップ", zh: "跳转到主要内容").

`role="banner"` added to `<header class="app-header">` in both
Shell and AuthShell for explicit landmark declaration.

**Narrow breakpoints added** (WCAG 1.4.10 reflow):
- `@media (max-width: 480px)` — auth-card full-bleed, smaller
  stat values, tighter route-tab padding, `.danger-zone` padding.
- `@media (max-width: 360px)` — `.form-actions` stacks vertically;
  `.grid-cards` collapses to single column.

### Verification matrices committed

Six documents under `docs/src/mockup-integration/`:

- `accessibility-matrix.md` — per-screen ABDD verification (all pass)
- `no-js-matrix.md` — no-JS operation coverage (all core flows pass)
- `keyboard-navigation-matrix.md` — keyboard reachability (all pass)
- `responsive-matrix.md` — 768px / 480px / 360px (all pass)
- `i18n-copy-review.md` — localisation audit (0 leaks, all keys present)
- `security-sensitive-copy-review.md` — anti-enumeration, OIDC consent,
  confirmation accuracy (all pass)

### Acceptance criteria

- [x] All migration RFC acceptance criteria rechecked (no new failures found).
- [x] Keyboard matrix has no blocker items.
- [x] No-JS matrix has no blocker items for core flows.
- [x] Responsive matrix covers 768px, 480px, and 360px.
- [x] Security-sensitive copy review is complete.
- [x] CHANGELOG and ROADMAP are updated.

---

## 1. Summary

Run final hardening for migrated UI surfaces against accessibility, mobile, no-JS, i18n, and security regression requirements.

## 2. Background

The mockup integration must be treated as a controlled architectural migration,
not as a direct visual replacement. The current product is already a working
Rust / Axum / Leptos SSR service with security-sensitive identity flows.
The mockup provides UI/UX intent: information hierarchy, screen relationships,
ABDD behavior, visual language, and operational clarity.

This RFC preserves the following project-level constraints:

- Leptos SSR only.
- No hydration dependency.
- No third-party CSS framework.
- Preserve public `render_*` entry points unless this RFC explicitly changes them.
- Preserve handler-side owned `*Data` structs.
- Preserve i18n table discipline.
- Preserve CSRF, step-up, confirmation, audit, and anti-enumeration contracts.
- Preserve CI gates for text leaks, CSS tokens, semantic palette parity, and inline-style bounds.

## 3. Goals

- Verify all migrated screens together.
- Create keyboard and no-JS test matrices.
- Catch i18n and copy regressions.
- Catch mobile layout regressions.
- Confirm destructive and authentication flows remain secure.

## 4. Non-Goals

- Do not introduce new feature work.
- Do not change UI architecture unless required to fix regressions.
- Do not defer critical security or accessibility regressions.

## 5. Dependencies

- `RFC-MI-030`
- `RFC-MI-031`
- `RFC-MI-040`
- `RFC-MI-041`
- `RFC-MI-051`
- `RFC-MI-060`
- `RFC-MI-070`

## 6. External Design

This is the final stabilization RFC for the mockup integration arc.

It does not add new UI concepts. It verifies that the previous RFCs compose into
a safe, accessible, maintainable whole.


## 7. Detailed Design

### Required Matrices

Create:

```text
docs/src/mockup-integration/
├── accessibility-matrix.md
├── no-js-matrix.md
├── keyboard-navigation-matrix.md
├── responsive-matrix.md
├── i18n-copy-review.md
└── security-sensitive-copy-review.md
```

Each migrated screen must be listed with pass/fail/notes.

### Minimum Screen Groups

- setup
- login/auth/MFA/reset/step-up
- dashboard
- users
- clients
- settings
- audit
- self-service security
- OIDC consent
- confirmations
- error pages


## 8. Data / State / API Model

ABDD acceptance focus:

- keyboard-only completion
- screen-reader announcement readiness
- visible focus
- semantic landmarks
- non-color-only status
- mobile readability
- reduced-motion respect
- no-JS operation for core flows


## 9. UI/UX and ABDD Requirements

No database migration.

No new persistent state.

Test-only additions are allowed, such as fixtures or helper assertions for:

- CSRF fields
- route tabs
- labels
- `aria-current`
- `aria-invalid`
- copy buttons


## 10. Migration Plan

1. Build matrices from implemented RFCs.
2. Run automated checks.
3. Perform manual keyboard/no-JS/mobile review.
4. Fix blockers.
5. Update CHANGELOG and ROADMAP.
6. Mark implemented RFCs according to lifecycle policy.


## 11. Acceptance Criteria

- [ ] All migration RFC acceptance criteria are rechecked.
- [ ] Keyboard matrix has no blocker items.
- [ ] No-JS matrix has no blocker items for core flows.
- [ ] Responsive matrix covers 768px, 480px, and 360px.
- [ ] Security-sensitive copy review is complete.
- [ ] CHANGELOG and ROADMAP are updated.

## 12. Test Plan

- `cargo fmt --check`.
- `cargo clippy --workspace --all-targets -D warnings`.
- `cargo test --workspace`.
- `text-leaks` invariant: no literal `>t.some_key<` leaks.
- `css-tokens` invariant: every `var(--*)` reference resolves.
- `semantic-palette-parity` invariant remains green.
- `inline-style-bound` remains within the project limit.
- Manual no-JS test for sign-out, login, setup, and destructive confirmations.
- Manual keyboard path test for every migrated screen group.
- Mobile viewport review at 768px, 480px, and 360px.
- Review security-sensitive wording for enumeration and protocol leakage.

## 13. Risks and Mitigations

- **Risk:** Late review discovers architectural issue.  
  **Mitigation:** Earlier RFCs include explicit acceptance gates; this RFC is a hardening pass, not first review.

- **Risk:** Accessibility findings are treated as polish.  
  **Mitigation:** ABDD is a core product constraint; blocker findings must be fixed before completion.


## 15. Rollback Plan

Rollback should be targeted to the RFC that caused the regression. Full rollback of the integration arc should be avoided unless security is compromised.
