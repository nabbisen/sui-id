# sui-id RFCs

Design notes for sui-id features and policies. Each RFC scopes
one piece of work in enough detail that an implementer can start
without a second design pass — but no more than that.

These are not blanket commitments. The [ROADMAP](../ROADMAP.md)
sets which of these will actually ship and in what order. An RFC
landing here means the design is settled enough to write code
from; not landing here means the design is still soft.

## How this directory works

The lifecycle is governed by
[RFC 018 — RFC lifecycle policy](./done/018-rfc-lifecycle-policy.md).
Briefly:

- **`proposed/`** — open for review and discussion. Implementer
  should not yet start work; the design may change.
- **`done/`** — implemented and shipped. The RFC is now a
  historical record of the design decisions.
- **`archive/`** — withdrawn or superseded. Preserved as
  evidence the design was considered.

Files do not move out of `done/` or `archive/` after they land
there. Numbering is permanent: a file's RFC number is assigned
at creation and never changes, even if the file moves between
folders.

## A note on namespaces

Most RFCs in this directory share a single sequential numbering line
(`001-…`, `002-…`, … `087-…` at the time of writing; the next free
slot is `093`). One **parallel namespace** also exists:

- **`RFC-MI-NNN-*`** — the **Mockup Integration epic**, introduced
  in v0.49.0. These RFCs cross-reference each other heavily by their
  `MI-NNN` identifiers (`RFC-MI-020` depends on `RFC-MI-010`,
  `RFC-MI-021`, `RFC-MI-012`, etc.); they were authored together as a
  coherent eight-phase plan and are introduced under their original
  numbering so the cross-reference graph stays intact. The supporting
  planning artifacts (migration plan, codebase handoff, mockup
  handoff package) live in
  [`../docs/mockup-integration/`](../docs/mockup-integration/).

  When a new MI RFC is created, take the next slot in the MI line
  (the existing set occupies `MI-000`, `010`–`012`, `020`–`022`,
  `030`–`031`, `040`–`041`, `050`–`051`, `060`, `070`, `080`; gaps
  are deliberate to leave room for siblings within each phase). When
  a new non-MI RFC is created, take the next slot in the main line
  (`069`, then `070`, …).

  Numbers in either namespace are permanent and never re-used, per
  RFC 018.

## Index

### Proposed — Mockup Integration epic (Phase 0 → Phase 8)

**The mockup integration arc is complete as of v0.57.0.**

All 16 MI RFCs have been implemented across Phases 0–8.
See the Implemented table above for the full list with release versions.

The migration plan (`docs/mockup-integration/migration-plan.md`) and
verification matrices (`docs/src/mockup-integration/`) document the
complete arc.

Phase-1 blockers resolved:

- **`D-01`** (RFC-MI-010 v0.50.0) — `components.rs` split into 11 bounded shards. ✅
- **`D-02`** (RFC-MI-022 v0.51.1) — Path-based route tabs; query-parameter model rejected. ✅
- **`D-03`** (RFC-MI-021 v0.51.0) — CSRF threaded through `Shell` server-side. ✅

### Implemented — maintenance / dependency refresh

| RFC | Title | Release |
|-----|-------|---------|
| 070 | [ureq → reqwest migration](./done/070-ureq-to-reqwest.md) | v0.57.1 |
| 069 | [rand 0.10 migration](./done/069-rand-0.10-migration.md) | v0.57.1 |

### Implemented — UX rethink + pre-1.0 polish

| RFC | Title | Release |
|-----|-------|---------|
| 073 | [Dashboard action items](./done/073-dashboard-action-items.md) | v0.58.0 |
| 071 | [Auditor role](./done/071-auditor-role.md) | v0.59.0 |
| 072 | [End-user app-access surface](./done/072-end-user-app-access.md) | v0.60.0 |
| 074 | [Navigation restructuring and UX polish](./done/074-nav-ux-polish.md) | v0.61.0 |

All pre-1.0 RFCs are now implemented. The remaining items in
`rfcs/proposed/` are all post-1.0 exploratory work.

### Implemented — verification-soak

| RFC | Title | Release |
|-----|-------|---------|
| 075 | [File-size refactor](./done/075-file-size-refactor.md) | v0.62.0 |
| 076 | [Configuration reference documentation](./done/076-configuration-reference.md) | v0.62.0 |



### Proposed — maintenance / dependency refresh

All maintenance RFCs from this category are implemented. See the
Implemented table above.

### Proposed — security-assurance arc (RFCs 078–086, v0.63.2)

Created from the architect audit requested by
`security-critical-assurance-strategy-v0.63.1.md`. The audit
report (gaps G1–G9, Category A–D classification, sequencing) is
at [`docs/security-assurance-audit-v0.63.1.md`](../docs/security-assurance-audit-v0.63.1.md).
Recommended order: 078 → 080 → 079 → 081 → 082 → 083 / 085 →
084 → 086.

| ID  | Title | Category / Priority |
|-----|-------|---------------------|
| 079 | [Authorization code lifecycle assurance](./proposed/079-authorization-code-lifecycle-assurance.md) | A |
| 080 | [Refresh rotation atomicity & reuse detection](./proposed/080-refresh-rotation-atomicity.md) | A — highest-risk finding (G1) |
| 081 | [Actor scope boundary & scoped repository signatures](./proposed/081-actor-scope-boundary.md) | B |
| 082 | [Authorization decision core & property tests](./proposed/082-authorization-decision-core.md) | B |
| 083 | [Security state-machine testing with proptest](./proposed/083-security-state-machine-testing.md) | B |
| 084 | [Fuzzing for untrusted input boundaries](./proposed/084-fuzzing-untrusted-input-boundaries.md) | B/C |
| 085 | [Audit event completeness for privileged operations](./proposed/085-audit-event-completeness.md) | B |
| 086 | [Lightweight formal / model-checking pilot](./done/086-formal-model-checking-pilot.md) | C — shipped v0.69.0 |

### Proposed — toolchain maintenance

| # | Title | Category |
|---|---|---|
| 087 | [Clippy and rustfmt baseline cleanup (Rust 1.96)](./done/087-clippy-rustfmt-baseline-cleanup.md) | D — shipped v0.65.1 |

### Proposed — post-1.0 candidates (open for review)

| ID  | Title                                                          | Priority |
|-----|----------------------------------------------------------------|----------|
| 008 | [Third-party-posture bundle](./proposed/008-third-party-posture.md) | Low-medium — post-1.0 |
| 025 | [Multi-tenant expansion path: detailed design](./proposed/025-multi-tenant-expansion.md) | Low — post-1.0, no schedule |
| 004 | [Federation as upstream OIDC client](./proposed/004-federation.md) | Low — post-1.0 |
| 005 | [Pluggable user backends (LDAP)](./proposed/005-pluggable-user-backends.md) | Low — post-1.0 |
| 006 | [Prometheus metrics endpoint](./proposed/006-metrics.md) | Low — post-1.0 |
| 009 | [Pluggable SQL backends (PostgreSQL, MariaDB)](./proposed/009-sql-backends.md) | Low — post-1.0 |

### Implemented

| ID  | Title                                                          | Shipped in |
|-----|----------------------------------------------------------------|------------|
| 078 | [Security-critical type modeling baseline](./done/078-security-type-modeling-baseline.md) | v0.64.0 |
| MI-080 | [UI Regression and Accessibility Hardening](./done/RFC-MI-080-ui-regression-a11y-hardening.md) | v0.57.0 |
| MI-070 | [OIDC Consent UX Integration](./done/RFC-MI-070-oidc-consent-ux.md) | v0.56.0 |
| MI-060 | [Self-Service Security Tab Integration](./done/RFC-MI-060-self-service-security-tabs.md) | v0.55.0 |
| MI-051 | [Danger Zone and Confirmation Screen Integration](./done/RFC-MI-051-danger-confirmation.md) | v0.54.0 |
| MI-050 | [Form System and Validation Feedback](./done/RFC-MI-050-form-system-validation.md) | v0.54.0 |
| MI-040 | [Setup Wizard UX Integration](./done/RFC-MI-040-setup-wizard-ux.md) | v0.53.1 |
| MI-041 | [Authentication Surface Integration](./done/RFC-MI-041-authentication-surfaces.md) | v0.53.0 |
| MI-031 | [Audit Log and Read-Only Table Integration](./done/RFC-MI-031-audit-readonly-tables.md) | v0.52.0 |
| MI-030 | [Dashboard and Summary Surface Integration](./done/RFC-MI-030-dashboard-summary.md) | v0.52.0 |
| MI-022 | [Route-Based Tab Component](./done/RFC-MI-022-route-based-tab-component.md) | v0.51.1 |
| MI-021 | [Server-Rendered CSRF for Shell-Level Forms](./done/RFC-MI-021-server-rendered-csrf-shell.md) | v0.51.0 |
| MI-020 | [Shell Layout Integration](./done/RFC-MI-020-shell-layout-integration.md) | v0.51.0 |
| MI-012 | [Theme Persistence Decision](./done/RFC-MI-012-theme-persistence.md) | v0.50.1 |
| MI-011 | [Mockup Token Mapping and Visual Primitive Adoption](./done/RFC-MI-011-token-mapping-visual-primitives.md) | v0.50.1 |
| MI-010 | [Component CSS Sharding and Export Discipline](./done/RFC-MI-010-component-css-sharding.md) | v0.50.0 |
| MI-000 | [Baseline Delta Inventory and Integration Mapping Contract](./done/RFC-MI-000-baseline-delta-inventory.md) | v0.49.1 |
| 068 | [`handlers/me_security.rs` split per tab domain](./done/068-me-security-handlers-split.md) | v0.48.0 |
| 067 | [Inline-style discipline + CI bound](./done/067-inline-style-discipline.md) | v0.48.0 |
| 066 | [`handlers/admin.rs` split per screen domain](./done/066-admin-handlers-split.md) | v0.47.1 |
| 065 | [`pages.rs` split per screen domain](./done/065-pages-split-per-screen.md) | v0.47.0 |
| 064 | [Empty / error state primitives](./done/064-empty-error-state-primitives.md) | v0.46.0 |
| 063 | [Dashboard signal vs. noise pass](./done/063-dashboard-signal-noise.md) | v0.46.0 |
| 062 | [Card variant primitives](./done/062-card-variant-primitives.md) | v0.46.0 |
| 061 | [Semantic palette extension](./done/061-semantic-palette-extension.md) | v0.46.0 |
| 060 | [Audit-note rollout](./done/060-audit-note-rollout.md) | v0.45.0 |
| 059 | [Confirm-screen template component](./done/059-confirm-screen-template-component.md) | v0.45.0 |
| 058 | [Dangerous-action step-up enforcement](./done/058-dangerous-action-step-up-enforcement.md) | v0.45.0 |
| 057 | [Language save confirmation](./done/057-language-save-confirmation.md) | v0.44.0 |
| 056 | [Recovery codes remaining count](./done/056-recovery-codes-remaining-count.md) | v0.44.0 |
| 055 | [Consolidate self-service onto `/me/security/*`](./done/055-self-service-unification.md) | v0.44.0 |
| 054 | [Aria-label / title attribute i18n audit](./done/054-aria-title-attribute-audit.md) | v0.44.0 |
| 053 | [Copy-button i18n contract](./done/053-copy-button-i18n.md) | v0.43.0 |
| 052 | [Status word vocabulary unification](./done/052-status-word-vocabulary.md) | v0.43.0 |
| 051 | [Per-screen i18n completeness audit](./done/051-per-screen-i18n-completeness.md) | v0.43.0 |
| 050 | [Admin chrome i18n (Nav, Footer, ThemeToggle)](./done/050-admin-chrome-i18n.md) | v0.42.0 |
| 049 | [CSS token vocabulary freeze](./done/049-css-token-vocabulary-freeze.md) | v0.42.0 |
| 048 | [Fix `t.xxx` brace-missing literals in `pages.rs`](./done/048-fix-i18n-brace-missing.md) | v0.42.0 |
| 047 | [Dev mode summary + client secret rotation audit](./done/047-dev-summary-and-secret-rotation.md) | v0.41.0 |
| 046 | [Audit log per-row copy ID](./done/046-audit-row-copy-id.md) | v0.41.0 |
| 045 | [User disable reason input](./done/045-user-disable-reason.md) | v0.41.0 |
| 044 | [UI state word contract](./done/044-state-word-contract.md) | v0.40.0 |
| 043 | [Dashboard recent important events](./done/043-dashboard-recent-events.md) | v0.40.0 |
| 042 | [Error / rate-limited page i18n](./done/042-error-pages-i18n.md) | v0.40.0 |
| 041 | [HIBP consistency in admin::create_user](./done/041-hibp-consistency.md) | v0.40.0 |
| 040 | [`/me/security` tabbed structure](./done/040-me-security-tabs.md) | v0.40.0–v0.41.0 |
| 039 | [Settings i18n completion](./done/039-settings-i18n-completion.md) | v0.39.0 |
| 038 | [OIDC consent screen](./done/038-consent-screen.md) | v0.39.0 |
| 036 | [Distribution readiness (docs Phase 5)](./done/036-distribution-readiness.md) | v0.37.0 |
| 035 | [User detail page](./done/035-user-detail-page.md) | v0.37.0 |
| 034 | [Login + passkey empty states](./done/034-login-passkey-empty-states.md) | v0.36.0 |
| 033 | [Audit log enhancements](./done/033-audit-log-enhancements.md) | v0.36.0 |
| 032 | [Dev mode browser banner](./done/032-dev-mode-browser-banner.md) | v0.35.0 |
| 031 | [Dashboard operator prompts](./done/031-dashboard-operator-prompts.md) | v0.36.0 |
| 030 | [Dangerous-operation confirmation screens](./done/030-dangerous-ops-confirmation.md) | v0.36.0 |
| 029 | [Admin i18n completion](./done/029-admin-i18n-completion.md) | v0.35.0 / v0.37.0 |
| 028 | [Copy-to-clipboard UX](./done/028-copy-to-clipboard-ux.md) | v0.31.0 |
| 027 | [Client scope configuration UX](./done/027-client-scope-configuration-ux.md) | v0.29.13 |
| 026 | [Admin logout + session self-management](./done/026-admin-logout-session-self-management.md) | v0.29.13 |
| 024 | [Documentation file consolidation](./done/024-doc-file-consolidation.md) | v0.32.0 |
| 023 | [Visual design system: tokens, components, motion](./done/023-visual-design-system.md) | v0.32.0 |
| 022 | [Single-realm scope statement](./done/022-single-realm-scope-statement.md) | (doc-only; folded into v0.30+ docs) |
| 021 | [Schema invariant CHECKs and migration safety](./done/021-schema-invariant-checks.md) | v0.29.10–11 |
| 020 | [User identity invariants and OIDC claim consistency](./done/020-user-identity-invariants.md) | v0.29.x |
| 019 | [Auth flow data integrity hardening](./done/019-auth-flow-data-integrity.md) | v0.29.x |
| 018 | [RFC lifecycle policy](./done/018-rfc-lifecycle-policy.md) | v0.29.5 |
| 017 | [UI/UX design contracts](./done/017-ui-ux-design-contracts.md) | v0.32.0 |
| 016 | [Server logging completeness](./done/016-server-logging-completeness.md) | v0.29.4 |
| 015 | [Documentation consistency pass](./done/015-doc-consistency-pass.md) | v0.29.4 |
| 014 | [Hot-path caches and benchmark harness](./done/014-hot-path-caches-and-benchmarks.md) | v0.31.0 |
| 013 | [Reduce SQLite blocking on async handlers](./done/013-db-blocking-mitigation.md) | v0.30.0 |
| 012 | [Setup wizard scope reconciliation](./done/012-setup-wizard-reconciliation.md) | v0.29.4 |
| 011 | [Enforce WebAuthn transport at the server](./done/011-webauthn-transport-enforcement.md) | v0.29.4 |
| 010 | [Revoke sessions on forgot-password](./done/010-forgot-password-revoke.md) | v0.29.4 |
| 003 | [HIBP scope expansion](./done/003-hibp-expansion.md) | v0.29.4 |
| 002 | [i18n scope expansion: zh, formatters, audit labels](./done/002-i18n-expansion.md) | v0.34.0 |
| 001 | [Persistent email outbox + retry worker](./done/001-email-outbox.md) | v0.33.0 |

### Archive

| ID  | Title          | Disposition                                                |
|-----|----------------|------------------------------------------------------------|
| 007 | [Multi-tenancy](./archive/007-multi-tenancy.md) | Superseded by [RFC 025](./proposed/025-multi-tenant-expansion.md) |

## Implementation order

The current near-term direction is the v0.42 → v1.0-rc UI/UX
hardening plan, six phases (A through F) one per release:

- **Phase A (v0.42.0, shipped):** RFCs 048, 049, 050 — stop the
  bleeding (rendered `t.xxx` literals, undefined CSS variables,
  non-i18n chrome).
- **Phase B (v0.43.0, in `proposed/`):** RFCs 051, 052, 053, 054 —
  i18n completeness sweep across every page body, status words,
  copy buttons, and ARIA attributes.
- **Phase C (v0.44.0):** Self-service unification onto `/me/security/*`.
- **Phase D (v0.45.0):** Dangerous-operations contract enforcement.
- **Phase E (v0.46.0):** Visual hierarchy + palette extension.
- **Phase F (v0.47.0):** Code structure (split `pages.rs` and
  `handlers/admin.rs`).
- **Buffer (v0.48.0):** RFC index reconciliation and per-screen
  verification doc.

After Phase F, v1.0-rc opens. The post-1.0 backlog (RFCs 004, 005,
006, 008, 009, 025) targets longer-term work and is intentionally
out of the v1.0 critical path.

## Template

The standard shape is light:

```markdown
# RFC NNN — Title

**Status.** Proposed | Implemented (vX.Y.Z) | Withdrawn | Superseded by RFC NNN
**Tracks.** ROADMAP item or other context this addresses.
**Touches.** crates / modules the work lands in.

## Summary

One paragraph. What changes for the user, why now, why this shape
over the alternatives.

## Background (optional)

Context the implementer needs that isn't on ROADMAP.md. Skip when
the title alone tells you what's going on.

## Design

What the implementer builds. Schemas, function signatures, state
machines, error paths. Treat this as the contract.

## Multiple implementation steps

If the work splits into stages that can ship separately, list them
here with rough scope.

## Tests (when non-trivial)

What the implementer should write to call it done.

## Security considerations (when applicable)

What an attacker might try, and what the design does about it.

## Open questions

Anything the implementer should bring back before merging.
```

### When to add the heavier sections

The light template handles small, mechanical items. Anything
medium or larger — schema changes, new background workers,
cross-cutting policies, third-party integration shapes — earns
the heavier sections:

- **Requirements** — explicit list of what must be true after the
  change ships, separately from the design that delivers it.
- **Design** (replaces "Design" section title above) — same
  intent, but expected to be thorough rather than sketchy.
- **Test plan** — coverage map: what unit, integration, and
  regression tests get added; what existing tests might need to
  move.
- **Security considerations** — first-class section, not a footnote.

Each RFC declares which sections it carries by the headings it
uses. There's no separate metadata.

## Process

The full lifecycle is described in
[RFC 018](./done/018-rfc-lifecycle-policy.md). The short version:

1. New RFC: open a draft as `rfcs/proposed/NNN-slug.md` with
   Status `Proposed`. The number is the next unused integer,
   zero-padded to three digits, and never reused.
2. Iterate in review until the design is settled.
3. When the work ships, move the file to `rfcs/done/`, update
   Status to `Implemented (vX.Y.Z)`, and update inbound
   references in this README and other RFCs.
4. RFCs that don't pan out move to `rfcs/archive/` with Status
   `Withdrawn` (and a one-line reason) or `Superseded by RFC NNN`.
   They stay there as a record.

Files are never deleted. The full reasoning is in RFC 018.


## UI-Security Contract (RFCs 088–092, v0.70.x–v0.74.x)

Units 2–6 of the v2.3 UI/UX contract. Each unit is one RFC, sequenced by
dependency.

| RFC | Title | Status |
|---|---|---|
| 088 | [Auditor authorization matrix and static read-only rendering](./done/088-auditor-matrix-read-only-rendering.md) | ✅ Shipped v0.70.0 |
| 089 | [Step-up authentication contract](./proposed/089-step-up-contract.md) | Proposed |
| 090 | [Signing-key rotation confirm + settings pending-change](./done/090-signing-key-confirm-pending-change.md) | ✅ Shipped v0.72.0 |
| 091 | [LoginContext rendering and SelfServiceShell navigation](./done/091-login-context-self-service-shell.md) | ✅ Shipped v0.73.0 |
| 092 | [UI components: ThemeToggle, EmptyState, CopyField, Error summary](./proposed/092-ui-components.md) | Proposed |
