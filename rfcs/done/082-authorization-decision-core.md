# RFC 082 — Authorization Decision Core and Property Tests

**Status.** Implemented (v0.67.0)
**Tracks.** Strategy theme 5 (audit gap G5). Category B.
**Touches.** New `sui-id-core/src/authz.rs` (+ `authz/tests.rs`),
`sui-id-core/src/actor.rs` (RFC 081), call sites of role checks
in core and the binary crate.

## Summary

Extract the project's authorization rules into one small, pure,
side-effect-free function —
`authorize(actor_view, action) -> Decision` — that every
enforcement point (extractor conversions from RFC 081, handler
guards, last-admin safeguard) delegates to, and pin its
invariants (deny-by-default, role monotonicity, read/write
separation, last-admin protection) with property-based tests.

## Motivation

Strategy §5.5 asks for a clearly bounded authorization core.
Today the rules — Admin writes everything; Auditor reads all
admin surfaces, writes nothing; User touches only `/me/*`; the
last active admin cannot be disabled/deleted/demoted — are
correct but *distributed* across extractors, `can_write` checks,
and admin/users domain logic. Distributed rules can't be property
tested as a whole, and each new surface re-implements a slice of
them. A pure core makes the rules enumerable, testable, and (per
RFC 086) a candidate for a bounded Kani proof.

## Background

Three roles, no permission lattice beyond them (and none wanted —
strategy §4 rejects premature policy engines). The decision
inputs are small: actor role, action, and for the last-admin rule
a single environmental fact (count of other active admins).

## Target code areas

```rust
// sui-id-core/src/authz.rs (new, pure: no db, no clock, no IO)

/// Every privileged action in the system, enumerated. Adding a
/// handler that needs privilege means adding a variant — which
/// forces a row in the decision table and in the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // Admin read surfaces
    AdminReadUsers, AdminReadClients, AdminReadAudit,
    AdminReadDashboard, AdminReadSettings, AdminReadSigningKeys,
    // Admin mutations
    AdminWriteUsers, AdminWriteClients, AdminWriteSettings,
    AdminRotateSigningKey, AdminRotateClientSecret,
    AdminResetUserMfa, AdminForceLogout,
    AdminChangeUserRole { target_is_last_admin: bool },
    AdminDisableUser   { target_is_last_admin: bool },
    AdminDeleteUser    { target_is_last_admin: bool },
    // Self-service
    SelfReadSecurity, SelfWriteSecurity,
    SelfRevokeOwnSessions, SelfRevokeConsent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision { Permit, Deny }

pub fn authorize(role: Role, action: Action) -> Decision {
    use Decision::*;
    match (role, action) { /* exhaustive table; final arm: Deny */ }
}
```

Environmental facts (is the target the last admin?) are computed
by the caller and passed in as data, keeping the function pure —
the strategy's "small pure authorization core" verbatim. RFC 081
conversions and the admin/users safeguard both call this table;
the table becomes the single normative artifact.

## Security properties / invariants

- **P1 (deny by default).** The match's structure guarantees any
  un-listed (role, action) pair denies; a `proptest` over the
  full input space asserts no panic and totality.
- **P2 (role monotonicity).** ∀ action: permit(User) ⇒
  permit(Admin); permit(Auditor) ⇒ permit(Admin) for read
  actions. Removing privileges from a role never adds permits.
- **P3 (read/write separation).** Auditor permits ⊆ read-only
  actions; ∀ mutation actions: authorize(Auditor, ·) = Deny.
- **P4 (last-admin).** ∀ role:
  authorize(role, {Disable,Delete,ChangeRole} with
  `target_is_last_admin = true`) = Deny — including for Admin.
- **P5 (self-scope).** User permits ⊆ Self* actions.
- **P6 (purity).** The function is deterministic and total
  (type-guaranteed; pinned by property test anyway).

## Non-goals

- No resource-instance-level permissions, no scopes/claims-based
  API authorization (OAuth scope policy stays where it is, in
  `enforce_scope_policy`), no external policy engine, no DSL.
- Not a replacement for endpoint integration tests.

## Proposed design

(Above.) One deliberate trade-off, per the strategy's "balance"
guidance: actions are a flat enum with embedded environmental
booleans rather than a (subject, action, resource, context)
quadruple — the quadruple is the seam we'd grow toward *if* RFC
025 (multi-tenant) lands, and the enum keeps today's table
readable by any Rust developer.

## Data model impact

None.

## API impact

Internal only. Extractors/guards change call form, not behaviour.

## Testing strategy

- Exhaustive table test: iterate all roles × all actions
  (enum-iterable via a test-only `ALL` const) against a
  hand-written expected matrix kept adjacent to the table —
  double-entry bookkeeping.
- proptest for P2/P3/P5 as universally quantified properties.
- P4 unit cases for all three last-admin actions × all roles.
- Integration tests in the binary crate remain the endpoint-level
  oracle (unchanged).

## Migration strategy

None.

## Rollout plan

With RFC 081 (suggested v0.66.0). The decision table should land
in the same release as the capability types so there is never a
release with two normative sources of authorization truth.

## Risks and mitigations

- *Risk:* enum drift — a new handler skips the table. Mitigation:
  RFC 081 makes mutations require capability types whose
  conversions call the table; skipping it doesn't compile.
- *Risk:* table and matrix-test co-editing errors. Mitigation:
  that is the point — two artifacts must agree, and property
  tests cross-check structure.

## Acceptance criteria

- `authz.rs` exists, pure, exhaustive, with final-arm deny.
- P1–P5 tests pass; extractor conversions and last-admin
  safeguard delegate to it; behaviour at every endpoint
  unchanged (integration suite green); 0 warnings.

## Open questions

- Granularity: is one `AdminWriteSettings` enough, or should
  dangerous settings (security tab) be a distinct action? Decide
  during the audit-matrix work of RFC 085, which enumerates the
  same operations.
