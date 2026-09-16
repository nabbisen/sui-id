# R11 — login failures: what is observable, and what goes uncounted

**Governing.** [RFC 094](../../accepted/094-transactional-audit-registry.md) U22
(login failure accounting, Class A); `ROADMAP.md` risk register row R11.
**Implementer.** Mid-capability model. **Baseline.** `49a2897` or later.

## What R11 said, and what the code shows

R11 recorded an asymmetry: since the U22 conversion, a wrong password for a
**known** user propagates a database failure (`crates/sui-id-core/src/authn/session.rs:190`,
`.await?`), while unknown-user, disabled and already-locked write their audit row
best-effort (`let _`, lines 132, 140, 156). It rated that a username-enumeration
oracle.

On 2026-09-16 `@nabbisen` ruled for option (a), *one uniform response for every
login failure*. Re-reading the code at that ruling showed **(a) already holds over
HTTP**: `login_post` (`crates/sui-id/src/http/handlers/admin/auth.rs`, the `Err(_)`
arm) maps every error from `try_login_with_cascade` — including a store error from
U22 — to the same 401 page with the same message and the same metric. The oracle R11
described is not observable. What *is* true, and was not recorded:

1. **The error is discarded unlogged.** No tracing call in the command runner, in the
   core login path, or in `login_post`'s error arm. An operator cannot see a
   failure-accounting outage.
2. **An audit-write failure silently disables lockout while sign-in keeps working.**
   U22 commits the counter update and its audit row in one transaction, *after* the
   password check; the audit append reads the chain head inside that transaction
   (`crates/sui-id-store/src/repos/audit.rs:110`). If appending to `audit_log` fails,
   every wrong password returns 401 and is not counted. The correct-password path
   writes its audit row best-effort and needs only the session insert, so it still
   succeeds. Nothing is logged.
3. **Nothing tests any of it.**

Found by reading the code, not yet measured at runtime — Part 1 measures it.

## Part 1 — dispatched now (independent of the open decision)

**1a. Measure.** A test that makes the `audit_log` append fail (for example, a
trigger or constraint in a test database that rejects inserts into `audit_log` only)
and shows, through the real handler:
- a wrong password for a known user → 401, same body as an unknown user;
- the failure counter did **not** advance;
- a correct password → signs in.

If any of the three differs from what is written above, stop and report; the
description here is wrong.

**1b. Log the discarded error.** In `login_post`'s error arm, log any error that is
not an ordinary credential failure (`InvalidCredentials`) at error level, with the
request's span and no password material. The external response stays exactly as it
is — the uniformity is the point.

**1c. Lock the uniform response with tests.** For each branch — unknown user, disabled,
locked, known user with wrong password, and a store failure on that last branch —
assert the identical status, body and metric. This makes ruling (a) a tested
contract instead of an accident of one match arm.

## Part 2 — should a login that cannot be audited succeed? Ruled 2026-09-16

**`@nabbisen` accepted the architect's recommendation: fail closed.** Not dispatched:
it waits for the RFC 094 amendment described below. Part 1 is unaffected.

The design as recommended: If the audit log
cannot be written, no login succeeds — the successful-login audit row becomes
must-succeed like U22's, so an attacker cannot guess freely while accounting is
down and then sign in. The cost is availability: during an audit-log outage nobody
can sign in. That matches RFC 094's and RFC 085's standing rule that an
audit-subsystem failure is an operation failure, not a silent gap. It changes how
RFC 094 classifies successful-login bookkeeping (inventory U24), so it is an RFC 094
amendment and is written once ruled.

## Evidence for Part 1

The test from 1a (report what it measured), the tests from 1c, the log line from 1b
captured in a test; fmt, both clippy scopes, `cargo test --workspace` count before
and after, MSRV 1.95; G13 55/55.
