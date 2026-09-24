# RFC 118 — A credential change clears the lockout, and the user is told

**Status.** Accepted
**Accepted on.** 2026-09-25
**Approved by.** `@nabbisen`, 2026-09-25: "RFC 118 is accepted." Accepted on the
**amended** text: its independent design review returned "accept with changes,
not as written", and the change that matters is D1's second-factor carve-out —
without it this RFC would have opened a second-factor bypass while closing an
availability defect.
**Security review.** Required
**Independent design review.** [Design review 2026-09-24](../handoffs/118-lockout-clears-on-credential-change/design-review-2026-09-24.md) by the implementation role, which authored neither this RFC nor its handoff. It **reproduced the defect end to end on both paths** in a throwaway worktree, and returned one blocker, two high and four medium findings. **Its verdict was accept with changes, not as written**, and it was right on every one: all are resolved below.
**Design prerequisites.** None. [RFC 115](../accepted/115-user-creation-without-a-password.md) is Implemented; this closes the residual its threat-model entry states.
**Implementation prerequisites.** None.
**Closure prerequisites.** **A lock cannot outlive the credential change that made it moot** — for any path that sets a credential, and without disturbing a lock that the second-factor lockout set; a holder of a consumed reset token is told what was cleared, in the completion's own response, and that message is unreachable without such a token; and no sign-in response changes for any account.
**Tracks.** Account availability. Found while reviewing RFC 115 stage 1, 2026-09-24, from the implementer's disclosure that they had not checked whether activation clears a lock.
**Touches.** `crates/sui-id-store/src/commands.rs` (U09, U10), `crates/sui-id-store/src/repos/users.rs` (the one helper), `crates/sui-id-core/src/account/forgot_password.rs`, `crates/sui-id/src/http/handlers/forgot_password.rs`, `crates/sui-id-web/src/pages/`, `crates/sui-id-i18n/`, `ci/audit-coverage-matrix.md` and `docs/src/reference/audit-events.md` (D5's attribute), `docs/src/guides/operators.md`, `docs/threat-model.md`.
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

**D1 — A successful credential change clears the password lockout, and only
that.** In the same transaction that writes the credential, U09 and U10:

- clear `failed_login_count` — **always**;
- clear `locked_until` — **only when `mfa_failure_count < MFA_FAILURE_LOCKOUT_THRESHOLD`**;
- **never** touch `mfa_failure_count`.

**The carve-out is the whole point, and the first version of this RFC did not
have it.** `locked_until` is a *shared* column: the second-factor lockout (L07)
writes the same field from `mfa_failure_count`, and nothing in the row records
which lockout set it. Verified at `commands.rs:2445` against `commands.rs:341`.
So "clear the lock" as first written would have let a holder of the user's
**mailbox** reset the password, clear a lock that the *second factor* had
imposed, and then guess second-factor codes — in exactly the case second factors
exist for: **password and mailbox compromised, second factor intact.** The
reset flow is throttled per IP only, and rotating IPs is free.

**D2 — The justification, corrected.** A lockout exists to stop *guessing*.
Completing a reset proves possession of a single-use out-of-band token; changing
a password proves knowledge of the current one. Both are stronger evidence than
the password attempt the **password** lock was counting, so clearing that lock
costs nothing an attacker did not already have. **That argument does not extend
to the second-factor lock**, which is what D1's carve-out preserves. The first
version of this RFC asserted it for both.

**D3 — One helper, not two call sites.** `users::clear_password_lockout_within_tx`,
beside `record_password_login_within_tx`, called from both closures. RFC 115's
`r115_s2_credentials_writers_are_the_allowlist` is extended so that the
production writers of `credentials` outside setup and `--dev` are exactly the
ones that call it, and a mutation removing a call is caught. The clear is **not**
put inside `credentials::upsert_within_tx`: setup and `--dev` create new rows
with nothing to clear, and a future "import a hash" path might legitimately not
want one.

**D4 — The completion response says what was cleared.** The first version said
the user is told they are *locked*. **That message could never fire**: D1 clears
the lock inside the completion's own transaction, so by the time the completion
flow can speak, the state it would describe is gone. What the user can usefully
be told is what was *cleared*.

U10 returns a **pre-clear snapshot taken in its own transaction** — the counter,
whether a lock was live, and whether a second-factor lock is being kept and when
it lifts. The handler renders it in the **response to the completion `POST`
itself**, not a redirect, with `Cache-Control: no-store` and
`Referrer-Policy: no-referrer`, as the recovery-link issuance page already does.

It says, to a holder of a consumed token: that sign-in had been refused after
repeated failures and that this is cleared; that if they are refused again
shortly, someone may be trying passwords for the account; and, when a
second-factor lock is retained, **the time it lifts** — as a time, never a
count. Never the number of attempts, never the source, and nothing on any
surface without the token.

**A side effect worth stating:** today a completed reset redirects to
`/admin/login?reset=ok`, and **nothing reads `reset`** — so a user who completes
a reset currently receives no confirmation at all. D4 is the first confirmation
that flow has ever given.

**D5 — The operator keeps a signal.** Clearing erases the live evidence that an
account was being guessed when the reset landed; the `auth.lockout` history
survives, but the current state does not, and it is visible only by SQL. U09 and
U10 gain an optional attribute, `lockout_cleared=<count>`, present only when a
counter was non-zero or a lock was live, so `auth.lockout … →
auth.password.reset_completed lockout_cleared=…` reads as a story. This changes
two descriptors, the audit matrix and the reference.

**D6 — The sign-in form does not change.** It keeps its single generic refusal.
Telling an unauthenticated visitor that an account is locked is an enumeration
oracle, and D4 is safe only because it sits behind proof of possession. A change
that makes a lock visible before authentication is out of scope and would be a
defect. Noted beside it: `runtime/config.rs`'s doc comment claims `max_lockout`
stamps a `Retry-After` on a locked response. **Nothing does that** — it is stale,
and making it true would break this decision.

## What this does not do

- **It does not close the denial of service, and the first version of this RFC
  implied it did.** Its closure prerequisite claimed "an unauthenticated party
  cannot prevent a user from signing in with a password that user has just
  set". **No per-account lockout can deliver that.** After a clear the counter
  is zero; three counted failures re-lock at thirty seconds, and an attacker
  who attempts at each expiry keeps the account refused.

  What changes is **who must stay active**. Before: one burst locks the account
  for twenty-four hours and it stays locked while the attacker sleeps. After:
  the lock must be **continuously renewed**, and climbing from zero back to the
  24-hour step takes about **twenty-one hours** of correctly timed attempts.
  That is a large, real improvement, and it is not the same as closing it. The
  residual belongs in `docs/threat-model.md`, stated.
- **It does not change the backoff schedule.** The design review's separate
  recommendation — that the root of the availability problem is a counter keyed
  on the username alone, and that a per-source or known-device dimension would
  let an attacker lock only their own view — is its own RFC, and is recorded
  here so it is not lost.
- **It does not remove `sui-id admin unlock-user`**, which remains the
  operator's path and, unlike a credential change, also clears
  `mfa_failure_count`.

## Corrections to this RFC's own text

Recorded rather than quietly fixed.

1. **It named the wrong function.** The RFC and its handoff said the only reset
   is `clear_lockout`. That function has **no production caller** — only
   comments and one test. The real resets are **L01** and **L03**
   (`record_password_login_within_tx`, which itself refuses a locked user),
   **L02** (which also clears `mfa_failure_count`) and **U08**
   (`admin_unlock`). For an account with a second factor, a correct password
   alone clears nothing; only L02 does.
2. **The counter does not reset when a lock lapses.** So one wrong attempt per
   window renews the lock at the same step — which is why the "continuously
   renewed" framing above is the right one.
3. **The 24-hour ceiling is the default, not a constant.** `[security]
   max_lockout` moves it between fifteen minutes and forty-eight hours.

## Risks

- **D1's carve-out is a condition, and conditions rot.** If a later change gives
  the second-factor lockout its own column — which it should have — the
  condition becomes dead code that reads as a control. Whoever does that must
  remove it in the same change.
- **D4 is the enumeration-adjacent part**, and D6 is the boundary. The review
  traced every completion input and found the message unreachable without a
  consumed token *provided it is produced in the completion's own response*. The
  implementation must keep that property, and the handoff carries the tests for
  it.
- **D5 changes two descriptors**, so it touches the audit matrix and G13's
  count. Sequence it against RFC 116 stage 2, which is also editing that file.
