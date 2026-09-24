# RFC 118 — A credential change clears the lockout, and the user is told

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None. [RFC 115](../accepted/115-user-creation-without-a-password.md) is Implemented; this closes the residual its threat-model entry states.
**Implementation prerequisites.** None.
**Closure prerequisites.** No successful credential change leaves a lockout standing, for any path that sets a credential; an unauthenticated party cannot prevent a user from signing in with a password that user has just set; and a user who is refused immediately after setting their password is told **why**, on the surface where they have already proven possession, without that message becoming an account-enumeration signal anywhere else.
**Tracks.** Account availability. Found while reviewing RFC 115 stage 1, 2026-09-24, from the implementer's disclosure that they had not checked whether activation clears a lock.
**Touches.** `crates/sui-id-store/src/commands.rs` (U09, U10), `crates/sui-id-core/src/account/forgot_password.rs`, `crates/sui-id-core/src/account/me_security.rs`, `crates/sui-id/src/http/handlers/forgot_password.rs`, `crates/sui-id-web/src/pages/`, `crates/sui-id-i18n/`, `docs/threat-model.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/118-lockout-clears-on-credential-change/README.md`](../handoffs/118-lockout-clears-on-credential-change/README.md)

## Summary

An unauthenticated party who knows a username can lock an account and keep it
locked. Measured: `lockout_backoff` reaches **24 hours at ten failures** and
stays there, and nothing resets the counter except a successful password
verify — which the lock itself prevents. **No credential change clears it:**
not U10 (reset completion), and not U09 (self-service change) either.

So a user can set a new password and still be refused, and the refusal they see
is **"invalid credentials"** — for the password they chose seconds earlier.

For a newly created account this is worse than an inconvenience. RFC 115 made
an administrator-issued link the only way to activate one, and a link lives
thirty minutes. An attacker who locks the account for twenty-four hours makes
every link expire before it can be used to sign in. The account is activated
and unusable, repeatedly.

## Why this is a rule, not a patch to U10

The first framing of this defect was "U10 forgets to clear the lock". That is
the symptom. **U09 does not clear it either**, so the real state of the system
is that lockout has no defined relationship to credential changes at all: it is
cleared only as a side effect of the very operation it blocks.

Fixing U10 alone would leave the identical hole in self-service password
change and guarantee that the next path to set a credential inherits it.

## Decisions

**D1 — Any successful credential change clears the counter and the lock.** U09
and U10 both, in the same transaction that writes the credential, so there is
no window where the password is new and the lock is old. The precedent is RFC
102 stage 9, which extended `admin_unlock` to clear the second-factor count
rather than leaving two unlock paths to drift.

**D2 — The justification, stated so it can be argued with.** A lockout exists
to stop *guessing*. Completing a reset proves possession of a single-use token
delivered out of band; changing a password proves knowledge of the current one.
Both are strictly stronger evidence than the password attempt the lock was
counting. An attacker who can satisfy either does not need the lock lifted —
they already hold the account.

**D3 — The user is told, where possession is already proven.** A user refused
immediately after setting their password is told that the account is
temporarily locked from earlier failed attempts and when it lifts — **on the
completion flow**, which they reached by presenting a valid token, not on the
sign-in form.

**D4 — The sign-in form does not change.** It keeps its single generic refusal.
Telling an unauthenticated visitor that an account is locked is an enumeration
oracle, and D3 is safe only because it is behind proof of possession. **A
change that makes the lock visible before authentication is out of scope and
would be a defect.**

## What this does not do

- It does not stop an account being locked. Anyone who knows a username can
  still cause a lockout; that is the lockout working. What it stops is the lock
  outliving the credential it was protecting.
- It does not change the backoff schedule. Whether 24 hours at ten failures is
  the right ceiling is a separate question, not reopened here.

## Risks

- **D1 removes a brake.** After it, an attacker who obtains a reset token can
  clear a lock they themselves caused. They hold the token, so they hold the
  account: nothing is lost that was not already gone. Stated because the
  reasoning must be checked, not assumed.
- **D3 is the enumeration-adjacent part**, and D4 is the boundary. The review
  should test that the new message is reachable *only* with a valid consumed
  token, and that no timing or response difference on the sign-in path follows
  from it.
