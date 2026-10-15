# RFC 096-B1 stage 11b — a restart grants no reset

**Dispatched.** 2026-10-15 JST, by the architect.
**RFC.** 096 — **Status: Accepted**, re-accepted 2026-10-14.
**Prior.** Stage 11 landed `749571b`. This stage is a follow-on found while
reviewing it, not a fix round against it: nothing stage 11 built is wrong.
**Why it is its own stage and comes before stage 12.** Stage 12 makes the
federated MFA path reachable. The defect below is unreachable until it does,
and must not be reachable after.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## 1 — the sentence, and what the code does instead

RFC 096 `:756`, in the paragraph describing `federation_mfa_pending`:

> A restart preserves count/status; it grants no reset.

The implementation grants a reset. Measured:

- `federation_mfa_pending` has **no `UNIQUE` on `user_id`** — migration 0047
  has the id primary key and the composite binding index, nothing else.
- `repos::federation_mfa_pending::insert_within_tx` is a plain `INSERT` whose
  `failure_count` comes from the caller, and F02 always supplies a fresh row.
- Nothing deletes, reuses or carries forward a prior pending row for the same
  user.

So a federated sign-in restarted while a continuation sits at
`failure_count = 4` produces a **second row at 0**, with its own independent
budget of five.

**What that costs once stage 12 lands.** The five-attempt cap on a federated
second factor becomes bypassable by restarting the upstream sign-in — five
guesses per restart, indefinitely, for someone who holds the upstream identity
but not the second factor. That is precisely the brute force the cap exists to
stop.

**It is not reachable today**, which is why this is a dispatch in the open and
not a `.git-exclude/` finding: the verifier and F03 have no caller, and MFA
still completes through the generic path under RFC 102 L07's own lockout.

**Two of us missed it.** Stage 10b's package raised the `user_id` question and
answered "RFC 096 never states a cross-row limit"; `:756` states it, and the
architect accepted the answer without checking. Stage 11's dispatch then said
the question "stays open", which was right about scope and wrong about the
design being undecided. Recorded because the next person reading the stage plan
will otherwise find two documents saying it is open.

## 2 — what to decide, and the constraint

`:756` fixes the requirement — a restart preserves count and status — and
leaves the mechanism to you. The shapes worth measuring:

- **F02 reuses the live row**, refreshing its bindings, next target and
  continuation while leaving `failure_count` and `status` alone. Closest to the
  sentence's literal wording. Ask what it means for the three binding digests
  and the sealed continuation, which stage 9 bound to the row's own columns.
- **F02 carries the count forward into a new row** and terminalizes the old
  one. Keeps one row per attempt, which the ceremony's composite FK and the
  24-hour purge already assume.
- **F02 refuses while a live row exists.** Simplest, and the most likely to
  strand a user whose first continuation is still inside its 300 seconds —
  argue that case explicitly before proposing it.

A `UNIQUE (user_id)` constraint is a possible part of the answer, not the
answer on its own: it stops the second row without saying what happens
instead. If you propose a migration, say what it does to a database that
already holds two live rows for one user, because 0047 has been shipping since
stage 8 and the purge only removes expired ones.

**Exhaustion is the case to be most careful with.** A row at
`status = exhausted` holds no continuation (0047's own `CHECK`). Whatever you
choose must not let an exhausted continuation become a fresh pending one — that
is the same reset by another route.

## 3 — what this stage must not do

**No handler.** Still stage 12's.

**No change to F03.** The counter's behaviour within one row is settled and
tested; this is about the second row existing at all.

**No widening of the 300-second lifetime** to compensate.

## What to return

1. **The mechanism you chose, with the two you did not**, and what you measured
   to choose — including the exhausted-row case.
2. **A test that a restart at count 4 cannot produce a fresh budget**, written
   so it fails if the reset comes back.
3. **The migration, if any**, with what it does to an existing database holding
   two live rows for one user.
4. **Per-hunk SHA-256 against the tip you named** — regenerated after your last
   commit, not your first. Stage 11's table was computed before its own
   citation-fix commit and certified the wrong tree; it cost a reviewer the
   work of proving which of seventeen mismatches were real.
5. **The full matrix, all 24 green.**
6. **Count reconciliation** from `749571b`: `sui-id-core --lib` **201**,
   `sui-id-store --lib` **509**, `sui-id --lib` **560**, `sui-id --test e2e`
   **533**, `sui-id --test compile_fail` **12**, `sui-id-i18n` **19**,
   `scripts/tests` **349**.
7. **Anything wrong with this dispatch**, including whether `:756` means what
   this dispatch reads it to mean. If you think it does not, that is the most
   useful thing you could return.
