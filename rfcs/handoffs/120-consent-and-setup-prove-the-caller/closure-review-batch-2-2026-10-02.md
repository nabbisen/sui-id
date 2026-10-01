# Closure review — batch 2: RFCs 120, 121, 122, 123, 124, 125, 126

**Date:** 2026-10-02
**Reviewed by.** The architect (high-capability model, requirements-architect role).
**Approval required from.** `@nabbisen`. **Not yet given.** Under RFC 000 the
implementer cannot be the sole approver, so nothing here closes until he approves.
**Commit cited.** `fc056df` — **Level B green on it: CI run `36878881472`, 24 of 24
jobs success.** RFC 131 D2's requirement is met by CI on the exact commit, not by a
local run.

**What these seven have in common.** Every one is a behavioural security property
that shipped to users in **0.79.0**, and every one has tests to point at rather
than prose. That is why they are one batch: verifying them means naming the test
that defends each clause, not re-reading a document.

**This review states where it stopped.** Two clauses below are verified by reading
rather than by measurement, and they are marked as such. A closure review that
claims uniform rigour it did not apply is worth less than one that says where the
soft spots are.

## RFC 120 — Consent and the setup wizard must prove who is asking

| Clause | Finding |
|---|---|
| The consent flow derives subject and authentication methods **from the session alone** | Implemented via `consent_state.rs`'s signed cookie; the subject is taken from the live session, and the parameters still carried are integrity-protected and bound to it |
| Every state-changing route reachable after first-run initialization requires an authorized actor, CSRF, a rate limit and an audit row | `crates/sui-id/tests/e2e/r120_routes.rs` holds the route set; `r120.rs` carries 17 more. **RFC 120's own count is 20** — corrected 2026-10-02: this row first said 21, which counts a 4th test in `r120_routes.rs` added later by **RFC 127 D7** (`the_documented_route_table_matches_expected_without_actor`), not by RFC 120. It did not exist at `cd4d137` |
| Each has a test that **fails on the code before this RFC** and passes after | **Measured 2026-10-02 by the implementation role, after this review flagged it as unmeasured.** They checked out `e1a251d` (parent of the implementing commit `cd4d137`) into a disposable worktree, copied the tests in unchanged, and ran them: **14 of 20 failed pre-fix.** The six that passed test properties that were never broken — two happy paths, `/oauth2/consent`'s CSRF which was already enforced, a pre-initialization guard, a meta-test on the lookup table, and the already-protected route set. The aggregate route-set test failed naming **exactly the five** routes the implementing commit claims. All pass on `fc056df`. **The clause holds, read correctly:** "each" binds to the routes and properties, not to all test functions as a flat set |

**Met, with that one clause carried on the record rather than silently.**

## RFC 121 — A verification failure is not a pass

| Clause | Finding |
|---|---|
| No surface reports the chain as intact when verification did not complete | `crates/sui-id/tests/e2e/r121.rs`, 4 tests |
| A failure is distinguishable from a verification that found nothing wrong, on every surface showing either, and the two surfaces agree | Both surfaces are covered by those tests; the RFC was amended on its own design review after that review found the deeper defect (`verify_chain_tail` not verifying linkage at all), which became RFC 125 and shipped first |
| A failure is recorded where an operator will meet it | Present in the operator-facing path |

**Met.** Worth recording: this RFC's closure depends on RFC 125 having landed, and
it did — the two were deliberately sequenced that way.

## RFC 122 — A one-time secret does not travel where it persists

| Clause | Finding |
|---|---|
| No secret shown once is carried in a URL — client secret at creation, rotated secret, **dynamically registered client's secret**, TOTP secret or QR, recovery codes | `r122.rs`, 7 tests |
| No response carrying one is storable (`Cache-Control: no-store`) | Covered, and held to an exact set |
| **A test holds the guarded set of routes to a stated list in both directions** | `r122_routes.rs:102` — `the_routes_carrying_the_no_store_layer_are_exactly_the_expected_set`. "Exactly" is the both-directions property the clause asks for: a route added without the layer fails, and a route listed but no longer guarded fails too |

**Met**, and the both-directions test is the clause that makes this one durable
rather than a snapshot.

## RFC 123 — An endpoint that authenticates a client costs the caller something

| Clause | Finding |
|---|---|
| No unauthenticated caller can make sui-id spend password-hashing work without a limit that stops them | `r123.rs`, 6 tests |
| No endpoint that authenticates a client is reachable without one | Covered by the same set |
| Whether a client id can be distinguished from a non-existent one **by timing** is measured and either closed or recorded as accepted with its reasoning | **Measured, and the measurement is durable:** [`d4-measurement.md`](../123-authenticating-a-client-costs-the-caller/d4-measurement.md), tracked in the repository |

**Met.** This clause is the one I expected to be the gap — a "measure it or record
why not" requirement is the kind that quietly goes unmet — and it is not: the
measurement is a committed file, not a claim in a review.

## RFC 124 — The uniform response must be uniform

| Clause | Finding |
|---|---|
| The recovery request does not reveal whether an address exists by response, status, **or time** | `r124_stage2.rs`, 4 tests |
| The request path's work is **identical in both branches by construction**, so no measurement is needed to defend it | Structural: the handler records the submitted address unconditionally and returns; `ForgotPasswordWorker` does the lookup, throttle, token mint and mail afterwards. There is no branch on the address in the request path to diverge |
| **No screen asserts something that may be false** — and an unregistered address is given a route forward rather than left waiting | Implemented in the screens and locales. **Verified by reading the strings, not by a native reader of all three locales** — the Japanese and Chinese copy is correct as written but has not had a native read, which is a standing item independent of this RFC |

**Met**, with the locale caveat recorded. The structural clause is the strongest in
this batch: it is defended by the shape of the code rather than by a timing test
that could pass by luck.

## RFC 125 — The audit chain must be verified as a chain

| Clause | Finding |
|---|---|
| Detects every single-row rewrite, **including one whose own hash was recomputed** | `repos/audit.rs`: `a_single_row_rewrite_with_its_own_hash_recomputed_is_now_caught`, plus `a_tamper_that_leaves_its_own_hash_stale_is_caught_by_the_row_formula` |
| Every deletion | `a_deleted_row_relinked_to_hide_it_is_caught_by_sequence_continuity` |
| Every truncation | `a_truncated_head_with_no_surviving_predecessor_is_caught` **and** `a_truncated_legacy_prefix_is_caught_even_though_prev_hash_still_matches` |
| Within the window it reports on | `the_window_edge_catches_a_rewrite_just_outside_it` and `the_window_edge_does_not_false_positive_on_a_clean_predecessor` — both directions of the boundary |
| The window it covers is stated wherever a result is shown | Stated on both surfaces (`crates/sui-id/src/cli.rs`, `crates/sui-id-web/src/pages/dashboard.rs`) |
| No document claims a property no test defends | The documents that overclaimed — module docs, the migration comment, `ROADMAP.md`, `docs/threat-model.md` — were corrected when the fix shipped |

**Met, and the best-evidenced RFC in the batch.** 17 tests in `repos/audit.rs`,
with a named test for each attack the prerequisite enumerates, including the two
that the original implementation missed (the recomputed-hash rewrite and the legacy
prefix).

## RFC 126 — Password hashing must not block the request runtime

| Clause | Finding |
|---|---|
| **No request-path call to Argon2 runs on a runtime worker thread** | Verified structurally, not by inspection of call sites: `Argon2` is constructed in exactly one place (`password.rs:25`) and used at exactly two (`:87`, `:110`), both inside `hash_password_sync` and `verify_password_sync` — which are **private**. The only public entry points are the async wrappers that `spawn_blocking` behind a semaphore. **The boundary cannot be bypassed by a caller outside the module**, which is stronger than every call site happening to be converted |
| Every call site is converted | Follows from the above. `backup/ops.rs` also uses Argon2 and is correctly **out of scope** — it derives a backup key, not a request-path credential |
| A test demonstrates that concurrent authentication attempts do not stall unrelated requests | **`r126.rs`, 3 tests** — corrected 2026-10-02 from 4. The architect's count matched `#[tokio::test` four times, one of which is **line 10, a doc comment quoting the attribute**. The three real tests are `d3_dummy_and_real_verification_pay_the_same_thread_hop_cost`, `d5_a_health_check_is_served_promptly_during_a_burst_of_real_login_attempts` and `d5_the_costliest_call_site_does_not_stall_a_health_check_either`. All pass. The deadline is calibrated per run from a measured single-hash cost rather than hard-coded, which is why the second implementation held after the first was reverted for a 30 ms deadline against 52.8 ms of CI jitter |

**Met.** The privacy of the `_sync` functions is the part worth approving on: it
turns "we converted the call sites" into "a call site that bypasses the boundary
will not compile."

## Independent verification by the implementation role

[`closure-verification-batch-2-2026-10-02.md`](./closure-verification-batch-2-2026-10-02.md),
committed beside this review.

**It measured the clause this review admitted it had not**, which is the best
possible answer to a stated limitation: RFC 120's "fails before, passes after",
run against the real pre-fix tree in a disposable worktree. The result holds, and
their reading of the prerequisite is the correct one — "each" binds to the routes
and properties, not to every test function, and a reader taking the flat reading
would wrongly conclude the clause fails.

**It also found two counts of mine wrong**, both corrected above:

- **`r126.rs` is 3 tests, not 4.** I matched `#[tokio::test` four times and one
  match is a doc comment. **This is the fourth time this session I have counted
  grep hits instead of reading them** — after the `hibp.rs` `spawn_blocking`
  citation, the `author` occurrences in batch 1, and RFC 115's `must_change`. The
  error is not the arithmetic; it is treating a match count as a fact about code.
- **RFC 120's own test count is 20, not 21.** The 21st is RFC 127 D7's, added later
  to a file RFC 120 created. Attributing another RFC's test to this one inflates
  the evidence for a closure, which is exactly the kind of thing closure evidence
  must not do.

Neither changes a verdict, and both are findings under this project's own
convention that a wrong declared count is a finding regardless. They also noted
`r122_routes.rs`'s test is at line 101 where this review said 102 — recorded, and
left as immaterial.

**What it is not:** approval. They implemented these RFCs, so RFC 000's rule that
the implementer cannot be the sole approver is satisfied by `@nabbisen`, not by
them. They correctly declined RFC 124's locale caveat as something they cannot
supply either.

## What approval would mean

On `@nabbisen`'s approval, and not before: `Closure reviewed on. 2026-10-02`,
`Closure approved by.` with his words, and `Closure evidence.` pointing here, added
to each of the seven; all seven moved to `rfcs/done/`; `rfcs/README.md` updated;
and the gates re-run, since G11's folder-versus-`Status` conditions change
behaviour when a file moves.

**Seven of the eighteen candidates.** With batch 1's five, twelve of the eighteen
would be closed, leaving batch 3's six — where **RFC 115's `must_change` question
must be settled first**: its prerequisite says "enforced or gone", and it is still
present in `migrations.rs` and two migrations, so which of the two holds has to be
established rather than assumed.
