# RFC-MI-040: Setup Wizard UX Integration

```toml
id = "RFC-MI-040"
title = "Setup Wizard UX Integration"
status = "Implemented (v0.53.1)"
phase = "Phase 4"
created = "2026-05-18"
implemented = "2026-05-18"
project = "sui-id"
scope = "Mockup integration into sui-id v0.48.4"
language = "English"
```

## Implementation note (added on transition to `done/`)

Implemented in **v0.53.1** — shipped after RFC-MI-041 (v0.53.0),
which the user requested first. Phase 4 is now fully closed.

### Changes made

**`StepState` enum and `SetupStep` struct** added to
`components/setup.rs`. Both are public, re-exported from
`components.rs`. `StepState::label_class()` maps each variant
to one of the three CSS classes below.

**Three new CSS classes** added to `components/setup.rs`
(`.setup-steps`, `.setup-step__label--current`, `--done`,
`--upcoming`):

- `.setup-steps` — the step indicator `<nav>` container row.
  Replaces `style="gap:…;justify-content:center;margin-bottom:…;flex-wrap:wrap;font-size:…"`.
- `.setup-step__label--current` — fg-default, medium weight
  (was `style="color:var(--fg-default);font-weight:…"`).
- `.setup-step__label--done` — fg-muted (was `style="color:var(--fg-muted)"`).
- `.setup-step__label--upcoming` — fg-subtle (was `style="color:var(--fg-subtle)"`).

**`setup_step_indicator()` in `pages/setup.rs`** refactored to
use the CSS classes instead of the two inline style= attributes:
1. `<nav class="row" style="…">` → `<nav class="setup-steps" …>`
2. `<span style=style>` → `<span class=label_cls>`

**Setup flow unchanged.** Five steps (Welcome, Admin, Language,
HIBP, Done), the same badge system, the same `aria-current="step"`
on the active entry. Setup token URL parameter model preserved.
No route contracts changed. No render function signatures changed.

### Acceptance criteria

- [x] Setup token URL-parameter model preserved (unchanged).
- [x] Setup cannot be reused after completion (unchanged; controlled by handler).
- [x] Each setup page has one clear primary action (unchanged).
- [x] No master key secret handled by HTTP UI (unchanged).
- [x] All setup text localised (unchanged; no new i18n keys).
- [x] `inline-style-bound` decreases (7 → 5 in this release).
- [x] `StepState` and `SetupStep` are public component-shard types
  available for future use by the handler layer.

---

## 1. Summary

Integrate mockup setup wizard clarity while preserving the product's setup-token and initialization safety model.

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

- Improve setup flow comprehension.
- Preserve setup token via URL parameter.
- Preserve setup-only route isolation.
- Preserve one-time initialization semantics.
- Clarify language and HIBP choices.

## 4. Non-Goals

- Do not move master-key handling into HTTP UI.
- Do not re-open setup after initialization.
- Do not expose advanced unsafe settings during initial setup unless separately approved.

## 5. Dependencies

- `RFC-MI-011`
- `RFC-MI-020`
- `RFC-MI-021`

## 6. External Design

The setup wizard should remain a short, safe, one-time path.

External flow:

```text
/setup?token=...
  → welcome
  → admin account
  → language/defaults
  → HIBP mode
  → done
  → sign in
```

If current product render functions split steps differently, preserve current
route contracts and adapt the visual step indicator rather than changing
security behavior.


## 7. Detailed Design

### Step Indicator

`setup.rs` component shard should provide a server-rendered step indicator:

```rust
pub struct SetupStep {
    pub key: &'static str,
    pub label: String,
    pub state: StepState,
}

pub enum StepState {
    Complete,
    Current,
    Upcoming,
}
```

The indicator is informational. It must not allow unsafe jumping into later
steps if the backend does not permit it.

### Setup Token

The token remains a URL parameter at entry. Do not reintroduce a text field as
the normal path.


## 8. Data / State / API Model

ABDD requirements:

- each step has one primary action
- password requirements are visible before submit
- HIBP mode choices include clear explanation
- errors are inline and summarized where appropriate
- focus moves to error summary after failed submit if implemented
- language picker is visible early and not confusing


## 9. UI/UX and ABDD Requirements

No database migration.

Potential render data:

```rust
pub struct SetupShellData {
    pub current_step: SetupStepKey,
    pub steps: Vec<SetupStep>,
    pub lang: Locale,
    pub flash: Option<Flash>,
}
```

Server settings changed by setup remain existing settings such as default
language and HIBP mode.


## 10. Migration Plan

1. Add setup step primitive.
2. Update setup welcome/admin/lang/HIBP/done renderers.
3. Preserve setup-token route behavior.
4. Add/adjust i18n keys.
5. Verify initialized installations cannot use setup pages.


## 11. Acceptance Criteria

- [ ] Setup token URL-parameter model is preserved.
- [ ] Setup cannot be reused after completion.
- [ ] Each setup page has one clear primary action.
- [ ] No master key secret is handled by HTTP UI.
- [ ] All setup text is localized.

## 12. Test Plan

- `cargo fmt --check`.
- `cargo clippy --workspace --all-targets -D warnings`.
- `cargo test --workspace`.
- `text-leaks` invariant: no literal `>t.some_key<` leaks.
- `css-tokens` invariant: every `var(--*)` reference resolves.
- `semantic-palette-parity` invariant remains green.
- `inline-style-bound` remains within the project limit.
- Integration test: uninitialized setup flow still completes.
- Integration test: initialized setup route is closed.
- Manual language switch check on setup welcome.
- Manual no-JS setup flow check.

## 13. Risks and Mitigations

- **Risk:** Step indicator implies jumping is allowed.  
  **Mitigation:** Render it as progress, not navigation, unless backend supports safe navigation.


## 15. Rollback Plan

Restore previous setup render functions. Do not alter setup state or initialization logic during rollback.
