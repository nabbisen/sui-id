# RFC 085 — Audit Event Completeness for Privileged Operations

**Status.** Implemented (v0.68.0)
**Tracks.** Strategy theme 8 (audit gap G8). Category B.
**Touches.** New `docs/src/reference/audit-coverage-matrix.md`
(or extension of the existing audit-events reference), new
`sui-id-core/src/audit_guard.rs`,
`sui-id-store/src/repos/audit.rs` (within-tx append),
privileged domain functions across `sui-id-core/src/admin/*`,
`me_security.rs`, `key_rotation.rs`, token/session revocation
paths; CI script `scripts/check-audit-matrix.sh`.

## Summary

Turn audit coverage from a call-site convention into a checked
contract: (1) a normative **coverage matrix** enumerating every
privileged operation and its required event name, fields, and
atomicity class; (2) a `within_tx` audit append so Class-A
operations commit state change and audit row atomically or not
at all; (3) an `Audited<T>` completion wrapper so a privileged
domain function cannot return success without having produced
its audit record; (4) a CI check keeping matrix, event-name
constants, and emission sites in sync.

## Motivation

Strategy §5.7: security-sensitive operations must produce
complete audit events; important state changes should not commit
without their record where the design requires atomicity. In
v0.63.1, events are appended ad hoc after the fact, frequently as
`let _ = audit::append(...)` — fire-and-forget even for
high-impact operations, and nothing detects a *missing* call
site. The hash chain protects integrity of what was written; this
RFC protects *that it is written*. RFC 081's `Actor` gives every
emission a reliable principal, removing today's `actor: Option<…>`
ambiguity for request-driven operations.

## Background

Existing strengths to preserve: stable dot-delimited event names;
SHA-256 hash chain; documented events reference; theft-detection
and master-key-rotation events already exist. The audit-row copy
vocabulary is governed by `docs/ui-ux-contracts.md`.

## Target code areas

1. **Coverage matrix** (normative doc): rows = privileged
   operations (the same enumeration as RFC 082's `Action` enum,
   plus system events like key rotation and theft detection);
   columns = event name, required fields (actor, target, result,
   note schema), atomicity class:
   - **Class A (atomic):** state change and audit row commit in
     one transaction. Members: user disable/delete/role-change,
     client create/delete/secret-rotate, signing-key rollover,
     force-logout, master-key rotation, MFA admin-reset,
     settings changes.
   - **Class B (best-effort, must-attempt):** events where the
     primary action is itself a denial or detection (e.g.
     `auth.refresh.theft_detected`) — emission failure is logged
     loudly but does not mask the security response.
2. **Store:** `audit::append_within_tx(tx, &AuditLogRow)` —
   hash-chain computation inside the caller's transaction
   (chain head read + new row write under the same tx; the
   single-writer connection makes this race-free, and the
   pattern stays correct under future storage because the read
   and write share the transaction).
3. **Core:** `audit_guard.rs`:

   ```rust
   /// Proof that an audit record was appended. Constructed only
   /// by audit append functions.
   pub struct AuditReceipt { /* private */ }

   /// Privileged domain functions return this instead of `T`.
   pub struct Audited<T> { value: T, receipt: AuditReceipt }
   ```

   Handlers unwrap via `into_inner()`; the type makes "mutated
   but never audited" unrepresentable for converted functions.
4. **CI:** `scripts/check-audit-matrix.sh` greps event-name
   constants and asserts (a) every matrix row's event name exists
   in code, (b) every `audit.` / `admin.` / `auth.` constant in
   code has a matrix row — bidirectional, same spirit as the
   css-tokens gate.

## Security properties / invariants

- **P1 (completeness).** Every operation listed in RFC 082's
  action enumeration that mutates state has a matrix row and an
  emission site (CI-checked).
- **P2 (atomicity, Class A).** A Class-A state change is
  observable ⇔ its audit row is in the chain. Crash between the
  two is impossible by construction.
- **P3 (no secret leakage).** Audit `note` fields never carry
  raw secrets; with RFC 078 types, secret-bearing types don't
  `Display`, so accidental interpolation fails to compile;
  a unit test scans emitted fixtures for known test-secret bytes.
- **P4 (attribution).** Every request-driven event names its
  `Actor` (RFC 081); CLI operations name the CLI principal.
- **P5 (chain integrity preserved).** within-tx append maintains
  the existing hash-chain semantics; the chain verifier passes
  over mixed old/new rows.

## Non-goals

- No external log shipping, SIEM integration, or retention
  changes.
- No conversion of *informational* events (login success traces)
  to Class A — only privileged mutations.
- Not an alerting system.

## Proposed design

(Above.) Conversion order follows risk: admin/users and
admin/clients first, then signing keys and settings, then
force-logout/MFA-reset, with the matrix written *first* so each
conversion is checklist-driven.

## Data model impact

None (audit table unchanged; chain algorithm unchanged).

## API impact

Internal: privileged domain functions return `Audited<T>`;
handlers updated mechanically.

## Testing strategy

- Per Class-A operation: success test asserting row presence with
  required fields; **injected-failure test** (audit append forced
  to fail) asserting the state change rolled back (P2).
- Chain-verification test across pre/post-RFC rows (P5).
- Matrix CI script with deliberate desync fixture test.
- Secret-scan fixture test (P3).

## Migration strategy

No data migration. Old rows remain valid chain members.

## Rollout plan

Suggested v0.67.0 alongside RFC 083 (the state-machine harness
can then also assert audit emission as an invariant of privileged
ops). Matrix doc lands first within the release branch.

## Risks and mitigations

- *Risk:* Class-A rollback turns audit-subsystem failure into
  operation failure. Intended — fail-safe per spec §6.3; the
  failure itself is traced with request ID.
- *Risk:* wrapper boilerplate. Mitigation: a small
  `audit_and(tx, row, || mutation)` helper produces
  `Audited<T>` in one expression.

## Acceptance criteria

- Matrix published; CI gate live and bidirectional.
- All Class-A operations converted; injected-failure tests prove
  rollback; chain verifier green; 0 warnings; baseline green.

## Open questions

- Should `auth.code.replay_denied` (RFC 079 open question) and
  failed step-up attempts join the matrix as Class B? Decide
  while writing the matrix — the matrix review is the venue.
