# RFC 094 migration checklist

Complete Stage 0 before changing production behavior. Convert one bounded wave
at a time; the workspace and structural gate must remain green between waves.

## Stage 0 — inventory freeze

- [ ] Enumerate every production durable write entry point across core, store,
  web handlers, CLI/setup, background tasks, and federation/registration.
- [ ] Reconcile every row in `command-inventory.md` to exact Rust function and
  SQL write-site IDs in `ci/write-commands.toml`.
- [ ] Reclassify any current Class-B row that performs a durable security
  mutation, or split mutation and observation into Class A and B.
- [ ] Confirm `client.dynamic_register` is Class A in M2; RFC 095 owns later
  metadata/concurrency completion, not an audit-atomicity exception.
- [ ] Independently review the inventory against code and approve the threat
  delta before Stage 1.

## Stage 1 — registry foundation

- [ ] Add event kind and class enums plus deterministic descriptors.
- [ ] Add typed payloads with actor/target requirements and bounded attributes.
- [ ] Prove secret types cannot be formatted/coerced into payload attributes.
- [ ] Generate or mechanically verify audit reference documentation.
- [ ] Add duplicate-name, class-mismatch, missing-field, and stable-serialization
  tests.
- [ ] Add the checked-in command inventory and structural comparison tool.
- [ ] Add `ReadConn`, sealed `declare_write_command!`, A/P/O/X runners, and the
  exact reviewed raw-write module policy.
- [ ] Ensure `ReadConn` returns only owned mapped rows, requires SQLite
  read-only statements, and denies statements/batches, writable PRAGMA,
  ATTACH/DETACH, backup/restore, extension, raw-handle, and FFI access.
- [ ] Observe compile/AST failure for a new bare write absent from inventory and
  registry.
- [ ] Observe compile/runtime rejection for prepared UPDATE, writable PRAGMA,
  ATTACH, backup API, returned raw statement, and indirect raw-helper attempts.

## Stage 2 — transaction foundation

- [ ] Add private `WriteTx<AtomicAudit>` capability and `Database::class_a`
  runner taking private-field `AuthorizedCommandContext<C>` explicitly.
- [ ] Generate sealed `C::Event` sums and exhaustive descriptor matches; event
  variants cannot carry actor, command ID, correlation ID, or timestamp.
- [ ] Add arbitrary-kind, unmapped/duplicate variant, and wrong-context/event
  compile/structural fixtures.
- [ ] Make receipt construction private and post-commit.
- [ ] Add test-only failure points before/within append and at commit.
- [ ] Replace misleading `audit_and` best-effort semantics; make Class-B API
  explicitly must-attempt and observable on append failure.
- [ ] Prove the audit chain is read and written on the caller transaction.
- [ ] Convert login failure and refresh rotation as always-Class-A commands with
  exhaustive typed event variants for every committed outcome.
- [ ] Register initial root-family refresh issuance separately as T09/P and
  bind only its initial-issuance insert call site.
- [ ] Precompute T04 successor outside the transaction; atomically combine
  guarded old-row revoke, successor insert, and rotated event, or family revoke
  and theft event, inside one synchronous transaction.
- [ ] Use a query-only preliminary refresh-row read, re-read authoritative state
  inside T04, and hold the unused raw successor only in a zeroize-on-drop
  wrapper across every reuse/error path.
- [ ] Split client enable/disable and registration authorization issue/revoke
  into their distinct command IDs/tests.

## Wave A — users, credentials, sessions

- [ ] user create / warned create / disable / enable / delete / role change.
- [ ] admin password reset, MFA reset, unlock.
- [ ] self password change and password-reset consumption.
- [ ] MFA enrollment/disable/recovery regeneration mutations.
- [ ] force logout and self/admin session revocation commands.

## Wave B — clients and settings

- [ ] client create/update/scope/URI/disable/enable/delete/secret rotation.
- [ ] server security settings and metrics-token changes.
- [ ] pending sensitive change create/apply/cancel.
- [ ] registration-token create/revoke and baseline atomic dynamic registration.
- [ ] scope-definition create/delete.

## Wave C — keys, tokens, federation identity

- [ ] signing-key rotation/activation and deletion/retirement.
- [ ] master-key database reseal phase with external-file recovery contract.
- [ ] refresh-family and administrative token revocation commands.
- [ ] durable federation link creation/change.
- [ ] federation-provider create/enable/disable/delete.
- [ ] C17 enable consumes fresh sealed exact-version/policy/activation-generation
  preflight, checked-increments generation, and stores evidence atomically;
  disable increments generation and clears evidence; C23 increments version +
  generation; C18 increments generation before delete; no standalone writer.
- [ ] C23 provider trust-policy replacement under the accepted amended design, with
  guarded version/generation increment, forced disable, attempt invalidation, audit
  rollback, and post-commit cache eviction.
- [ ] F01–F06 exact federation attempt/link/user boundaries under the accepted
  amended design, including
  pending-MFA/method-ceremony/session/bookkeeping/event boundaries, five-failure
  state machine, and cross-command guards.
- [ ] verify denial/detection observations remain correctly Class B.

## Per-command conversion checklist

Repeat for every inventory row:

- [ ] authorization and non-racy validation occur before writer acquisition;
- [ ] racy state is rechecked inside the transaction;
- [ ] mutation uses only the private transaction capability;
- [ ] typed event is built from the authoritative mutation result;
- [ ] audit append shares the transaction and failure propagates;
- [ ] handler unwraps `Audited<T>` only after successful commit;
- [ ] legacy/bypass write call sites are removed;
- [ ] every injection point rolls back mutation and audit;
- [ ] successful retry commits one mutation and one event;
- [ ] concurrency loser emits no false success;
- [ ] structural inventory, docs, and tests agree;
- [ ] no audit attribute can contain a secret.
- [ ] command is present in the generated write-site universe and no raw writer
  survives outside the reviewed executor/migration modules.

## Stage 3 — signing-key rotation

- [ ] Convert `K01` signing-key rotation to the Class-A runner: retire old,
  insert/activate new, and append `signing_key.rotate` in one transaction.
- [ ] Inject failure before append and before commit; assert the prior active
  key is unchanged and no event committed.
- [ ] Assert exactly one active signing key after a successful rotation, backed
  by the unique-active constraint.

**Master-key rotation is not part of RFC 094.** The 2026-07-28 scope amendment
moved `K04`, `K05`, the rotation journal, atomic key-file publication, startup
crash recovery, and old-key custody to
[RFC 100](../../proposed/100-master-key-rotation-recovery.md). Do not implement
them in this milestone; RFC 100 carries its own checklist and crash-injection
matrix.

## Stage 4 — authority switch and closure

- [ ] Make the typed structural gate blocking.
- [ ] Relabel/remove literal parity as authoritative; preserve only useful
  diagnostic output with its limitation.
- [ ] Correct audit hash-chain and atomicity documentation.
- [ ] Run all RFC 093 lanes on one clean commit.
- [ ] Assemble per-command rollback/concurrency evidence and registry diff.
- [ ] Obtain independent adversarial closure review before moving RFC 094 to
  Done.
