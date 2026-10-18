# RFC 096-B1 stage 12b fix round — the row the old path consumed

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. Two items, both small, both additions to a
> build that is otherwise right.

**Dispatched.** 2026-10-18 JST, by the architect.
**RFC.** 096 — **Status: Accepted**.
**Baseline.** The uncommitted 12b tree. Leave it uncommitted; the architect
commits on acceptance.
**Review result.** The full reading is in the review package; this dispatch
carries everything needed to act.

## Why this round exists

The 12b dispatch named the account lock as the protection stage 12b must not
drop, and that item is honoured. **There was a second protection in the same
place and the dispatch did not name it.** That omission is the architect's.

## 1 — the generic pending row is no longer consumed

Before 12b, a federated second factor completed through `verify_pending` →
`complete` → L02, whose first act is

```rust
crate::repos::login_pending_mfa::consume_within_tx(tx.tx(), pending_id, user_id, now)?;
```

`commands.rs:3415`. Its body (`repos/login_pending_mfa.rs:73-76`) is a
`DELETE`, guarded on id, user and expiry. **The row was destroyed at
completion.**

After 12b, the `Promoted` arm clears the `sui_id_pending_mfa` cookie and
nothing touches the row. L02 is never reached on this path, so the
`login_pending_mfa` row **survives a successful federated sign-in** for
whatever remains of its five minutes (`core/src/authn/mfa.rs:38`).

A pending row stands in for factor one. One that outlives the sign-in it was
issued for is a credential left usable after the thing it authorised has
already happened.

**Severity is low** — the cookie is `HttpOnly` and already cleared, a live
second factor is still required, and the row's clock is not refreshed. It is
defence in depth. It is also the same class as the lock: *12b must not remove
a protection that currently ships.* The dispatch's "leave the dual-row
emission in place" was about **issuing** the row, for the stated reason that
keeping it is what makes the lock continuous. It said nothing about declining
to consume it.

**Build.** `repos::login_pending_mfa::delete` already exists (`:95`). Call it
in the `Promoted` arm, keyed off the same `PENDING_MFA_COOKIE` the failure
path already parses, best-effort with a `tracing::error!` on failure — the
identical shape the §3 lockout hook already uses, for the identical reason:
a different crate, a different lifetime, and no business inside F03's own
guarded transaction. **If you measure a reason to prefer folding it into
F03 after all, return the measurement rather than building it.**

**Test.** A successful federated promotion leaves no `login_pending_mfa` row
for that user. Assert the row count before and after — not the `Set-Cookie`
header, which is already asserted and would pass with the row intact. Then
delete the call and confirm the test fails.

## 2 — `next_path` is implemented and untested

The `Promoted` arm reads the row's `next_path` and redirects there. That is
correct, and the field is live rather than vestigial: `federation.rs:557`
captures `?next=/…`, `:585` writes it to the attempt row, `:1646` carries it
into the pending row, and neither the completion write
(`repos/federation_mfa_pending.rs:420`, which nulls only
`continuation_sealed`) nor `find_by_id` (`:308-326`, no status filter) loses
it.

**No test asserts it.** `a_correct_federated_totp_code_promotes_and_clears_every_pending_cookie`
checks for `303` and a session cookie and never reads `Location`. Replace the
whole `find_fed_pending_row(…).and_then(|row| row.next_path)` expression with
the literal `"/admin"` and the suite stays green.

Landing someone on `/admin` instead of where they were going is exactly the
confusion the second half of this project's design philosophy forbids, and it
would ship silently.

**Build.** Nothing — the code is already right.

**Test.** Drive the flow with `?next=/admin/clients` and assert the `Location`
header. Then mutate the lookup to the literal and confirm it fails.

## 3 — out of scope, explicitly

**Do not widen this round to `/admin/login/webauthn/{start,complete}`.** Your
§5 scope decision is confirmed: that path still completes through the generic
row, and `admin/webauthn.rs:162` already locks a wrong assertion there. Item 1
is about the row's consumption on the **code** path only.

## 4 — what to return

1. Item 1's fix, its before/after row-count test, and its mutation evidence.
2. Item 2's `Location` assertion and its mutation evidence.
3. **A fourth `CHANGELOG` entry for item 1**, and an amendment to the headline
   entry: it says switching to F03 "would, on its own, have silently *removed*
   an existing protection" and names only the lock. There were two. Say so —
   the entry is otherwise exactly the right shape.
4. Fresh hunk hashes against the **last** commit of this round.
5. The full matrix. Both items touch `crates/`, so Level B.

Leave the tree uncommitted, as before.

## 5 — what the original round got right

Recorded because a fix round should not read as a verdict on the whole:
`MethodMismatch` really is direction-2-only and the new copy is safe —
verified at `federated_mfa.rs:441-448`, where it is selected only when a live
ceremony row exists. §4's cookie fix covers the initial issue and the restart
by construction rather than by two parallel edits. The hunk-hash artifact
covers exactly the real diff. The baseline count of 559 is right and the
architect's carried 558 was the stale one.
