# RFC 096-B1 stage 11b fix round — the update direction of a cascade

**Dispatched.** 2026-10-15 JST, by the architect.
**RFC.** 096 — **Status: Accepted**, re-accepted 2026-10-14.
**First round.** [`stage11b-a-restart-grants-no-reset-2026-10-15.md`](stage11b-a-restart-grants-no-reset-2026-10-15.md);
result in `.git-exclude/reviewed/rfc-096-b1-stage11b-the-update-direction-of-a-cascade-2026-10-15.md`.
**Baseline.** `81ff2d9`, unchanged — nothing has landed since. Keep the working
tree; it is uncommitted and correct.

One item. The mechanism, the migration and both of your corrections to my
dispatch all stand.

## 1 — (blocking) the refresh updates columns a live ceremony's key will not let it update

Your §1 chose reuse partly on the ceremony's composite foreign key: a new row
"would need a new id, and any ceremony still bound to the old row's id would be
cascaded away." Right about `ON DELETE`. That key has **no `ON UPDATE` clause**
(`0047:240-249`), so `NO ACTION` governs updates to the parent's referenced
columns — and `refresh_for_restart_within_tx` updates three of them:
`provider_id`, `provider_config_version`, `provider_activation_generation`.

Measured:

```
PROBE ceremony insert: Ok(1)
PROBE restart-updates-provider-with-live-ceremony:
  Err(SqliteFailure(ConstraintViolation, extended_code: 787,
      "FOREIGN KEY constraint failed"))
```

A pending row with a live ceremony, restarted through a second linked provider,
is refused by the ceremony's own key, and `issue_federated_mfa_pending` returns
a raw `StoreError` instead of the reuse this stage exists to perform.

This is stage 11b's: before it, F02 only inserted, so no parent row's
referenced columns were ever updated.

**The constraint on the fix.** A restart re-issues the continuation, so an
in-flight challenge bound to the previous round has no claim on surviving it —
"retry requires F06" is already this design's rule. Deleting a live ceremony as
part of the refresh looks right to me and needs no migration, but the choice is
yours to measure and argue.

If you propose `ON UPDATE CASCADE` instead, argue it against what the binding
is for: the ceremony's copied provider triple exists so the ceremony cannot
outlive the authority it was issued under, and cascading makes it silently
follow a change instead.

**Do not special-case `provider_id` alone.** The same failure returns through
`provider_config_version`/`generation` the moment M2b makes them real rather
than the placeholders stage 10a measured at `(0, 0)`.

**It needs a test that fails without the fix** — a live ceremony, a restart
that changes the provider triple, and the outcome you chose asserted
precisely.

## Not in this round

**The federated path has no account-level lockout.** Chasing the above I found
that `record_second_factor_failure` (RFC 102 L07) is called only from
`mfa.rs:249`, the local `verify_pending`; F03, `promote` and the whole
`federated_mfa` module reference no lockout at all. After this stage the
federated bound is five guesses per continuation and a fresh continuation once
one expires — no escalating backoff, no account lock, where the local sibling
path has both. Low severity, and a decision rather than a patch. **Not yours
and not this round's**; it is scheduled.

**The cookie `max_age` point from your own §7** is recorded for stage 12. Leave
it.

## What to return

1. **The fix**, the mechanism you chose, and the one you did not.
2. **The test** that fails without it.
3. **Per-hunk SHA-256** against `81ff2d9`, regenerated after your last commit.
4. **The full matrix, all 24 green.**
5. **Count reconciliation** from your own `525` for `sui-id-store --lib`, and
   the rest from `749571b` as before.
6. **Anything wrong with this round.** You were right twice last time — that
   F02 is reachable today, and that `:756`'s scope is the two statuses and not
   four. Both were corrections to my dispatch and both improved the result.
