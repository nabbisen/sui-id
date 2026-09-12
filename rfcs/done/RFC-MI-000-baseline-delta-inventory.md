# RFC-MI-000: Baseline Delta Inventory and Integration Mapping Contract

```toml
id = "RFC-MI-000"
title = "Baseline Delta Inventory and Integration Mapping Contract"
status = "Implemented (v0.49.1)"
phase = "Phase 0"
created = "2026-05-18"
implemented = "2026-05-18"
project = "sui-id"
scope = "Mockup integration into sui-id v0.48.4"
language = "English"
```

**Companion artifacts retired (2026-09-12).** The six inventory files
under `docs/mockup-integration/inventory/` that this RFC describes were
retired to version-control history under RFC 098 step 5 (one topic, one
document: a completed arc's working papers are not product documentation).
They are not deleted, only unpublished — recover any of them with
`git show 600d184:<path>`, `600d184` being the last commit that carried
them. Their paths are named in the text below and are no longer links. The
RFC's own record is not rewritten; this note is the change.

## Implementation note (added on transition to `done/`)

Implemented in **v0.49.1**. The six required inventory files were
produced under `docs/mockup-integration/inventory/`
rather than the speculative location `rfcs/proposed/mockup-integration-inventory/`
mentioned in §6 below. The new location keeps non-RFC planning
artifacts in `docs/` (alongside the migration plan and the codebase
handoff) and out of the `rfcs/` namespace, whose folders are
reserved for RFC-state tracking by the lifecycle policy.

Inventory files shipped:

- `docs/mockup-integration/inventory/screen-map.md`
- `docs/mockup-integration/inventory/dangerous-action-map.md`
- `docs/mockup-integration/inventory/tab-routing-delta.md`
- `docs/mockup-integration/inventory/token-delta-draft.md`
- `docs/mockup-integration/inventory/i18n-copy-delta-draft.md`
- `docs/mockup-integration/inventory/route-render-handler-map.md`
- `docs/mockup-integration/inventory/README.md` (index)

All twelve acceptance criteria in §11 below were met. Twenty-one
decisions were surfaced (5 screen-level, 6 dangerous-action, 5
token, 5 i18n) — each with a recommended default. None blocks
Phase 1.

The headline structural findings:

- The mockup token vocabulary is a **strict subset** of the
  product's (33 ⊂ 75 tokens) — zero new tokens.
- The mockup's 382-key i18n delta is **mostly renames**; net new
  translation work is ~58 keys × 3 locales ≈ 174 entries.
- The mockup's 18 dangerous-action values reduce to **9 link
  rewrites + 5 do-not-implement + 3 step-up-policy-deltas + 1
  inline-only**.
- The mockup's `?tab=` model is rewritten to the product's path
  model with a mechanical anchor-substitution table.
- **No mockup intent crosses the web-crate boundary**; protocol
  layers (`sui-id-core`, OIDC engine, audit chain) are untouched.

---

## 1. Summary

Define the pre-implementation inventory artifacts that make the mockup integration auditable before code changes begin.

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

- Create a screen-to-render mapping table.
- Create a dangerous-action mapping table.
- Create a tab-routing delta table.
- Create token and i18n delta drafts.
- Classify each mockup feature as ready, needs clarification, requires extension, or do not implement yet.

## 4. Non-Goals

- Do not modify runtime code.
- Do not decide visual implementation details that belong to later RFCs.
- Do not introduce new routes, tokens, strings, or components.

## 5. Dependencies

- None

## 6. External Design

This RFC establishes the integration map that later RFCs must follow.

The output should be a small document set under `rfcs/proposed/mockup-integration-inventory/`
or an equivalent planning directory until the first implementation RFC lands.

Required inventory files:

```text
mockup-integration-inventory/
├── screen-map.md
├── dangerous-action-map.md
├── tab-routing-delta.md
├── token-delta-draft.md
├── i18n-copy-delta-draft.md
└── route-render-handler-map.md
```

Each inventory file must be written in English Markdown and must be usable by
implementation engineers without re-reading the mockup source archive.


## 7. Detailed Design

### Screen Map

Minimum columns:

| Mockup screen | Product route | Current render function | Handler | Shell | Auth requirement | CSRF | Status |
|---|---|---|---|---|---|---|---|

Status values:

- `ready-to-integrate`
- `needs-visual-adaptation`
- `requires-handler-change`
- `requires-backend-review`
- `do-not-implement-yet`

### Dangerous-Action Map

Minimum columns:

| Mockup action | Product action | Confirmation route | `render_confirm_*` | Step-up | CSRF | Audit event | Decision |
|---|---|---|---|---|---|---|---|

### Tab Routing Delta

The mockup may express tabs as query parameters. The product must preserve
path-based deep links. This table records every affected tab.

### Token Delta Draft

Classify every mockup token as:

- mapped to existing token
- mapped to existing utility
- requires new token
- rejected

### i18n Copy Delta Draft

Every new visible string must have a proposed key, English text, Japanese text
placeholder, Chinese text placeholder, and security-review flag.


## 8. Data / State / API Model

The inventory must preserve the user's cognitive model:

- one screen has one primary responsibility
- destructive operations are isolated
- setup, login, OIDC, admin, and self-service streams are not mixed
- all unresolved mismatches are surfaced rather than hidden

ABDD review markers must be included in the screen map:

- semantic structure impact
- keyboard path impact
- screen-reader feedback impact
- color-only risk


## 9. UI/UX and ABDD Requirements

No database migration is allowed.

This RFC introduces only planning artifacts. It does not add Rust structs,
routes, handlers, or persistent state.

The inventory should reference existing Rust names where known, for example:

- `render_dashboard`
- `render_users`
- `render_clients`
- `render_settings_*`
- `render_me_*`
- `render_confirm_*`
- `Shell`
- `AuthShell`


## 10. Migration Plan

1. Extract mockup screen list from the handoff package.
2. Extract current product render surface from `crates/sui-id-web`.
3. Map each screen to the nearest current render function.
4. Mark mismatches that require RFC-level decisions.
5. Review the maps with the project manager and architect.
6. Freeze the maps as the baseline for RFC-MI-010 and later RFCs.


## 11. Acceptance Criteria

- [ ] Inventory files exist and are reviewed.
- [ ] No runtime code has changed.
- [ ] Every mockup screen has an explicit status.
- [ ] Every destructive mockup action has a product-safe route decision.
- [ ] Every tab mismatch is visible before tab component design begins.
- [ ] Token and i18n deltas are quantified before visual adoption.

## 12. Test Plan

- Manual review of inventory completeness.
- Cross-check every current public `render_*` function against the screen map.
- Cross-check every mockup route against the screen map.
- Cross-check every destructive mockup action against the dangerous-action map.

## 13. Risks and Mitigations

- **Risk:** The inventory becomes stale if implementation starts late.  
  **Mitigation:** Refresh inventory if more than two release cycles pass.

- **Risk:** The team treats mockup routes as authoritative.  
  **Mitigation:** The route-render-handler map must always show product ownership.


## 15. Rollback Plan

Delete the inventory directory. No product code is affected.
