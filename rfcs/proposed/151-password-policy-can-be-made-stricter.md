# RFC 151 — The password policy can be made stricter, never weaker

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted. Built in cycle P, as a
freeze exception decided 2026-11-15 (ROADMAP, cycle P's row).
**Closure prerequisites.** An administrator can raise the minimum password
length from the *Sign-in* settings. No setting can make it lower than 12, and
a test fails if any path enforces a lower one. Changing it is one Class-A
command whose audit record names the old and new value. Every page that asks
for a new password states the current minimum. Each is tested, with a
mutation.
**Tracks.** The operator-experience track, Track D: one of two features
given a freeze exception.
**Touches.** `crates/sui-id-store/src/repos/server_settings.rs`, a migration,
`crates/sui-id-store/src/commands.rs`, `crates/sui-id/src/http/handlers.rs`,
`crates/sui-id/src/http/handlers/settings.rs`, the settings and password
pages in `crates/sui-id-web`, `crates/sui-id-i18n/src/locale/*.rs`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/151-password-policy-can-be-made-stricter/README.md`](../handoffs/151-password-policy-can-be-made-stricter/README.md)

## Summary

Raised by the owner, 2026-11-15: *"you can't even change the minimum length
of passwords."* Measured on `ddcf4b0`:
- **The minimum is fixed at 12.** Production always runs
  `SecurityLevel::Standard` (`crates/sui-id/src/runtime/state.rs:121-135`),
  whose minimum is 12 (`crates/sui-id-core/src/security.rs:51-56`). Dev mode
  uses 8.
- **It is read in three live places**:
  - `handlers::password_min_len` (`handlers.rs:968`), which both the
    password-reset and self-service change paths call (`forgot_password.rs:216`,
    `me_security/password.rs:121`);
  - the *Authentication* settings tab, which displays it read-only
    (`handlers/settings.rs:235`);
  - setup's first administrator (`crates/sui-id-core/src/setup.rs:153`), which
    is created before any setting exists.
- The enum was a refactor of two constants, not a ruling that the policy
  stays fixed (CHANGELOG `:3775-3807`).

## Decision

### 1 — one setting, stricter-only

- **`server_settings.password_min_len`**, an integer, added by a migration
  with the value **12**, so an upgrade changes nothing.
- **Allowed values: 12 to 128.** 12 is the floor, because it is today's
  standard and this RFC must never make sui-id weaker than it ships. 128 is a
  ceiling against a mistyped value that would lock everyone out of changing
  their password.
- **The floor is enforced in two places.** The command refuses a value below
  12, and the store's own check refuses a row below 12, so a direct write
  cannot weaken it either.

### 2 — one command, audited

**`settings.password_min_len.changed`**, a Class-A command on RFC 094's seam,
in the same shape as `change_idle_session_timeout`
(`crates/sui-id-store/src/commands.rs:5400`). Its audit note records the old
and new value. The value is not secret, so it is a direct command, **not** RFC
090's pending-change confirmation, which is for secret payloads such as the
SMTP password.

### 3 — what it applies to, said plainly

- **It applies to every new password**: a reset, a self-service change, and an
  administrator's recovery link. `handlers::password_min_len` reads the
  setting, and every one of those paths already goes through it. The recovery
  link lands on the same `/reset-password` completion as a reset
  (`crates/sui-id-core/src/account/recovery_link.rs:5`, `:62`).
- **Setup keeps 12.** The first administrator is created before the setting
  exists.
- **Existing passwords are not invalidated.** A user whose password is
  shorter than a newly raised minimum keeps signing in, and meets the new
  minimum at their next change. The settings page says this next to the
  field, because an operator raising the minimum might otherwise believe
  every password now meets it.
- **Dev mode is unaffected** (8, as now).

### 4 — the minimum is stated wherever a password is chosen

Every page that asks for a new password shows the current minimum as visible
hint text under the field (RFC 144: hints, not placeholders): *"At least N
characters."* The setting lives under **Sign-in** (RFC 149 §1), where today's
read-only *Authentication* tab becomes editable for this one value.

### 5 — tests

- The floor: the command refuses 11, the store refuses a row of 11, and 12
  is accepted. **Mutation:** lower the floor to 8, and the test fails.
- The application: after raising the minimum to 16, a 15-character reset and
  a 15-character self-service change are both refused, and 16 is accepted.
- **No path enforces a hard-coded minimum.** A test fails if
  `SecurityLevel::Standard.password_min_len()` is used for enforcement
  outside setup. **Mutation:** add such a use.
- The audit record names the old and new value.

## What this RFC does not do

It adds no other password rule: no character classes, no expiry, no history.
Each would be a separate decision. Current guidance (NIST SP 800-63B) is
against forced composition rules and periodic expiry, and sui-id already
checks breached passwords, if HIBP is on. It also does not change HIBP's
mode, which has its own command already.

## Alternatives rejected

- **Allow lowering below 12.** Rejected. Track D's password policy is
  stricter-only, as recorded in the ROADMAP's cycle P row (2026-11-15).
- **Invalidate passwords below a raised minimum.** Rejected. It would lock
  users out on a settings change, through no action of theirs, and break
  sign-in for an administrator who raised it.
