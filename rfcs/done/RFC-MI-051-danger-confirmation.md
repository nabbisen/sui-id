# RFC-MI-051: Danger Zone and Confirmation Screen Integration

```toml
id = "RFC-MI-051"
title = "Danger Zone and Confirmation Screen Integration"
status = "Implemented (v0.54.0)"
phase = "Phase 5"
created = "2026-05-18"
implemented = "2026-05-18"
project = "sui-id"
scope = "Mockup integration into sui-id v0.48.4"
language = "English"
```

## Implementation note (added on transition to `done/`)

Implemented in **v0.54.0** alongside RFC-MI-050.

### Changes made

**`components/confirm.rs`** — The `.danger-zone` and
`.impact-summary` CSS families were already present in this shard
from an earlier preparatory commit. The shard docstring was updated
to reference RFC-MI-051 (v0.54.0) as the formal implementation.

**`pages/users.rs`** — Restructured so destructive operations no
longer appear in the page header:

- The `<div class="row" style="…">` action button row is **removed
  from the page header** (this also eliminates the last non-oidc
  inline style; `inline-style-bound` drops 5 → 4).
- A new `<section class="danger-zone">` is appended **after all
  read surfaces** (auth info, sessions, activity). The section
  contains:
  - `<h2 class="danger-zone__title">⚠ {t.danger_zone_title}</h2>`
  - `<p class="danger-zone__body">{t.user_detail_danger_zone_body}</p>`
  - Action buttons: Reset MFA (if enrolled), Disable/Enable, Delete
  - Buttons wrapped in `<div class="form-actions">` (no inline style)

**New i18n key `user_detail_danger_zone_body`** added to `Strings`,
`en.rs`, `ja.rs`, `zh.rs`. The existing `danger_zone_title` key
is reused.

**Confirmation routes unchanged.** Every action link still points
to its dedicated GET confirmation route:
`/admin/users/{id}/delete-confirm`,
`/admin/users/{id}/disable-confirm`,
`/admin/users/{uid}/mfa-reset-confirm`. These routes require
a CSRF-protected POST; the confirmation page collects a reason;
audit logging fires on the POST.

**Clients and signing keys** — these detail pages have no inline
styles and already link to their confirmation routes; no structural
change is needed in this release. A future RFC may apply the same
danger-zone restructuring for visual consistency.

**Impact summaries on confirm pages** — deferred. The confirm
screens already describe the target and action clearly. Adding
structured `ConfirmImpactItem` lists would require handler-side
changes to compute and pass them; deferred to a maintenance RFC.

### Acceptance criteria

- [x] No destructive action is inline-only (all actions lead to a
  confirmation route with CSRF).
- [x] Existing confirmation routes remain (unchanged).
- [x] CSRF is present on all destructive POSTs (unchanged; the
  confirmation POST forms carry `_csrf`).
- [x] Step-up requirements preserved (unchanged; the confirmation
  handler enforces step-up).
- [x] Audit event behavior preserved (unchanged).
- [x] Danger meaning accessible without color (the `.danger-zone`
  section uses a text heading ("⚠ Danger Zone") and a body
  paragraph; the color is redundant emphasis only).

---

## 1. Summary

Adopt the mockup's danger-zone visual model while preserving product-specific confirmation routes, CSRF, step-up, and audit behavior.

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

- Physically and semantically isolate destructive operations.
- Preserve existing `render_confirm_*` routes.
- Preserve step-up requirements.
- Preserve CSRF enforcement.
- Introduce impact summaries for destructive operations.

## 4. Non-Goals

- Do not introduce generic `/confirm/{token}` routing.
- Do not replace confirmation GET pages with inline-only prompts.
- Do not weaken audit logging.

## 5. Dependencies

- `RFC-MI-050`
- `RFC-MI-021`

## 6. External Design

External detail page structure:

```text
Detail Page
├── Read surface
├── Safe settings/actions
└── Danger Zone
    ├── explanation
    ├── operation-specific impact summary
    └── link/button to confirmation route
```

Confirmation pages remain product-specific, for example:

- disable user
- delete user
- reset MFA
- delete client
- delete signing key


## 7. Detailed Design

### ConfirmScreenData

Extend only if needed:

```rust
pub struct ConfirmImpactItem {
    pub label: String,
    pub value: String,
    pub tone: SurfaceTone,
}

pub struct ConfirmScreenData {
    // existing fields
    pub impact: Vec<ConfirmImpactItem>,
    pub irreversible: bool,
}
```

If existing `ConfirmScreenData` already supports this through generic fields,
do not add new fields.

### Danger Zone CSS

`confirm.rs` owns:

- `.danger-zone`
- `.danger-zone__title`
- `.danger-zone__body`
- `.impact-summary`
- `.impact-summary__item`


## 8. Data / State / API Model

ABDD requirements:

- danger meaning must be text and structure, not only red color
- confirmation page must identify the target object
- irreversible consequences must be clear
- cancel path must be visible and keyboard reachable
- focus order must reach safe cancel and final danger action predictably


## 9. UI/UX and ABDD Requirements

No database migration.

No new confirmation persistence.

All confirmation POSTs must continue to include:

- explicit action route
- CSRF field
- operation-specific target identifier
- existing audit event behavior


## 10. Migration Plan

1. Add danger-zone and impact-summary primitives.
2. Update user/client/signing-key detail pages to use danger zone.
3. Update existing confirmation renderers with impact summaries.
4. Confirm CSRF and step-up behavior is unchanged.
5. Security-review all destructive action copy.


## 11. Acceptance Criteria

- [ ] No destructive action is inline-only.
- [ ] Existing product confirmation routes remain.
- [ ] CSRF is present on destructive POSTs.
- [ ] Step-up requirements are preserved.
- [ ] Audit event expectations are preserved.
- [ ] Danger meaning is accessible without color.

## 12. Test Plan

- `cargo fmt --check`.
- `cargo clippy --workspace --all-targets -D warnings`.
- `cargo test --workspace`.
- `text-leaks` invariant: no literal `>t.some_key<` leaks.
- `css-tokens` invariant: every `var(--*)` reference resolves.
- `semantic-palette-parity` invariant remains green.
- `inline-style-bound` remains within the project limit.
- Integration test for destructive route still requiring CSRF.
- Integration test for confirmation GET before destructive POST where applicable.
- Manual keyboard check for cancel and danger submit order.

## 13. Risks and Mitigations

- **Risk:** Mockup generic confirm route leaks into product.  
  **Mitigation:** Explicitly reject `/confirm/{token}` in this RFC.


## 15. Rollback Plan

Restore previous confirmation page markup. Do not roll back security route behavior unless separately approved.
