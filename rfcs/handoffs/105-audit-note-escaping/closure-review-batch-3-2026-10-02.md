# Closure review — batch 3: RFCs 105, 112, 115, 116, 118, 127

**Date:** 2026-10-02
**Reviewed by.** The architect (high-capability model, requirements-architect role).
**Approval required from.** `@nabbisen`. **Not yet given.** Under RFC 000 the
implementer cannot be the sole approver, so nothing closes until he approves.
**Commit cited.** `fc056df` — **Level B green on it: CI run `36878881472`, 24 of 24
jobs.** Every implementation below is present at that commit.

**The last of the eighteen candidates.** With batches 1 and 2 closed, approving
these six would take `accepted/` from 11 to 5, leaving only the four blocked on
real work (094, 095, 096, 117) and RFC 131, held on its own unenforceable clause.

**Method note, and it is a correction of my own practice.** Four times this
session I counted `grep` matches and reported the number as a fact about code —
twice caught by the dev team. **This review names tests by reading them.** Where a
clause is satisfied, the test that defends it is named; where I could not establish
something by reading, it says so instead of producing a count.

## RFC 105 — Audit notes: escape attribute values

| Clause | Test that defends it |
|---|---|
| A value cannot introduce a key/value boundary | `no_hostile_value_introduces_a_pair_boundary_under_any_split`; `a_forged_pair_cannot_be_matched_by_a_substring_query` |
| The encoding is reversible | `every_hostile_value_round_trips_exactly`, plus the proptest `any_value_round_trips_and_never_splits` |
| …and minimal, touching only what it must | `the_encoding_touches_only_what_it_must`; `keys_are_never_encoded` |
| **Historical rows are not rewritten** | `a_row_written_before_the_encoding_still_reads_last_occurrence_wins`; `a_historical_value_with_a_bare_percent_is_read_literally`; `a_token_with_no_equals_sign_is_skipped` |
| Documented beside the builder; every reader agrees | `encode_note_value` is documented at the builder, and the readers named in the clause are the operator guide's queries, `docs/src/reference/audit-events.md`, and the audit page |

**Met.** The historical-rows clause is the one worth approving on: three tests
pin that pre-encoding rows still read correctly, which is what makes this a
forward-only change rather than a migration.

## RFC 112 — Refuse to run against a database this build does not understand

| Clause | Test that defends it |
|---|---|
| Refuses a database **newer than its ceiling** | `the_ceiling_is_exact`; `an_overflowing_stamp_is_reported_as_too_new` |
| Refuses one **carrying application tables with no readable version** | `a_populated_database_with_no_version_is_invalid_and_names_the_table_count`; `a_populated_database_whose_version_row_is_gone_is_refused_and_the_row_is_not_recreated`; `a_foreign_sqlite_file_is_refused_not_migrated` |
| **Before it writes anything — in both journal modes** | `a_too_new_database_is_refused_and_untouched_in_rollback_journal_mode` **and** `…_in_wal_mode`. The clause names both modes explicitly and both exist |
| The operator is told in **one line, first and alone on stderr**, naming both versions, the path and the route back | `a_too_new_line_names_the_path_both_versions_the_route_and_what_not_to_do`; `without_a_stamp_the_line_says_a_newer_sui_id`; `an_invalid_line_names_the_path_the_detail_and_the_table_count`; and `a_line_is_one_line_whatever_a_path_or_a_stored_value_contained` — which defends "one line" against a path or stored value that itself contains a newline |
| A failed version read is **a refusal, not a re-run** | `every_unreadable_stamp_is_invalid_not_a_version`; `a_read_error_is_returned_not_collapsed_to_a_version`; `every_unreadable_stamp_is_refused_and_leaves_the_true_version_recorded` |
| Every reader of the stored version uses the same reader and rule | `a_recorded_canonical_version_is_read_as_that_number`; `only_the_two_schema_refusals_are_a_refusal`; `both_refusal_variants_share_one_exit_code`; `a_refusal_survives_anyhow_context_and_downcasts` |

**Met, and the best-tested RFC in this batch.** Every clause has a test named for
the property rather than for the code path, including the two I expected to be
thin: "one line" survives an adversarial path, and both refusal variants share
exit 65 by test rather than by coincidence.

## RFC 115 — Creating a user without choosing their password

| Clause | Test that defends it |
|---|---|
| No path lets an administrator **set** a password | `r115_s2_credentials_writers_are_the_allowlist`; `r115_s2_creating_a_user_on_the_web_sets_no_password_and_leads_to_issuance` |
| The only route to a new account's password is the U37 recovery link | `r115_s2_a_created_user_activates_through_the_issued_link_with_their_own_password`; `r115_s3_the_new_user_form_warns_an_administrator_who_cannot_issue_the_link` |
| …audited, step-up, second-factor gated and **throttled** | `r115_s3_bulk_creation_is_not_capped_at_five_an_hour`; `r115_s3_provisioning_cannot_be_used_to_reach_an_existing_account_unthrottled`; `r115_s3_an_administrator_created_on_the_web_is_not_exempt` |
| A created account cannot be activated by any unaudited path, **including `/forgot-password`** | `r115_d1_forgot_password_gives_a_never_activated_account_the_neutral_response`; `r115_d1_an_account_that_has_held_a_password_is_unaffected`; `r115_d8_a_never_activated_sign_in_fails_like_a_wrong_password_and_is_counted`; `r115_d8_a_never_activated_sign_in_costs_an_argon2_verify` |
| **`must_change` is enforced or gone** | **Gone.** Migration 0043 drops it; `r115_s3_the_credentials_table_has_no_must_change_column` and `r115_s3_must_change_is_gone_from_production_code` — the second a grep-proof over production files. **I flagged this clause as unsettled on 2026-10-02 and was wrong**: the occurrences I saw were the migration that removes it and the historical migrations it undoes |
| No form's `Debug` can print a password or any other secret | `r115_s3_no_form_field_holds_a_secret_in_a_plain_string`; `r115_s3_the_dead_dto_secrets_are_deleted` |

**Met.** The residual the clause names — that an issuer can complete the link they
issued — is stated in D7 and RFC 103's threat-model entry, which is what the
prerequisite asks for: a stated residual, not a silent one.

## RFC 116 — Gate contracts: one source, one gate each

| Clause | Finding |
|---|---|
| Every surviving file in `contracts/` is read by a gate that fails when it stops being true | Nine contract files, and `contracts/README.md` carries a row for each — **10 rows against 10 files, checked in both directions by G18** (`scripts/check-contracts.py`), so an unlisted file and a listed non-file both fail |
| No fact in `contracts/` exists in a second hand-maintained place | A3.4 holds `[gates]` to RFC 093's table byte-for-byte; the generator makes `ci.yml` derived rather than maintained; RFC 131 D4's checker now stops documents restating a gate's command |
| The audit matrix's `class` and `actor` columns are checked against the code, keyed on **table rows** rather than on names anywhere in the file | G13 (`scripts/check-audit-matrix.sh`), row-keyed |
| Every lane runs through one dispatcher or its exception is a dated decision | All 21 lanes dispatch through `scripts/ci-gate.sh`; the `changes` job added by RFC 130 is the one non-lane job and is not a gate |
| **The placement of what survives is settled** | `ci/` → `contracts/`, with `former_directories = ["ci"]` recorded so the move is checked rather than remembered |

**Met.** Two clauses here are judgments rather than tests — "no fact exists in a
second hand-maintained place" and "the placement is settled" — and I record them as
established by the mechanisms above rather than by a test naming them. RFC 131 D4,
landed after RFC 116, strengthened the first of the two.

## RFC 118 — A credential change clears the lockout, and the user is told

| Clause | Test that defends it |
|---|---|
| A lock cannot outlive the credential change, **for any path that sets a credential** | Both paths, separately: `r118_a_reset_link_lets_the_user_sign_in_after_a_lockout` and `r118_a_self_service_change_lets_the_user_sign_in_after_a_lockout` |
| …**without disturbing a lock the second-factor lockout set** | `r118_a_reset_does_not_lift_the_second_factor_lock` |
| A holder of a consumed reset token is told what was cleared, in the completion's own response | `r118_the_completion_response_says_what_was_cleared`; `r118_an_account_with_nothing_to_clear_gets_a_plain_confirmation`; `r118_the_locale_of_the_request_picks_the_language` |
| **That message is unreachable without such a token** | `r118_the_message_appears_once_a_replay_gets_the_invalid_link_page`; `r118_the_sign_in_page_ignores_any_query`; `r118_the_reset_form_itself_says_nothing_about_lockout` |
| **No sign-in response changes for any account** | `r118_every_refused_completion_is_the_same_whether_or_not_the_account_was_locked`; `r118_it_never_shows_a_count_or_a_source`; `r118_a_breached_password_in_block_mode_is_refused_alike`; `r118_a_refused_completion_clears_nothing` |

**Met.** The unreachability clause is the one that matters for disclosure — three
tests close the three ways a caller might reach the message without a token — and
`r118_the_lock_can_still_be_renewed_but_from_zero` confirms clearing is not
disabling.

## RFC 127 — Documentation carries the truth; a test enforces it

| Clause | Finding |
|---|---|
| Each security-relevant fact has a stated home in `docs/` | `docs/src/reference/security-surfaces.md` is that home |
| The test that enforces it **cites** that home | `r120_routes.rs` and `r122_routes.rs` both cite it; `r120_routes.rs:460` carries *"RFC 127 D7: the documented table is the checked artefact, not merely a…"* |
| The document **names the test** that keeps it true | Present in `security-surfaces.md` |

**Met for the facts it covers — with the completeness clause stated honestly.**
"No security-relevant fact is discoverable only by reading source or tests" is a
claim about *everything*, and I cannot verify a universal negative by reading. What
I can say is that the mechanism exists, is cited in both directions by the two
route tests, and that RFC 127's own additions to `r120_routes.rs` are what made
batch 2's test-count correction necessary — the mechanism is live enough to have
changed another RFC's evidence. **If `@nabbisen` wants the completeness clause
established rather than exemplified, that is a measurement I have not done and
should not be approved as though I had.**

## What approval would mean

On approval, and not before: the three closure fields on each of the six, with his
words; all six moved to `rfcs/done/`; `rfcs/README.md` and every cross-reference
updated; and the gates re-run — batch 1 proved that moving a file breaks
same-directory links no grep for `accepted/` will find, and G11 is what catches it.

Status would read **`Implemented (v0.79.0)` for all six**, and the reasoning
matters because a first instinct here is wrong.

Measured against `CHANGELOG.md`: 112, 116 and 118 are recorded under 0.79.0; 127 is
not in the changelog at all (it is internal discipline) but its implementation is
an ancestor of the `0.79.0` tag; and **105 and 115 are recorded under 0.78.0.**

The instinct is therefore to stamp 105 and 115 with `v0.78.0`. **That would name a
release no user can install.** 0.78.0 was tagged 2026-09-24 and **never published
to crates.io** — abandoned on `@nabbisen`'s ruling of 2026-10-01, recorded in
`CHANGELOG.md`. The registry went 0.77.0 → 0.79.0. So the first release in which
105's and 115's work reached a user is 0.79.0, the same as the other four.

`Implemented (vX.Y.Z)` should name a version someone can point at, not a tag that
exists only in git. All six read 0.79.0, and the fact that two of them landed in
the abandoned 0.78.0 cycle is recorded here rather than encoded into a field that
cannot carry it.
