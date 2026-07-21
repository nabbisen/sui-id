# RFC 094 verification and evidence contract

## Required test shape

Use a table-driven harness keyed by stable command inventory ID. Each Class-A
row supplies fixture setup, command invocation, domain-state snapshot, audit
tail snapshot, applicable injection points, and committed-result assertions.
Missing harness registration is a structural-gate failure.

For each applicable injection point, observe:

1. command returns the expected internal failure;
2. domain snapshot is unchanged;
3. audit tail/head is unchanged and chain verification passes;
4. retry without injection commits exactly one mutation and one typed event;
5. a second retry follows the command's guarded/idempotent policy without a
   duplicate false-success record.

Concurrency-sensitive rows additionally run N-way contention with N at least
8, assert the intended winner count, and reconcile committed state with event
count and chain order.

## Negative structural fixtures

CI self-tests must demonstrate failure for:

- inventory command without descriptor;
- descriptor without documentation representation;
- duplicate serialized event name;
- command class differing from descriptor class;
- Class-A row without failure-injection test ID;
- production write surface not bound to an inventory command;
- new raw SQL write omitted from both inventory and registry;
- undeclared write attempted through `ReadConn`;
- prepared UPDATE, writable PRAGMA, ATTACH, backup/restore, returned raw
  statement, or indirect raw-helper attempt through `ReadConn`;
- raw connection/transaction access outside the exact reviewed executor and
  migration modules;
- direct construction of receipt/event identity from arbitrary string;
- direct construction of authorized context, caller-supplied actor/command/time,
  or payload used with the wrong command type;
- unmapped command-event variant, duplicate descriptor mapping, or event
  descriptor outside the command's closed allowed set;
- generated audit reference drift;
- C23 invoked through C17, cache eviction placed inside its transaction, or an
  old provider version authorizing after committed policy replacement;
- preflight evidence written outside C17, capability cloning/serialization/
  substitution, age exactly 600 accepted, stale-version/policy/generation enable,
  duplicate concurrent enable, generation overflow/coercion, preflight while
  enabled, or stored/pre-disable capability reused after disable/restart;
- F04 nesting U01/C19/U30 or any F01–F04 subordinate primitive callable as a
  production entry point;
- verified-attempt/federated-MFA capability construction, cloning, cross-user/
  provider/command substitution, handler-built `BoundRejected`/reason, invalid
  binding that touches a row, or password-primary use of an F02 row; and
- F01/F03/F04 session or success construction from an uncompleted, failed,
  expired, already-consumed, wrong-user, or wrong-provider protocol row; and
- a sixth MFA failure transition; count reset on restart/method switch; correct
  promotion at count 5; wrong proof consuming TOTP/recovery/passkey authority;
  WebAuthn ceremony unbound to F02 or completed outside F03; and F03/F05 event
  name or reason outside the frozen enum.

## Class-B verification

For each must-attempt family, inject append failure and assert that the primary
denial/detection remains effective and structured error telemetry is emitted.
Telemetry assertions must not compare or print secrets.

## Closure evidence layout

Store durable, tracked evidence at a repository path approved during RFC
acceptance. It must include:

- exact commit SHA and clean-tree statement;
- command inventory and typed-registry digest/diff;
- toolchain/tool versions and complete RFC 093 matrix results;
- one result row per Class-A inventory ID and injection point;
- concurrency result rows for sensitive guarded commands;
- one observed branch result for every closed multi-event Class-A enum, with no
  successful no-event variant;
- after complete-RFC reacceptance, C17 preflight capability age/race/replay and
  P1/P2-enable-disable-P2 rejection, every activation-generation transition/
  overflow/rollback, plus C18/C23 rollback/version/attempt invalidation and
  post-commit cache-eviction failure;
  and F01–F06 fault/concurrency
  reconciliation across attempt, pending-MFA, anti-replay, user, link, session,
  bookkeeping, cap, event, and chain state;
- 2/8/64-way wrong/correct MFA races for every method, final-attempt equality,
  restart, expiry, method substitution, F06 replacement, and post-limit replay;
- T04 rollback/contention rows for guarded revoke, successor insert, family
  revoke, each event append, and commit; initial issuance reconciles separately
  to T09/P;
- T04 preliminary-read/authoritative-re-read sequencing and zeroizing
  destruction on every non-returning successor-secret path;
- structural-gate positive run and negative-fixture self-tests;
- historic/new audit-row compatibility and chain-verification results;
- known limitations and every reviewed P/O/I/X exclusion; no Class-A
  exception is permitted;
- master/signing-key fault-matrix results and recovered terminal state for
  every journal/artifact/database/file boundary plus old-key disposition;
- short-write and mid-operation termination results for initial Intent,
  journal updates, next/backup temporaries, no-replace publication, and every
  file/directory fsync;
- reviewer identity, review date, findings, dispositions, and verdict.

Logs are sanitized. They contain IDs only where permitted, never passwords,
tokens, cookies, secrets, key material, raw authorization codes, real external
credentials, or database contents unrelated to the assertion.

## Closure decision

M2 passes only if there is no Class-A exception and the independent reviewer
can select any inventory row and trace it from command to private transaction
mutation, typed descriptor,
injected rollback test, generated documentation, observed clean-commit result,
and closure finding. Aggregate test counts without this traceability are
insufficient.
