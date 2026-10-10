# RFC 096-B1 stage 3 — the claim, and one-time nonce consumption

**Dispatched.** 2026-10-10 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
authorized 2026-10-09. This is its stage 3. *(Its "F03" annotation was wrong —
F03 is federated MFA submission. Corrected in the plan 2026-10-10.)*
**Prior.** Stage 0 (`ReadConn`), stage 1 (migration `0046`), stage 2 (`F07`,
attempt start) complete.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## This is where "the rule" becomes "genuinely one-time"

RFC 096 `:67-68` splits the nonce deliberately: 096-A delivered *"the nonce
validation rule"*, and *"the durable attempt state that makes a nonce genuinely
one-time is 096-B1, because it is a mutation."* Stage 7 of 096-A built
`validate_nonce` — a constant-time digest comparison taking the expected digest
as a parameter, with **zero production callers**. This stage supplies that
parameter from the row and makes the claim single-use.

## Three things carried in from earlier reviews, not to be rediscovered

**1. The claim must be a conditional `UPDATE`, not a read then a write.** Stage
1's review measured this: the table `CHECK` makes `status` and `claimed_at`
*agree*, but `UPDATE ... SET status='pending', claimed_at=NULL` on a claimed row
**succeeds** — a row `CHECK` cannot see where a row came from, so nothing in the
schema makes the transition one-way. Single-use is a security property
(`:590-592`).

So the claim is one statement: `UPDATE ... SET status='exchanging',
claimed_at=?1 WHERE id=?2 AND status='pending'`, and **the affected-row count is
the authority**. One row means this caller claimed it; zero means someone else
did, or it was never `pending`. A `SELECT` followed by an `UPDATE` loses that
regardless of how the two are ordered.

**2. The fail-closed clock check is yours, and you identified it.** Stage 2's
package said the real regression check — *"reject a claim where
`now_at_claim < created_at`, not just rely on `expires_at <= now_at_claim`"* —
belongs where a second clock sample first exists. **I confirmed that, and this is
that stage.** `expires_at <= now` alone is the wrong test under a backward
clock: it makes the window look *less* elapsed, not more, so it fails open.

**3. Opening the verifier *is* the tamper check.** Stage 2 binds the AAD to
`id`, `provider_id`, `provider_config_version` and
`provider_activation_generation`, all columns on the row. Reconstruct the AAD
from the row you just claimed and open the sealed verifier: if any of those four
has changed since the seal, the open fails. That is not a separate check to add
— it is the one you get for free, and a test should assert you rely on it rather
than re-comparing the fields by hand.

## Scope

- **The claim**, `pending -> exchanging`, as above.
- **Nonce consumption**: reconstruct the AAD, open the verifier, and call
  096-A's `validate_nonce(claims, expected_digest)` with the row's
  `nonce_sha256`. **Note the shape mismatch and decide it**: `validate_nonce`
  takes a 64-character lowercase hex `&str` and refuses anything else, while
  `nonce_sha256` is a 32-byte `BLOB`. Converting at the boundary is fine;
  changing either side is a decision to argue, not to take quietly. (096-A's
  review also recorded that `is_64_hex` accepts uppercase while `sha256_hex`
  emits lowercase — a deferred tidy, not this stage's, but worth knowing before
  you choose the conversion.)
- **Its manifest row.** Same reasoning as `F07`: a durable mutation, so
  registered; Class **P** unless you can argue otherwise; `sealed = false` while
  the write goes through `with_conn` like every existing Class-P command. The id
  is yours to assign on the precedent now established — `U37` by RFC 103 stage
  3, `O05`/`O06` by RFC 124, the `L` family by RFC 102 stage 2. **Note that
  stage 2's own citation for this was wrong** (`U30`–`U33` are RFC 094's own);
  the three above are the ones that hold.

## Not in scope

**No code exchange, no session, no identity mapping.** Stage 4 routes the
callback through 096-A's validators; stages 5 and 6 map and establish. This
stage claims the attempt and consumes the nonce — nothing downstream of that.

**Do not touch `handlers/federation.rs`.** Still forbidden; stage 7 is what
earns it.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The conditional `UPDATE`**, with a test that two concurrent claims yield
   exactly one winner. Not two sequential calls — a genuine race if the harness
   can express one, and if it cannot, say so and pin the affected-row-count
   logic directly.
2. **A test that a claimed row cannot be re-claimed**, and one that a
   non-`pending` row cannot be claimed at all.
3. **The clock-regression test**: `now_at_claim < created_at` refused, with its
   own error, distinct from "expired".
4. **A test that the verifier open is doing the tamper check** — alter one bound
   column, then claim, and assert the open fails rather than a hand-written
   field comparison catching it.
5. **The `validate_nonce` boundary decision**, argued.
6. **Mutation evidence** per rule — including one that replaces the conditional
   `UPDATE` with a read-then-write and shows what stops being true.
7. **Per-hunk SHA-256** against the tip you named.
8. **Gate evidence** from a throwaway clone with its own `target/`, plus
   `cargo test --workspace` and **G17**.
9. **Anything you think is wrong with this dispatch.** Three of the last four
   packages corrected something of mine.
