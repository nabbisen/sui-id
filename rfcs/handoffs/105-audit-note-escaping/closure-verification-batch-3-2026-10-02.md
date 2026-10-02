# Independent verification — closure review batch 3 (RFCs 105, 112, 115, 116, 118, 127)

**Date:** 2026-10-02
**Reviewing.** [`rfcs/handoffs/105-audit-note-escaping/closure-review-batch-3-2026-10-02.md`](./closure-review-batch-3-2026-10-02.md)
**Not an implementation package.** No tree change accompanies this. `git status --porcelain` is empty before and after.
**Why this exists.** Same reasoning as batches 1 and 2. This review explicitly adopted "name the test by reading it" after this role caught two grep-count errors in batch 2 — worth checking whether the new method actually produced accurate evidence, not just whether the method sounds more careful.

## Headline: no discrepancy found this time

Every named test across all six RFCs was checked for existence by name and run. All exist, all pass. The method correction (reading tests instead of counting matches) produced accurate evidence — a useful data point on its own, after two consecutive batches each turned up a stale or wrong count.

## RFC 115 — the `must_change` clause, the one this batch depended on

Batch 2's closing note said RFC 131 and 131-adjacent work aside, **RFC 115's `must_change` question had to be settled before batch 3 could close** — "still present in `migrations.rs` and two migrations, so which of the two holds has to be established rather than assumed." This review now says "Gone," reversing its own 2026-10-02 flag. Checked directly rather than trusted, since this is the one load-bearing correctness claim in the batch:

- `grep -rn "must_change"` across `crates/` and migrations confirms exactly the shape claimed: the column is created in `0001_initial.sql`, gains a `CHECK` in `0022_boolean_checks.sql`, and is dropped in `0043_drop_credentials_must_change.sql`. The only other occurrences are in test fixtures (`tests_rfc021.rs`, `tests_rfc115.rs`) exercising the historical schema, and `migrations.rs`'s own list of migration filenames.
- The grep-proof test, `r115_s3_must_change_is_gone_from_production_code`, uses `production_sources()` (defined in `r115_stage2.rs`), which walks every crate's `src/` excluding `tests/` dirs and `tests_*.rs`/`tests.rs` files — genuinely production code, not a narrowed scan built to pass. It explicitly carves out `sui-id-store/src/migrations.rs` with a comment explaining why (the migration list names the dropped file by filename, not by using the column) — read, not assumed.
- Ran both named tests plus the schema test: `r115_s3_the_credentials_table_has_no_must_change_column`, `r115_s3_must_change_is_gone_from_production_code` (`cargo test -p sui-id --test e2e must_change`: 2/2 pass), and `tests_rfc115::schema_tests::a_fresh_database_has_no_must_change_column` (`cargo test -p sui-id-store --lib`: pass).

**The reversal is correct.** This was the single blocking question carried over from batch 2's closing note, and it is genuinely settled.

## All other named tests, checked by name and run

| RFC | Tests named | Checked | Result |
|---|---|---|---|
| 105 | 9 | all found in `tests_rfc105.rs`; ran the module | 10 passed (module total), 0 failed |
| 112 | 17 | all found (including both journal-mode variants — `..._in_rollback_journal_mode` and `..._in_wal_mode`, both exist) | ran `tests_rfc112`: 19 passed (module total), 0 failed |
| 118 | 14 | all found in `r118.rs` | ran `cargo test -p sui-id --test e2e r118`: 16 passed (module total), 0 failed |

(RFC 115's are covered above. RFC 116 and 127 are gate/documentation properties, not named unit tests — checked separately below.)

## RFC 116 — gate/structural claims

- `bash scripts/ci-gate.sh G18`: pass, reporting **10 contract files** — matches "nine contract files, and `contracts/README.md` carries a row for each" (10 including the README's own self-row, consistent with the gate's own count).
- `contracts/contract-paths.toml:25` carries `former_directories = ["ci"]`, confirming the `ci/` → `contracts/` move is a checked fact, not a memory.
- A3.4, G13 already confirmed green in prior batches' sweeps on this same commit; re-running was redundant and skipped.

## RFC 127 — the citation claims, both directions

- `r120_routes.rs:460` reads exactly *"RFC 127 D7: the documented table is the checked artefact, not merely a…"* — line number matches precisely.
- `r122_routes.rs` cites RFC 127 D2 and D7 at lines 36 and 137, both pointing at `docs/src/reference/security-surfaces.md`.
- `docs/src/reference/security-surfaces.md` itself names both tests by their full path (`r120_routes::the_routes_that_answer_without_an_actor_are_exactly_the_expected_set`, `r122_routes::the_routes_carrying_the_no_store_layer_are_exactly_the_expected_set`) — the cross-reference is real in both directions, not asserted in one.

**The completeness clause is correctly left unverified here too.** "No security-relevant fact is discoverable only by reading source or tests" is a universal negative; I have no way to check it either, for the same reason the review states. Agreeing with a stated limitation rather than trying to clear it anyway.

## What this does not do

No closure decision. No new finding this batch — unlike batches 1 and 2, nothing here needed correcting. The value of this pass is confirming that the review's own corrected method (naming tests by reading, after two caught errors) held up under an independent re-check, and that the one load-bearing reversal (RFC 115) is solid.

**Entry point of this package:** `.git-exclude/review-requests/closure-review-batch-3-independent-verification-2026-10-02.md`
