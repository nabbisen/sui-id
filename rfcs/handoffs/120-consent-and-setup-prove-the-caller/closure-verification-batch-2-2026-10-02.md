# Independent verification — closure review batch 2 (RFCs 120, 121, 122, 123, 124, 125, 126)

**Date:** 2026-10-02
**Reviewing.** [`rfcs/handoffs/120-consent-and-setup-prove-the-caller/closure-review-batch-2-2026-10-02.md`](./closure-review-batch-2-2026-10-02.md)
**Not an implementation package.** No tree change accompanies this. `git status --porcelain` is empty before and after; the pre-fix worktree used for RFC 120's measurement was removed afterward, confirmed clean.
**Why this exists.** Same reasoning as batch 1: the dispatch names a closure-review document, not a "what to build" section — closure actions are the architect's, gated on `@nabbisen`. The review itself names two clauses it did not measure (RFC 120's "fails before" claim, RFC 124's locale caveat); I measured the one that is actually measurable from here.

## RFC 120 — the "fails before, passes after" clause, actually measured

The review stated this clause "is asserted on the implementer's report, not re-measured here." I re-measured it directly: checked out the parent of the implementing commit (`e1a251d`, parent of `cd4d137`) into a disposable worktree, copied `r120.rs` and `r120_routes.rs` from `cd4d137` in unchanged, registered the two modules, and ran them against the pre-fix source.

**They compiled and ran against the old tree — 14 of 20 failed, 6 passed.** That is not a problem; it is the correct shape once you read the RFC's actual prerequisite text rather than the review's compressed paraphrase. The prerequisite says *"every state-changing route requires an actor, CSRF, a rate limit and an audit row. **Each** has a test that fails before and passes after"* — "each" binds to the routes/properties, not to all 20 test functions as a flat set. Measured against that reading:

- **The aggregate route-set test failed, naming exactly the right five.** `r120_routes::the_routes_that_answer_without_an_actor_are_exactly_the_expected_set` panicked on the pre-fix tree listing `GET /setup/hibp`, `GET /setup/lang`, `POST /oauth2/consent`, `POST /setup/hibp`, `POST /setup/lang` — the exact five the implementing commit's message claims. This is the test that defends the route-level clause as a whole.
- **All 6 consent-flow tests that exercise the actual defect failed pre-fix**, including the four stronger ones (`consent_state_is_bound_to_the_session_that_received_it`, `consent_state_the_server_did_not_issue_is_refused`, `consent_state_cookie_is_scoped_to_its_path_and_not_readable_by_script`, `consent_denial_re_validates_its_target_against_the_client`) plus the two simpler ones (`consent_requires_a_session`, `consent_denial_encodes_what_it_appends`).
- **All 7 setup-route tests that exercise CSRF/rate-limit/actor/audit failed pre-fix** (`setup_step_forms_carry_the_csrf_pair`, `setup_steps_require_csrf_from_an_administrator`, `setup_steps_are_rate_limited`, `setup_steps_by_an_administrator_apply_and_audit_the_old_and_new_value`, `setup_steps_refuse_a_caller_with_no_session_once_initialized`, `setup_step_forms_are_not_shown_to_a_caller_with_no_session`, `setup_steps_refuse_a_user_who_is_not_an_administrator`).
- **The 6 that passed pre-fix test properties that were never broken**, not the defect: two happy-path flows (`consent_approval_issues_a_code_…`, `consent_denial_redirects_to_the_registered_target_…`), one already-enforced check (`consent_answer_requires_the_csrf_pair` — `/oauth2/consent` already had CSRF; only the *setup* routes lacked it), a guard-clause test unrelated to the auth defect (`setup_steps_do_nothing_before_initialization`), a meta-test on the lookup table itself (`every_expected_route_has_a_reason`), and a test of the *already-protected* route set (`every_route_classified_as_requiring_an_actor_turns_an_anonymous_caller_away`).
- **All 21 tests pass on current `HEAD` (`fc056df`)**, confirmed directly (`cargo test -p sui-id --test e2e r120`: 21 passed, 0 failed).

**Conclusion: the clause holds, read correctly, and the review's caution was warranted but the claim survives.** A reader taking "each" to mean all 21 test functions would wrongly conclude the clause fails; it is "each defect/property," and every one of those has its failing-before/passing-after test.

**One precision note.** The review's "21 tests for this RFC" (4 + 17) counts today's `r120_routes.rs`, which now holds a 4th test — `the_documented_route_table_matches_expected_without_actor` — added later by **RFC 127** (its own docstring says so: *"RFC 127 D7: the documented table is the checked artefact"*), not by RFC 120. It did not exist at `cd4d137` (confirmed: `git show cd4d137:crates/sui-id/tests/e2e/r120_routes.rs` has 3 tests, not 4), which is why my pre-fix run totaled 20, not 21. RFC 120's own test count is 20 (3 + 17), not 21; the 21st belongs to a different, later RFC that happens to share the file. Doesn't affect RFC 120's closure — the clause is about routes and properties, satisfied either way — but the count in the review's table is off by one for the reason stated, not by chance.

## RFC 121, 122, 123, 125, 126 — test counts and named tests, checked directly

| RFC | Claim | Measured |
|---|---|---|
| 121 | `r121.rs`, 4 tests | `cargo test -p sui-id --test e2e r121`: 4 passed, 0 failed |
| 122 | `r122.rs`, 7 tests | file has 7; `r122_routes.rs`'s `the_routes_carrying_the_no_store_layer_are_exactly_the_expected_set` exists at line 101 (cited as `:102`, off by one, immaterial) |
| 123 | `r123.rs`, 6 tests; D4 measurement tracked | `cargo test`: 6/6 pass; `git ls-files rfcs/handoffs/123-authenticating-a-client-costs-the-caller/d4-measurement.md` confirms it is tracked |
| 124 | `r124_stage2.rs`, 4 tests | `cargo test -p sui-id --test e2e r124_stage2`: 4 passed, 0 failed |
| 125 | 17 tests in `repos/audit.rs`, 7 named | `grep -c '#\[test\]\|#\[tokio::test\]' crates/sui-id-store/src/repos/audit.rs` = 17; all 7 named tests (`a_single_row_rewrite_with_its_own_hash_recomputed_is_now_caught`, `a_tamper_that_leaves_its_own_hash_stale_is_caught_by_the_row_formula`, `a_deleted_row_relinked_to_hide_it_is_caught_by_sequence_continuity`, `a_truncated_head_with_no_surviving_predecessor_is_caught`, `a_truncated_legacy_prefix_is_caught_even_though_prev_hash_still_matches`, `the_window_edge_catches_a_rewrite_just_outside_it`, `the_window_edge_does_not_false_positive_on_a_clean_predecessor`) confirmed present by exact name |
| 126 | **`r126.rs`, 4 tests** | **Measured: 3, not 4.** `grep -n '^async fn \|^fn '` and `cargo test -p sui-id --test e2e r126 -- --list` both agree: `d3_dummy_and_real_verification_pay_the_same_thread_hop_cost`, `d5_a_health_check_is_served_promptly_during_a_burst_of_real_login_attempts`, `d5_the_costliest_call_site_does_not_stall_a_health_check_either`. No fourth test exists in this file or under this filter. |

**The RFC 126 count is wrong and should be corrected before this closes** — same category as batch 1's "twelve `author` occurrences" (accurate once, now stale or simply miscounted), and this project's own convention treats that as a finding regardless of whether the underlying clause still holds. It does still hold: all 3 tests pass, and the structural privacy claim below is independently sound regardless of the count.

## RFC 126 — the structural privacy claim, checked at the cited lines

Read `crates/sui-id-core/src/authn/password.rs` directly rather than trusting the description:
- `argon2()` (line 25) is the sole construction site.
- `hash_password_sync` (line 80) and `verify_password_sync` (line 108) are both private (`fn`, not `pub fn`), and `argon2()` is called inside each at **lines 87 and 110** — exactly the lines the review cites.
- The only public entry points, `hash_password` (line 69) and `verify_password` (line 96), both `spawn_blocking` the private sync function after acquiring `hash_semaphore()`. A caller outside the module has no path to either sync function directly.

The claim is correct, down to the exact line numbers.

## Not re-verified

**RFC 124's locale caveat** (Japanese/Chinese copy not natively read) — the review already states this is "a standing item independent of this RFC," not a blocker, and native-fluency judgment isn't something I can supply either. Left as the review states it.

## What this does not do

No closure decision, no RFC 131 involvement (different batch). It adds one precision finding (RFC 126's test count) and a full, direct measurement of the one clause the review flagged as unmeasured (RFC 120), with the result that the clause holds under its actual wording.

**Entry point of this package:** `.git-exclude/review-requests/closure-review-batch-2-independent-verification-2026-10-02.md`
