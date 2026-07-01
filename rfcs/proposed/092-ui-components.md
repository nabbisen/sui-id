# RFC 092 — UI Components: ThemeToggle, EmptyState, CopyField, Error Summary

**Status.** Proposed
**Tracks.** UI/UX handoff v2.3 §5 (ThemeToggle, EmptyState, CopyField,
Error Summary) — unit 6. Category B.
**Touches.** `sui-id-web/src/components/` (new components), `static/theme-init.js`
(root-class swap logic), `sui-id-web/src/components/chrome.rs` (shell
integration), all page renderers that have empty-state lists, i18n.

## Summary

Four UI component additions that complete the v2.3 component contract:

1. **ThemeToggle** — `no-js`/`js` root-class swap owned by `theme-init.js`
   (already loaded in `<head>`), `localStorage` try/catch robustness,
   `<noscript>` fallback copy.
2. **EmptyState** — consistent empty-list presentation for users, clients,
   signing keys, and audit-log results.
3. **CopyField** — `readonly` + `role="status"` field with a copy-to-
   clipboard button; used for client secrets, client IDs, and signing key IDs.
4. **Error summary** — accessible `role="alert"` summary block at the top
   of forms that have multiple field errors.

## Motivation

### ThemeToggle

v2.3 Appendix E:
> - `<html>` initially ships with `class="no-js"`.
> - `theme-init.js` is loaded in `<head>` before first paint (no `defer`/`async`).
> - No inline script is used for the root-class swap.
> - `localStorage` errors do not break page rendering.
> - No-JS users see "Theme follows your system setting." rather than a dead button.

The current implementation does not have `class="no-js"` on `<html>`, and
the ThemeToggle renders without a no-JS fallback. The `theme-init.js` file
exists but the `localStorage` try/catch robustness was not verified.

### EmptyState

Currently, empty admin lists (no users, no clients, no signing keys) render
an empty `<table>` body or nothing at all. v2.3 §5 specifies an `EmptyState`
component with a consistent message + CTA link for each surface.

### CopyField

Client secrets are displayed as plain text after rotation; there is no copy
button. v2.3 §5 specifies `CopyField` with `readonly` + `role="status"` for
one-time values. The `copy.js` static file already exists (registered in the
shell); this RFC adds the HTML component wrapper.

### Error summary

Forms with field-level validation errors do not currently aggregate errors
at the top of the form. v2.3 §5 specifies an accessible error summary
`role="alert"` block listing each error when multiple fields fail, and
`aria-invalid` on individual fields.

## Target code areas

### ThemeToggle

1. **`static/theme-init.js`** — add `try/catch` around `localStorage`
   access. On any exception, fall back to `prefers-color-scheme` / CSS
   default. Add `document.documentElement.classList.replace('no-js', 'js')`
   at the top of the script.

2. **`sui-id-web/src/components/chrome.rs`** (or the shell template) — add
   `class="no-js"` to the `<html>` element emitted by the shell. The shell
   currently emits `<html lang=…>` without the class.

3. **`sui-id-web/src/components/theme_toggle.rs`** (new file, or extend
   `components.rs`) — render the `ThemeToggle` button wrapped in
   `.js .theme-toggle` visibility rules, and the `<noscript>` text inside
   `.no-js .theme-no-js-note`.

4. **CSS tokens** — add `.no-js .theme-toggle { display:none }` and
   `.js .theme-no-js-note { display:none }` to the token/component CSS.
   These rules do not add to the `semantic-parity` count; they are
   structural visibility rules.

### EmptyState

New component function in `sui-id-web/src/components/` (file placement
TBD; if `components.rs` is near 500 ELOC a new `empty_state.rs` file):

```rust
pub fn empty_state(t: &Strings, message: &str, cta: Option<(&str, &str)>) -> impl IntoView
// message: i18n key value
// cta: Option<(link_href, button_label)>
```

Per-list i18n keys:
- `empty_users` / `empty_users_cta`
- `empty_clients` / `empty_clients_cta`
- `empty_signing_keys` (no CTA — add via CLI or the rotate button)
- `empty_audit` (no CTA)

List pages that currently render an empty `<tbody>` branch to
`empty_state(...)` when the result set is empty.

### CopyField

New component function (extend the existing `copy_btn` helper or add a
`copy_field` wrapper):

```rust
pub fn copy_field(t: &Strings, value: &str, label: &str) -> impl IntoView
// Renders: <div class="copy-field">
//   <input readonly value="{value}" aria-label="{label}" role="status">
//   <button … onclick="copy_to_clipboard(…)">Copy</button>
// </div>
```

Used on: client-edit page (client ID, freshly-rotated secret), signing-key
list (key ID).

### Error summary

Update `sui-id-web/src/components/forms.rs` to add:

```rust
pub fn error_summary(t: &Strings, errors: &[&str]) -> impl IntoView
// Renders: <div role="alert" class="error-summary" aria-live="assertive">
//   <p>t.error_summary_heading</p>
//   <ul>
//     <li>…</li>
//   </ul>
// </div>
// Only rendered when errors.len() > 1.
```

Individual field errors continue to render inline via the existing
`field_error` component. The summary appears at the top of the form for
the user when multiple errors exist.

## Security properties / invariants

- **P1 (ThemeToggle storage robustness).** `localStorage` errors never
  throw to page-load-blocking scope. Errors are caught and the fallback
  path uses `prefers-color-scheme`.
- **P2 (no inline script).** The root-class swap runs from `theme-init.js`,
  not an inline `<script>` tag. The existing "three static scripts only"
  rule and CSP remain unchanged.
- **P3 (no-JS users get accessible fallback).** No-JS users see a text
  note ("Theme follows your system setting.") rather than a non-functional
  button.

## Non-goals

- No change to theme persistence logic beyond the `try/catch` robustness.
- No change to authentication or authorization.
- The `role="status"` on `CopyField` inputs is for AT announcement of the
  value; it does not change the security boundary.

## Data model impact

None.

## API impact

None.

## Testing strategy

- CI: `inline-style-bound` gate continues to pass (no inline style in new
  components).
- CI: `text-leaks` gate continues to pass (new i18n keys for empty states).
- Manual: disable JavaScript → `<html class="no-js">` → theme button hidden,
  `<noscript>` note visible.
- Manual: `localStorage.setItem` throws (private mode) → page still renders.
- Unit: `empty_state` renders the CTA link when provided, hides it when not.

## Migration strategy

None. All changes are additive rendering.

## Rollout plan

Ships as v0.74.0. Last unit of the v2.3 UI-security contract.

## Risks and mitigations

- *Risk:* adding `class="no-js"` to `<html>` causes a flash of
  unstyled / hidden content until `theme-init.js` runs. Mitigation: the
  script is loaded blocking in `<head>` (per Appendix E), so the class swap
  happens before first paint; there is no layout shift or content flash.
- *Risk:* error summary with `role="alert"` fires on every page load (not
  just on errors). Mitigation: the error summary is only rendered when
  `errors.len() > 1`; on clean page loads it is not present in the DOM.
- *Risk:* `components.rs` ELOC exceeds 500 with the new components.
  Mitigation: split into `copy_field.rs`, `empty_state.rs` sub-modules
  as needed; the 2018-style module convention (`no mod.rs`) supports this.

## Acceptance criteria

All items from Appendix E §"ThemeToggle" are met. Empty lists on users,
clients, signing keys, and audit surfaces render `EmptyState`. Client ID,
freshly-rotated client secret, and signing key ID render via `CopyField`
with `readonly` and copy button. Forms with multiple field errors show
`role="alert"` summary at top. CI gates: text-leaks = 0, inline-style = 0.

## Open questions

- Should `CopyField` use the existing `copy.js` `copy_to_clipboard` function
  or a new `copyField()` helper? Recommendation: extend `copy.js` with a
  minimal `copyField(id)` function (reads `.copy-field input[readonly]` and
  copies value). No new static file.
- Should the `EmptyState` CTA link appear only for admins (not auditors)?
  Yes — the CTA links to the "new user" / "new client" form; auditors should
  not see it. Pass `can_write: bool` to `empty_state(…)` and omit the CTA
  when `!can_write`.
