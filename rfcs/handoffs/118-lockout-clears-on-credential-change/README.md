# RFC 118 implementation handoff

**Governing RFC.** [RFC 118](../../accepted/118-lockout-clears-on-credential-change.md), **Proposed**.
Nothing here is authorized until it is Accepted.
**Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Scheduled.** Release cycle B, **first item**, ahead of RFC 116 stages 1–2.

## The measurements this RFC rests on

Re-run before starting; a disagreement is a blocker.

| Claim | How |
|---|---|
| The backoff reaches 24 h at ten failures and stays | `lockout_backoff`, `crates/sui-id-core/src/authn/session.rs` |
| An active lock refuses sign-in before the password is checked | same file, the `locked_until > now` branch |
| **U10 clears neither the counter nor the lock** | read `consume_and_reset_password` in `commands.rs` |
| **U09 clears neither either** | read `change_password_self` in the same file |
| The only reset is a successful password verify | `clear_lockout`'s call sites |

## One stage

**Rewritten 2026-09-25, after the design review.** The version of this section
written before that review specified a design the accepted RFC now rejects:
"clear `failed_login_count` and `locked_until`" with no carve-out, and a message
that could never have fired. **[RFC 118](../../accepted/118-lockout-clears-on-credential-change.md)
is the authority; build it, not this page's memory of it.** Read D1 and D4 there
before starting, and the [design review](design-review-2026-09-24.md) for why.

### D1 — clear the password lockout, and only that

In the transaction that writes the credential, U09 and U10:

- clear `failed_login_count` — **always**;
- clear `locked_until` — **only when `mfa_failure_count < MFA_FAILURE_LOCKOUT_THRESHOLD`**;
- **never** touch `mfa_failure_count`.

> **The carve-out is the point.** `locked_until` is a **shared column**: L07
> writes the same field from `mfa_failure_count` (`commands.rs:2445`) that U22
> writes from `failed_login_count` (`commands.rs:341`), and nothing records
> which lockout set it. Without the condition, someone holding the user's
> mailbox could reset the password, clear a lock the **second factor** imposed,
> and then guess second-factor codes. **A test must fail if the condition is
> removed.**

### D3 — one helper, not two call sites

`users::clear_password_lockout_within_tx`, beside
`record_password_login_within_tx`, called from both closures. Extend RFC 115's
`r115_s2_credentials_writers_are_the_allowlist` so the production writers of
`credentials` outside setup and `--dev` are exactly the ones that call it, and a
mutation removing a call is caught. **Do not** put the clear inside
`credentials::upsert_within_tx`: setup and `--dev` create new rows with nothing
to clear.

### D4 — the completion response says what was *cleared*

Not "you are locked" — that state no longer exists by the time the flow can
speak. U10 returns a **pre-clear snapshot taken inside its own transaction**;
the handler renders it in the **response to the completion `POST`**, not a
redirect, with `Cache-Control: no-store` and `Referrer-Policy: no-referrer`.
Today's redirect to `/admin/login?reset=ok` is read by nothing, so this is the
first confirmation that flow has ever given. i18n in en, ja and zh-Hans.

Say that sign-in had been refused after repeated failures and is now cleared;
that a further refusal soon may mean someone is trying passwords; and, when a
second-factor lock is **retained**, the **time it lifts** — as a time, never a
count. Never the number of attempts, never the source.

### D5 — the operator keeps a signal

An optional `lockout_cleared=<count>` attribute on U09 and U10, present only
when a counter was non-zero or a lock was live. Two descriptors, so the audit
matrix and `docs/src/reference/audit-events.md` change with it, and G13's count
is unaffected (no new event).

### D6 — the sign-in form is untouched

## Evidence

- **The defect, before the fix:** lock an account, reset its password through a
  valid link, and show sign-in with the new password still refused. **Write this
  first and show it failing.** The design review reproduced it on both paths;
  note that the per-IP limiter (10/min) stopped them reaching a ten-failure
  lock, so use a smaller count and rewrite `locked_until` to simulate elapsed
  time, as they did.
- U09 and U10 each clear; a mutation removing either call is caught.
- **The carve-out:** an account locked by the *second-factor* lockout keeps its
  lock through a reset, and `mfa_failure_count` is untouched. **A mutation that
  drops the condition must be caught.** This is the single most important test
  in the package.
- **Same transaction:** an injected failure before the append rolls back the
  credential **and** leaves the counter and lock exactly as they were. The
  pattern is `u10_injected_failure_before_append_rolls_back_everything`.
- **D4's boundary**, and state how it was tested rather than asserting it:
  every non-success completion input (no token, unknown, replayed, expired,
  revoked, too-short password, breached in `block` mode) returns a body and
  status **byte-identical to the baseline commit's**; `GET /admin/login` with
  any query is byte-identical to without; `POST /admin/login` for a locked
  account matches a wrong password in status, body and metric; and the message
  appears in exactly one response class, once — a replay shows the invalid-link
  page.
- `docs/threat-model.md`: **RFC 115's second residual is narrowed, not
  removed.** The re-lock residual stays and is restated — an attacker can still
  re-lock, but must now do it continuously rather than once. Do not delete the
  bullet.
- Correct the two stale references the review found: `clear_lockout` is dead
  code (the resets are L01, L02, L03, U08), and `runtime/config.rs`'s comment
  claiming `max_lockout` stamps a `Retry-After` describes nothing that exists.

## What to return

A review-request package under `.git-exclude/review-requests/`, in the usual
form: what was built, the evidence table, mutations with results, gates on the
final tree, and per-hunk SHA-256 hashes against the stated baseline.
