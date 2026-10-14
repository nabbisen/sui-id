# RFC 096-B1 stage 10b — F03, the four guarded outcomes

**Dispatched.** 2026-10-15 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted**,
re-accepted 2026-10-14 with the F03-seam amendment.
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md).
**Prior.** Stage 10a landed `6582bd7`: the verifier, its binding gate, the
candidate family, no writes.
**Release.** This stage is on the path to **cycle D**, which is gated on stage
12, not on a date. Nothing here is rushed by a release.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

This stage writes. It is the first federated MFA stage that does.

## 0 — what the seam decision changed, and what it obliges you to build

`@nabbisen` settled the F03 seam on 2026-10-14 (option A-plus-four) and
RFC 096 `:781-789` now records it. Two consequences bind this stage:

- **F03 cannot take a candidate as a parameter.** `sui-id-store` cannot name a
  `sui-id-core` type. F03 is a `pub` command taking plain, already-verified
  primitives, and **the promotion logic that calls it lives in `sui-id-core`**,
  beside the verifier — this is stage 10a's own measured conclusion, not a new
  one.
- **The fourth part of the decision is yours to build**: a source-scope test
  asserting that nothing in `crates/sui-id/` names F03, in the shape of
  `crates/sui-id/tests/e2e/r094_m2a_db_access_scope.rs`. Read that file first;
  it is the pattern, including a failure message that names the rule and the
  clause it comes from rather than only refusing. The type system cannot hold
  this half, so the test is the whole of the enforcement — write it as if a
  future contributor will meet it with no context.

## 1 — the four outcomes, as the RFC states them

`:812-818`, quoted rather than restated:

> F03 has four guarded Protocol outcomes: `Promoted` performs method-specific
> anti-replay/pending consumption, link observation/bookkeeping,
> `[Fed, local_method]` session and cap; `RejectedStillPending` increments to
> 1–4; `AttemptsExhausted` increments to exactly 5 and terminalizes;
> `Invalidated` terminalizes on authority drift/expiry. Only Promoted emits
> post-commit Class-B sign-in success. Rejection/exhaustion/invalidation emit
> their fixed Class-B observations after their transaction.

And the counter, `:766-772`:

> In one F03 Protocol transaction, a wrong proof guarded on `status=pending`,
> `failure_count < 5`, and unexpired unchanged authority increments by one. New
> count 1–4 returns `RejectedStillPending`; new count exactly 5 sets
> `status=exhausted`, destroys the continuation, deletes bound method
> ceremonies, and returns `AttemptsExhausted`. No sixth increment exists. A
> correct proof may promote only while count is 0–4; whichever guarded
> wrong/correct transaction serializes first determines the equality boundary.

**That last clause earns a test of its own**, in the shape stage 3's claim
boundary got: two transactions racing at count 4, one wrong and one correct,
and whichever serializes first decides. Assert the losing one's outcome, not
merely that one of them failed.

## 2 — three obligations stage 9's and 10a's reviews hand you

Named here so they are not held in memory.

**F03 must *open* `continuation_sealed`** with the row's own current values as
AAD before promoting. Stage 9's sealed empty plaintext is a tamper check and
nothing else; the schema can require the column to exist but cannot require
anyone to open it. `repos::federation_mfa_pending::build_aad` is the function.
**Note it is now `Option<Vec<u8>>`** — stage 10a's R1 — and a terminal row
carries `None` by the table's own CHECK, so the open belongs on the
pending-row branch only.

**Each binding is compared the way the module doc says it arrives.** A cookie
compared to a cookie twice is one mechanism, not two. The verifier already does
this; F03 must not quietly re-derive either digest differently.

**Two open questions you should answer rather than assume**, both recorded in
the stage plan: whether `failure_count = 5` may ever restart, and the absence
of a `UNIQUE` constraint on `federation_mfa_pending.user_id` — can a user hold
two live continuations at once, and if so, what does that mean for the shared
counter? Measure the schema and say what is true; if the answer is a defect,
that is a finding, not a change to make here.

## 3 — the three Class-B events

`:826-830` closes the vocabulary:

> F03 uses `auth.federation.mfa_rejected` with reason
> `totp|recovery|webauthn|malformed|method_mismatch`,
> `auth.federation.mfa_exhausted` with `attempt_limit`, or
> `auth.federation.mfa_invalidated`

None of the three exists yet — `grep` finds zero occurrences in both
`contracts/audit-coverage-matrix.md` and `docs/src/reference/audit-events.md`.
All three are new rows in both, plus the operator guide if it lists the MFA
events, the way stage 9's `auth.federation.mfa_required` was added. **G13 and
G15 both check the pair**, so a row in one and not the other fails.

`Promoted`'s own success event is the fourth and is the existing federated
sign-in success, not a new name — confirm which event that is at source before
writing it, rather than inventing one.

## 4 — what this stage must not do

**No WebAuthn branch.** F06 is stage 11's and the ceremony table has no reader.
`:794-800` describes what F03 does with a ceremony; none of it is buildable
until F06 exists. Leave the branch out, say so, and leave `RejectedReason`'s
`Webauthn` and `MethodMismatch` unreachable as stage 10a left them.

**No handler.** Stage 12 switches `/admin/login/mfa` over. If your promotion
path has no caller yet, say so plainly and name stage 12, exactly as 10a did.

**No release-shaped claim.** Cycle D is gated on stage 12; this stage does not
make the federated MFA path live.

## 5 — contracts

`contracts/write-commands.toml`'s F03 row is `status = "target-absent"`,
`sealed = false`, `files = []`, with `test_id` already naming
`p_f03_promoted`, `p_f03_rejected`, `p_f03_exhausted`, `p_f03_invalidated`.
Flip it to `implemented`/`sealed = true` with real files and real test names,
and make the four `test_id`s the tests that actually exist. G17 and G20 both
read this.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The four outcomes**, each with the guard that makes it exclusive, and the
   equality-boundary test of §1.
2. **§2's three obligations**, each answered with a measurement.
3. **The scope test of §0**, and a mutation proving it fails when a
   `crates/sui-id/` file names F03 — a scope test nobody has seen fail is a
   scope test nobody should trust.
4. **The three events** in both files, and which gate you ran to prove the pair
   agrees.
5. **Per-hunk SHA-256** against the tip you named — `python3.14
   scripts/hunk-hashes.py --baseline <sha>`.
6. **Gate evidence**: the full matrix from a throwaway workspace with its own
   `target/`. **G13, G15, G17 and G20** are the ones this stage moves.
7. **Count reconciliation** from the stage 10a landing commit `6582bd7`:
   `sui-id-core --lib` **186**, `sui-id-store --lib` **462**, `sui-id --lib`
   **560**, `sui-id --test e2e` **532**, `sui-id --test compile_fail` **11**,
   `sui-id-i18n` **19**, `scripts/tests` **349** — measured by the architect on
   that commit.
8. **Anything you think is wrong with this dispatch.** Three of the last four
   rounds found something, including two of my own errors and a factual error
   in an RFC I wrote. That rate is the useful number.
