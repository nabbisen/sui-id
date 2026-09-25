# RFC 094 Stage-0 durable-write command inventory

**Snapshot:** v0.76.12 working tree, inspected 2026-07-17
**Governing RFC:** [RFC 094](../../accepted/094-transactional-audit-registry.md)
**Review state:** Base inventory independently design-approved on 2026-07-17;
the C17/C18/C23/F01–F06 RFC 096 amendment was independently reviewed and
approved by `@nabbisen` on 2026-07-21, durably returned to Proposed in commit
`43085e38219e5eb1bfe11cc698b18f1fa5f5e4d7`, and accepted as part of the
complete amended RFC; implementation reconciliation and entry gates remain pending

This is the closed Stage-0 classification of production durable-write entry
points. **The rows are no longer in this file.** They live in
[`contracts/write-commands.toml`](../../../contracts/write-commands.toml), the one copy
([RFC 116](../../accepted/116-gate-contracts.md) D1), which
`scripts/check-write-commands.py` (lane G17) checks against the code: every
sealed command has a row, every row that says it is sealed names a command,
every `files` path exists. A row cited elsewhere as "row U24" or "F04" is that
`id` in the TOML. This file keeps what a table cannot carry: the classes, the
closed event-branch rule, the reasoning, and the record of what was retired or
removed. Source changes discovered during implementation amend the TOML and
return through design review before they can enter the closure universe.

**Retired: `U06`, admin password reset.** It was retired by
[RFC 103](../../done/103-administrator-issued-account-recovery.md) D1 and replaced
by `U37` (issue a recovery link); no path may set a password on a user's behalf.
Its row was `A`, event `user.reset_password`, mutation surface
`credentials::upsert` plus token and session invalidation, test
`a_u06_password`. The TOML has no `U06` row; this paragraph is its record.

## Classes

| Class | Meaning |
|---|---|
| **A** | Security/operator command: mutation and named typed audit event commit in one Class-A transaction. |
| **P** | High-frequency or short-lived protocol state: classified, capability-gated, and tested, but no per-write success audit because the corresponding protocol event/aggregate is the useful record. |
| **O** | Operational housekeeping/worker state: capability-gated and observable through operational telemetry, not a privileged audit success. |
| **I** | Internal mutation primitive: not a production entry point after conversion; callable only beneath the named A/P/O command capability. |
| **X** | Bootstrap/schema migration: restricted to the migration runner before service readiness; migration identity and result are release/upgrade evidence. |

P, O, I, and X are explicit exclusions from Class A, not bypasses. Every row is
compiler-registered under one of these capabilities. A new write with no class
does not compile and fails the structural negative fixture.

## Closed event-branch rule

Every A invocation commits exactly one event. Rows listing multiple event/test
IDs use a sealed result enum in one Class-A command declaration; the guarded
mutation returns one enum variant and exhaustive matching constructs the
corresponding typed payload. There is no successful variant without a payload.

- U01 branches on the HIBP policy outcome after one user-creation intent.
- U22 branches on whether the same failure-count update crosses the lockout
  threshold; both the non-threshold counter update and lockout are Class A.
- T04 branches on a normal refresh-rotation winner versus reuse-triggered
  family revocation; both outcomes are Class A.
- C17 branches on the requested provider enabled state; both branches have
  separate payload/test bindings. Its enable branch consumes a fresh sealed
  exact-policy/version/generation preflight capability, increments generation,
  and stores its evidence atomically; disable increments generation, clears
  evidence, and invalidates attempts.
- C19 branches on the transaction's authoritative insert-versus-update result.
- C23 has one provider-policy-replacement outcome; the old/new version/generation
  predicate and attempt invalidation are part of that outcome, not C17.
- F04 has one first-provision outcome. It is not a nested U01+C19+U30 command.

Client enable/disable and registration-authorization issue/revoke are distinct
operator intents and therefore use separate IDs (C05/C21 and C14/C22), even
though each pair shares one current repository function family.

## RFC 096 federation login commands (accepted amendment)

The F01–F06 rows of the TOML freeze the compound ownership boundary. F01–F03/F05/F06 are explicit
Protocol exclusions under the previously reviewed base-design U24/U30/U32 and Class-B
login-result policy; “P” does not mean separate best-effort writes. Each uses
one protocol transaction and the private subordinate primitives listed in
its row's `mutation_surface`.
F04 creates identity authority and is Class A.

F01/F03/F04 update link observation only for a successful session-producing
transaction. Observation cannot create, reassign, or delete a link and is an
internal primitive beneath those commands, not C19. Any observation/session/
cap/bookkeeping failure rolls back that protocol or Class-A transaction.

F02 creates no session, changes no link observation, and emits no success.
F03 starts from count zero and allows five wrong bound submissions total across
methods. Counts 1–4 commit `RejectedStillPending`; equality at 5 commits
`AttemptsExhausted`; no sixth transition exists. Correct promotion is allowed
only at 0–4. Invalid browser/CSRF binding touches no row. TOTP step, recovery
hash, or passkey counter cannot commit without the session. Wrong recovery/TOTP
consumes no anti-replay authority; wrong WebAuthn consumes only its failed F06
ceremony while incrementing the shared count. F04 has no MFA branch because its new passwordless user
cannot already own a local factor. Collision/disabled-user/link-only and all
post-claim validation failures go through F05 and never call F01–F04/F06.

F03 Class-B observations are exactly `auth.federation.signin.success` or:

- `auth.federation.mfa_rejected` with
  `totp|recovery|webauthn|malformed|method_mismatch`;
- `auth.federation.mfa_exhausted` with `attempt_limit`; or
- `auth.federation.mfa_invalidated` with
  `expired|provider_changed|provider_disabled|link_changed|user_inactive|factor_changed|ceremony_invalid`.

F05 Class-B observations are exactly:

- `auth.federation.signin.upstream_failure` with
  `upstream_error|transport|discovery|token_response|jose|claims|timeout|cancelled`;
- `auth.federation.takeover_blocked` with `email_collision`;
- `auth.federation.link_required` with `link_only`; or
- `auth.federation.signin.denied` with
  `attempt_expired|provider_changed|provider_disabled|user_inactive|link_changed|provision_policy|internal_failure`.

These are registry enums, not strings supplied by handlers. Payloads contain
internal IDs only and reject proof/counter/email/subject/endpoint/upstream text.

Compile-negative fixtures reject nested Class-A invocation; direct use of the
link-observation/user/link/session primitives; construction or substitution of
verified-attempt and federated-MFA capabilities; and event/session completion
from the wrong command. Fault tests inject every write/event/commit point and
reconcile attempt, pending-MFA, anti-replay, user, link, session, bookkeeping,
session-cap, event, and chain state.

## Settings, setup, keys, and operational state

**Removed 2026-07-28 — master-key rotation.** `K04` (master-key database
reseal phase) and `K05` (master-key activation completion) were removed from
this inventory by the RFC 094 scope amendment and are owned by
[RFC 100](../../proposed/100-master-key-rotation-recovery.md). Their database
phases consume the Class-A seam established by RFC 094, which is why RFC 100
depends on M2a. `K06` was reworded accordingly: it is subordinate to `S10` or
`K01` only, and RFC 100 re-establishes its own subordination when its commands
land. No other row changed.

## Completeness reconciliation

The implementation-generated manifest expands every grouped mutation surface
above to exact function and SQL-write-site identifiers. Stage 0 is complete
only when:

1. every current public repository writer named by source inspection maps to
   exactly one row above or becomes private beneath one row;
2. every direct SQL `INSERT`, `UPDATE`, `DELETE`, schema DDL, and write pragma
   outside migrations maps to a registered capability;
3. setup, CLI, worker, startup/backfill, and dev-only direct writes map to A,
   P, O, X, or become impossible;
4. the independent reviewer compares the generated source-site list with this
   inventory and records every discrepancy before RFC acceptance.

Read-only repository functions and pure helpers are outside the durable-write
universe. `audit::append` is not a domain command: Class-A callers reach
`append_within_tx` only through the transaction runner; Class-B callers reach a
must-attempt emitter. Neither raw append API remains public to domain code.
