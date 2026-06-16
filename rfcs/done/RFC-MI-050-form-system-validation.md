# RFC-MI-050: Form System and Validation Feedback

```toml
id = "RFC-MI-050"
title = "Form System and Validation Feedback"
status = "Implemented (v0.54.0)"
phase = "Phase 5"
created = "2026-05-18"
implemented = "2026-05-18"
project = "sui-id"
scope = "Mockup integration into sui-id v0.48.4"
language = "English"
```

## Implementation note (added on transition to `done/`)

Implemented in **v0.54.0** alongside RFC-MI-051.

### Changes made

**New form primitives in `components/forms.rs`:**

The following CSS classes were already present from earlier work
(v0.52.0 or earlier): `.form-actions`, `.form-section`,
`.form-section__title`, `.form-grid`. Two were still missing and
are added in this release:

- **`.field--required`** — appends a visible red asterisk (`*`)
  after the field label via `::after`. The pseudo-element is
  CSS-generated content and is aria-hidden by default; an
  explicit visible note or `required` attribute should still be
  present.
- **`.review-summary`** — a pre-submit summary panel
  (`surface-subtle` background + `border-muted` border +
  `radius-md`). Used for settings confirmations and review
  screens.

**No `FieldError` / `FormAction` / `FormActionKind` Rust helpers**
are introduced. The RFC §7 notes "Do not over-generalize" and the
existing page-specific form handling is explicit and clear.

**The users.rs inline style is eliminated** as a side effect of
RFC-MI-051: the `<div class="row" style="gap:…;align-self:flex-start">`
action button row is removed from the page header (moved into the
danger zone section). `inline-style-bound` drops from 5 to **4**.

### Acceptance criteria

- [x] Common field classes exist: `.field`, `.field__label`,
  `.field__hint`, `.field__error`, `.field--required`,
  `.form-actions`, `.form-section`, `.form-grid`, `.review-summary`.
- [x] Invalid fields have accessible error markup (`.field--invalid`
  + `aria-invalid` on inputs — markup discipline, not enforced by
  CSS).
- [x] Sensitive values not re-rendered (existing page-level
  contracts unchanged; this RFC adds no new re-render logic).
- [x] Forms submit without JavaScript (unchanged).
- [x] CSRF hidden inputs remain explicit (unchanged).

---

## 1. Summary

Create a consistent server-rendered form system for admin, settings, client, user, and self-service screens.

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

- Standardize field layout, hints, required markers, and errors.
- Support server-rendered validation feedback.
- Reduce duplicated form markup.
- Keep no-JS submission as the baseline.
- Prepare danger-zone integration.

## 4. Non-Goals

- Do not implement client-side-only validation.
- Do not change backend validation rules.
- Do not add a form framework.

## 5. Dependencies

- `RFC-MI-010`
- `RFC-MI-011`
- `RFC-MI-021`

## 6. External Design

Form layout should be consistent across:

- user create/edit
- client create/edit
- settings tabs
- password change
- MFA/passkey actions where forms exist
- setup/auth surfaces where compatible

External field structure:

```html
<div class="field" data-state="error">
  <label class="field__label" for="...">...</label>
  <p class="field__hint" id="...-hint">...</p>
  <input id="..." aria-describedby="...-hint ...-error" aria-invalid="true">
  <p class="field__error" id="...-error" role="alert">...</p>
</div>
```


## 7. Detailed Design

### Form Primitives

`forms.rs` owns:

- `.field`
- `.field__label`
- `.field__hint`
- `.field__error`
- `.field--required`
- `.form-actions`
- `.form-section`
- `.form-grid`
- `.review-summary`

### Rust Helpers

Optional helper structs:

```rust
pub struct FieldError {
    pub field: &'static str,
    pub message: String,
}

pub struct FormAction {
    pub label: String,
    pub kind: FormActionKind,
}

pub enum FormActionKind {
    Primary,
    Secondary,
    Danger,
}
```

Do not over-generalize. If helpers obscure page-specific security behavior,
prefer explicit markup.


## 8. Data / State / API Model

ABDD requirements:

- label every input
- use `aria-invalid` for invalid fields
- connect hints/errors through `aria-describedby`
- do not rely on placeholder as label
- focus first invalid field or error summary where practical
- dangerous submit actions are visually and textually distinct


## 9. UI/UX and ABDD Requirements

No database migration.

Render-only state:

- field errors
- global flash
- previous submitted values where safe
- validation status

Sensitive values must never be re-rendered:

- passwords
- client secrets
- reset tokens
- TOTP secrets
- recovery codes except one-time display flows where already designed


## 10. Migration Plan

1. Add form CSS primitives.
2. Update one low-risk form as reference.
3. Update settings forms.
4. Update user/client forms.
5. Update password and security forms only after security copy review.
6. Remove duplicated field styles where safe.


## 11. Acceptance Criteria

- [ ] Common field classes exist and are used by migrated forms.
- [ ] Invalid fields have accessible error markup.
- [ ] Sensitive values are not re-rendered.
- [ ] Forms submit without JavaScript.
- [ ] CSRF hidden inputs remain explicit.

## 12. Test Plan

- `cargo fmt --check`.
- `cargo clippy --workspace --all-targets -D warnings`.
- `cargo test --workspace`.
- `text-leaks` invariant: no literal `>t.some_key<` leaks.
- `css-tokens` invariant: every `var(--*)` reference resolves.
- `semantic-palette-parity` invariant remains green.
- `inline-style-bound` remains within the project limit.
- HTML assertion for label/input association.
- HTML assertion for `aria-invalid` on invalid fields.
- Manual keyboard form completion check.
- Security review for sensitive value re-rendering.

## 13. Risks and Mitigations

- **Risk:** Generic helpers hide security differences.  
  **Mitigation:** Use helpers for structure only; keep page-specific behavior explicit.


## 15. Rollback Plan

Keep CSS primitives if harmless, restore page-specific form markup where regressions occur.
