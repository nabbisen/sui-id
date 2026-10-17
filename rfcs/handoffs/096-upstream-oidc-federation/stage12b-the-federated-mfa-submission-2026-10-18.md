# RFC 096-B1 stage 12b — the federated MFA submission gets a caller

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. The account-lock item that held this stage was
> settled by the architect on 2026-10-18 and is a constraint below, not a
> question —
> [`federated-lockout-request-2026-10-17.md`](federated-lockout-request-2026-10-17.md).

**Dispatched.** 2026-10-18 JST, by the architect.
**RFC.** 096 — **Status: Accepted**.
**Why it exists.** `FederatedMfaVerifier` (stages 10a, 11) and F03
`complete_federated_mfa` (stage 10b) are built, tested and reachable from no
production code. 12a made the verified identity pipeline authoritative; 12c
fixed the token exchange. This is the last piece of stage 12 and cycle D's
remaining gate.
**Baseline.** Read the tip with `git log -1`, hash against it, name it.

## 1 — what is live today, and what this stage must not break

Measured on `9110996`, each step re-checked for this dispatch:

- `complete_federated_signin` issues **both** rows on one response —
  `federation.rs:1575` `issue_pending_mfa` (the generic
  `login_pending_mfa` row) and the F02 federated row — and sets both cookie
  sets together (`:1593-1636`).
- `/admin/login/mfa` is the only MFA route (`router.rs:82`). Its handler
  `mfa_challenge_post` reads `PENDING_MFA_COOKIE` (`admin/auth.rs:617`) and
  calls `sui_id_core::mfa::verify_pending` (`:628`). **There is no federated
  branch.**
- `verify_pending` calls `record_second_factor_failure` on a wrong proof
  (`core/src/authn/mfa.rs:249`), and the handler's comment at `:659` says so.

So **a federated user's wrong second factor locks the account today**, through
the generic path. This stage switches that traffic onto the verifier. Doing
that without carrying the lock forward would *remove* a protection that
currently ships.

**That is the whole of why item 3 below is not optional and is not a design
choice.** It is not a new control; it is the control that is already there.

## 2 — scope

Wire the federated submission into `/admin/login/mfa`:

1. The handler branches on `FED_MFA_PENDING_COOKIE`
   (`federation.rs:159`, `sui_id_fed_mfa_pending`) being present and parsing.
   Present and valid → the federated path. Absent → the existing local path,
   unchanged.
2. Federated path: `federated_mfa::verify()` (or `verify_webauthn()` for a
   `PublicKeyCredential` body), then `complete_federated_mfa()` with the
   candidate it returned.
3. Map F03's four outcomes onto responses:
   - `Promoted { session, evicted, method }` → session cookie, clear all four
     federated cookies **and** the generic pending-MFA pair, redirect to the
     row's `next_path` (same `starts_with('/')` filter the local path uses at
     `admin/auth.rs:643`).
   - `RejectedStillPending` / `AttemptsExhausted` → the uniform failure
     response, **plus item 3**.
   - `Invalidated` → the uniform failure response; clear the federated
     cookies, since the row is terminal and no retry can succeed.
4. `VerifyError::Binding` and `VerifyError::Internal` both produce the uniform
   failure response. `Internal` additionally logs with the request id, the way
   `admin/auth.rs:663-674` already does — same shape, same reason (RFC 102 C1,
   R11 1b).
5. `FederatedMfaOutcome::Expired` is not F03's to infer: hand it to
   `complete_federated_mfa` and let F03's own durable recheck write
   `Invalidated`. Do not short-circuit it in the handler.

**Out of scope:** anything 096-B2 (provider/link commands, C17/C18/C23), any
change to the verifier or to F03's guarded outcomes, and the generic
`issue_pending_mfa` call at `federation.rs:1575`. **Leave the dual-row
emission in place for this stage** — removing the generic row is a separate
change with its own blast radius, and keeping it is what makes the lock
continuous across the switch rather than re-established after it.

## 3 — the account lock. Decided, not open.

**The federated rejection path must feed `record_second_factor_failure`,**
so both doors to an account share one escalating bound and one lock.

**Where.** There is a precedent in this codebase and I recommend following it
rather than inventing a shape: `admin/webauthn.rs:162` calls
`sui_id_core::mfa::record_second_factor_failure` from the handler, after the
rejection, outside the main transaction. Do the same here, on
`RejectedStillPending` and on `AttemptsExhausted`.

**Not inside F03.** F03 counts the *row's* `failure_count`; L07 counts
`users.mfa_failure_count`. They are different counters with different
lifetimes, and folding one into the other would make F03's four guarded
outcomes depend on a user-level control RFC 096 `:812-818` does not give it.
If you measure a reason to prefer a transactional fold, return it as a finding
with the measurement rather than building it — but I do not expect one.

**The DoS objection is answered, so do not re-raise it without new
measurement.** Feeding the shared counter lets a federated attacker lock the
victim out of local sign-in. Measured, that is symmetric rather than new:
`session.rs:229-232` only issues a pending-MFA row after the password check
succeeds, so reaching the local second factor requires factor one, and
reaching the federated one requires holding the upstream identity. Both
attackers have already proven a first factor. The federated door is a
different first factor, not a cheaper one.

**The test that proves it.** A wrong federated second factor advances the
same `users.mfa_failure_count` a wrong local one does. Assert the counter's
value before and after, not merely that the request failed — a test that only
checks the response cannot tell the hook's absence from its presence. **Then
mutate it away**: delete the `record_second_factor_failure` call and confirm
the test fails. If it still passes, the test is not testing this.

## 4 — the cookie's lifetime

`FED_MFA_PENDING_COOKIE` and its two siblings are set with a flat
`max_age` of `PENDING_LIFETIME_SECS` (300s,
`repos/federation_mfa_pending.rs:122`) at `federation.rs:1599-1636`.

The row's `expires_at` is **not** refreshed on a restart:
`refresh_for_restart_within_tx` (`:266-297`) updates the provider columns, the
continuation, `next_path` and the two binding digests, and touches neither
`expires_at` nor `failure_count`. That is stage 11b's fix and it is correct —
it is what stops a restart buying a fresh budget of five.

**The consequence:** a restarted continuation re-sets the cookie to a fresh
300s while the row keeps its original clock, so **the cookie can outlive the
row**. A user who restarts late then submits against a row F03 will
`Invalidate` for expiry, and sees the uniform failure with no indication that
the window simply closed.

This is a usability defect, not a security one — the row is authoritative and
F03 refuses correctly. **Fix it by deriving the cookie's `max_age` from the
row's own `expires_at` rather than from the constant**, on both the initial
issue and the restart. If that turns out to need a row read the handler does
not already have, say so and leave it; do not add a read to the hot path for
a cosmetic gain. Record which you did and why.

## 5 — the copy a user actually reads

Half 2 of the owner's philosophy — *"APIs and UI/UX for users not to be
confused or misunderstand are also very important"* — applies to this stage
more than to anything else in 096-B1, because this is the first federated
failure a human sees.

`RejectedReason::MethodMismatch` is selected when the submission substitutes a
different method than the row is bound to (RFC 096 `:741-743`). In
**direction 2** — the row is bound to WebAuthn and a TOTP code arrives — the
current `mfa_challenge_failed_flash` is *"Verification failed. Try again, or
use a recovery code."* (`i18n/src/locale/en.rs:241`). That is actively
misleading: trying again will not help, and a recovery code is not what the
row is waiting for.

**Add one string** that says the row is waiting for a passkey, in all four
locales (`en`, `ja`, `zh_hans`, `zh_hant`), and select it for direction-2
`MethodMismatch` only.

**What the copy must not do:** distinguish *wrong proof* from *no such row* or
from any other binding failure. The uniform failure response exists because
those must be indistinguishable. `MethodMismatch` is safe to surface because
the user has already bound the row and the fact is about their own
submission, not about whether an account exists — but say nothing that
narrows the others. If you think a line of copy crosses that, return it as a
question rather than guessing.

## 6 — what to return

The usual package, plus:

- The **before/after counter values** from §3's test, quoted, and the result
  of the mutation check.
- Which of §4's two outcomes you took, and the measurement behind it.
- The four locale strings, so the copy can be read without opening the diff.
- **A `CHANGELOG` entry in this dispatch's own commit** — standing since stage
  12. Do not leave it to a catch-up.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the **last**
  commit of the round, not the first.
- The full gate matrix. This touches `crates/`, so it is Level B: 27 jobs, 0
  skipped, all 24 `[gate_owners]` entries matched both ways.

**One open dispatch at a time**, as always. This is the only one.

## 7 — what this closes

Stage 12 complete, and with 12a (`57b4ead`) and 12c (`744e699`) already
landed, **cycle D's last gate**. It does not close M4-B: 096-B2 remains
blocked on RFC 094 M2b, which is unstarted with 43 commands unconverted.
