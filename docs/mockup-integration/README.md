# Mockup Integration

This folder collects the planning and handoff artifacts for the
**Mockup Integration ("MI") development arc** — the controlled migration
that adopts the `sui-id-web-mockup` v0.4.8 UI/UX language into the
product codebase. The arc opens with **v0.49.0** (this release) and is
expected to span eight phases (Phase 0 through Phase 8) before
completion.

Per the migration plan, the integration is treated as a **controlled
architectural migration, not a big-bang visual replacement**. Each
phase corresponds to a small set of RFCs in `rfcs/proposed/` (named
`RFC-MI-NNN-*`); see [`rfcs/README.md`](../../rfcs/README.md) for the
phase-by-phase table.

## Contents

| File / folder | Purpose |
|---|---|
| [`migration-plan.md`](./migration-plan.md) | The revised migration plan (v0.2). Defines the eight phases, blockers, decision backlog, and non-negotiable guardrails. **Read first.** |
| [`codebase-handoff.md`](./codebase-handoff.md) | The architect-facing tour of the v0.48.4 codebase: rendering stack, design system, handler contracts, CI invariants, open questions. Generated against v0.48.4; refresh if more than two release cycles elapse before implementation starts. |
| [`mockup-handoff/`](./mockup-handoff/) | The mockup author's handoff package (HANDOFF, SCREEN_INVENTORY, FLOW_SUMMARY, OPEN_ISSUES, IMPLEMENTATION_NOTES). Describes the mockup's intent, the 35-route inventory, the five user flows, and 12 triaged open issues. |

## Reading order

1. [`migration-plan.md`](./migration-plan.md) §1–§3 — the executive
   summary and the three Phase-1 blockers (`D-01` component sharding,
   `D-02` route-based tabs, `D-03` server-rendered CSRF).
2. [`codebase-handoff.md`](./codebase-handoff.md) §3–§5 — the
   render-string pattern, the two shells, and the handler/state
   contract.
3. [`mockup-handoff/HANDOFF.md`](./mockup-handoff/HANDOFF.md) §1–§3 —
   what the mockup is and what it preserves; §14 conflict resolution
   priority.
4. [`rfcs/done/RFC-MI-000-baseline-delta-inventory.md`](../../rfcs/done/RFC-MI-000-baseline-delta-inventory.md) —
   the first RFC to implement: produces the six inventory files
   (screen map, dangerous-action map, tab-routing delta, token
   delta, i18n delta, route-render-handler map).

## Non-negotiable guardrails

These are restated in every MI RFC and remain in force throughout the
arc:

- Leptos SSR only — no hydration, no client-side framework.
- Path-based, deep-linkable tabs. Query-parameter tab state is rejected.
- Server-side CSRF validation unchanged; CSRF tokens are threaded
  through `Shell` server-side (RFC-MI-021).
- Dangerous-operation confirmation routes (`/admin/.../delete-confirm`
  GET → POST) and step-up gates preserved.
- OIDC protocol behaviour (Authorization Code + PKCE S256, exact
  redirect-URI match, RP-Initiated Logout) unchanged.
- Anti-enumeration wording preserved on `/forgot-password` and adjacent
  surfaces.
- i18n completeness (`text-leaks` = 0, three locale tables aligned).
- CI invariants (`text-leaks`, `css-tokens`, `semantic-palette-parity`,
  `inline-style-bound`) unregressed.
- No third-party CSS framework, no frontend build step beyond Cargo,
  no Wasm bundle.

## Conflict-resolution priority

When the mockup and the product implementation disagree, resolve in
this order (carried over from the migration plan §2.3 and
HANDOFF §14.3):

1. Security
2. Robustness
3. Maintainability
4. Standards compliance
5. Accessibility and usability
6. Visual preference

Differences in security, routing, CSRF, OIDC, MFA, and destructive
operations are escalated to the project manager / architect / security
reviewer; they are not silently resolved by an implementer's reading.
