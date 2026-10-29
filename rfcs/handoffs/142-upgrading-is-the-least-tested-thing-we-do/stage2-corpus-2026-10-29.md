# RFC 142 stage 2 — corpus record

**Dispatch.** [stage2-the-corpus-2026-10-29.md](./stage2-the-corpus-2026-10-29.md)
**Landed.** 2026-10-29. 38 fixtures, gate **G22**, local matrix 47 → 48.
**Reviewed and accepted by the architect**, who re-ran the suite (12
tests; 615 in `sui-id-store --lib`), `fmt`, `clippy -D warnings`, A3.4,
G20, the 386 script self-tests and the docs gates, and then **mutated
migration `0049` four times** to establish that the corpus can actually
fail: a deleted row, a corrupted value, an unclassified new table, and
a deleted `revoked_access_tokens` row were each caught — the last from
starting version 5, exactly where the range split predicts, which is
what proves that table is genuinely seeded rather than merely listed.

**One correction to the body below.** §5 describes the `0.29.7` SQL as
*"embedded verbatim (not paraphrased)"*. It is not verbatim — the
blob's comment header is dropped, 5,115 characters against 9,174 in
git. With comments and whitespace normalised out of both, **the
executable SQL is byte-identical (4,264 characters each way)**, so the
fixture is faithful and the historical bug it reproduces is the real
one. The code is right; the word overstated it.

---


**Task / RFC.** RFC 142 stage 2, "build the corpus" — the stage the RFC
exists for; it ships a gate. RFC Accepted; stage 1's measurement
(amended in review to 14 seeded tables) is the input.
**Handoff.** `rfcs/handoffs/142-upgrading-is-the-least-tested-thing-we-do/stage2-the-corpus-2026-10-29.md`
**Baseline.** `cf33777` (`git log -1` at start).
**Round commit** (in a throwaway clone, copied back file-by-file with
`md5sum` verification both ways, clone then deleted): `72242db`.

## 1 — whether any of the 38 fixtures failed on first run (§9)

**The 37-by-replay fixtures and the two `audit_log` fixtures: no.**
Once the generator's own loop bug (below) was fixed and my own test
assertions corrected, `corpus_every_shipped_version_reaches_head_with_data_intact`
passed on its first real run across all 37 shipped versions, and both
`audit_log` fixtures passed first try.

**The hand-built `0.29.7` fixture: yes, and what it found was my own
wrong assumption, not a defect in the migrations.** I asserted
`credentials` would survive the historical cascade (`"credentials is
1:1 with users, not cascaded away by this bug"`) — wrong:
`credentials.user_id REFERENCES users(id) ON DELETE CASCADE` (migration
0001) is its own direct foreign key to `users`, so `DROP TABLE users`
in the original `0021` text cascades `credentials` exactly like
`sessions`/`refresh_tokens`/`user_totp`. The fixture caught this on its
first run with a plain count mismatch (`left: 0, right: 1`). Fixed the
assertion, not the fixture — this is the fixture doing its job, stated
plainly rather than folded into a silent "all passed" claim.

**Not a fixture failure, but a real bug in the generator itself, found
by its own correctness test**: the loop that builds ranges pushed a
duplicate final range (two entries both starting at migration 38) —
`v == MAX_SCHEMA_VERSION` unconditionally pushed a new range even when
the open range from migration 38 was still live. Fixed by having every
non-opening iteration extend the current range's end and table
snapshot, so the loop ending *is* the close — no special case at
`MAX_SCHEMA_VERSION` needed. `derive_ranges_matches_the_accepted_stage1_table`
caught it immediately via a length mismatch (11 ranges expected, 12
produced).

## 2 — the generator: mechanical derivation and its own mutation evidence

`derive_ranges()` (`crates/sui-id-store/src/tests_rfc142.rs`) walks
migrations 1–49 via the existing `migrations::sql_for_version`/
`MAX_SCHEMA_VERSION` — no new helper added to `migrations.rs`, and its
behaviour is untouched (dispatch §7). A hand-rolled scanner, not
`regex` (not a dependency of any crate in this workspace, and the
vocabulary — `CREATE TABLE`, `ALTER TABLE ADD/DROP COLUMN`, `RENAME
TO`, `DROP TABLE`, `CREATE INDEX` — is narrow enough not to need one).

**Produces exactly stage 1's amended table**, independently re-derived
rather than hardcoded to match it: 11 structural ranges (`1,3,4,5,14,
16,22,25,36,37,38` → `2,3,4,13,15,21,24,35,36,37,49`), one of which
(37) has no shipped starting version and needs no seed script — 10
seed scripts, matching the review's correction exactly.

**Mutation evidence for the three "stop and ask" events** — five tests,
each constructing SQL this module never embeds (not trusting that the
real 49 migrations happen to exercise every branch):

1. `analyze_migration_flags_an_unresolved_new_table` — a `CREATE TABLE`
   for a name in neither list is reported as newly created (the
   caller then panics on it, naming the migration).
2. `analyze_migration_flags_a_check_on_an_already_existing_table` — the
   rename-rebuild idiom (migration 0022's shape) on an
   already-in-scope table is flagged.
3. `analyze_migration_does_not_flag_a_self_referential_check_on_a_new_column` —
   proves the negative: a brand-new column's own `CHECK` on itself
   (0017/0027/0034/0035's shape) is never ambiguous and must not ask.
4. `analyze_migration_flags_a_check_on_a_new_column_that_references_something_else` —
   a defensive rule with no real instance in this codebase: a `CHECK`
   on a *different* column than the one just added is still flagged,
   closing the gap the self-referential rule alone would leave for a
   future migration.
5. `analyze_migration_flags_a_drop_column_on_a_seeded_table` —
   migration 0043's shape.

**Mutation evidence that the corpus sweep's own row-count assertion is
not vacuous**: `a_dropped_row_between_seed_and_head_is_caught_not_silently_passed`
seeds R7, then deletes the seeded row before migrating to head (standing
in for a migration that silently drops it, since editing migration SQL
itself is out of scope per §7), and asserts the before/after counts
differ — proving the assertion the main sweep relies on would actually
fire, not merely that it happened not to this round.

## 3 — the 37-by-replay fixtures

One `#[test]`, `corpus_every_shipped_version_reaches_head_with_data_intact`,
looping over all 37 shipped versions (re-derives nothing from tags —
the literal list stage 1 independently confirmed twice, with a
sanity assertion that the list itself still has 37 entries) rather than
37 separate functions, so a failure at one version cannot hide a
different failure at another (every failure is collected, not
short-circuited).

Per version: `run_up_to(start)`, seed via the range's own seed function
(10 functions, cumulative — each range's function seeds *every* table
in scope at that point, not only the delta since the previous range,
because a later-starting fixture must still prove the earlier tables'
data survives), snapshot row counts, `run_up_to(MAX_SCHEMA_VERSION)`,
then assert: every snapshot count unchanged, `users.display_name`
(and `server_settings.default_lang`, where in scope) reads back the
exact seeded marker — **a value, not a count** (dispatch §2) — final
`schema_version == MAX_SCHEMA_VERSION`, and `PRAGMA foreign_key_check`
reports nothing.

**A real trap found and fixed, not merely avoided**: `users.user_uuid`
(added at migration 4 with `DEFAULT ''`) loses that default at
migration 22's rebuild (`NOT NULL`, no default, only `CHECK
(length(user_uuid) = 36)`). A row inserted directly into an
already-migrated database never goes through either migration's own
one-time backfill `UPDATE`. Every seed from range-start 4 onward sets a
real 36-character UUID explicitly (`uuid::Uuid::new_v4()`) rather than
relying on a default that stops existing partway through the chain the
fixture is about to replay — found by reasoning about the schema
history, confirmed by the sweep passing once applied.

**Two migration-owned singleton rows handled correctly, not
re-inserted**: `server_settings` (migration 0016) and `scope_definition`
(migration 0036) each auto-insert their own row(s) in their own
migration SQL. `server_settings`'s seed is an `UPDATE ... SET
default_lang = '<marker>'` (which doubles as the value-level assertion
for that table, since `default_lang` carries no `CHECK`), not an
`INSERT`, which would have collided on the `id = 'singleton'` primary
key. `scope_definition`'s seed is a fresh `INSERT` under a
marker-derived `name`, since its primary key never collides with the
four auto-seeded catalog rows.

## 4 — `audit_log`: two fixtures, not the 14-table corpus (§3)

`audit_log_fixture_at_or_above_schema_9_uses_the_real_writer_and_still_verifies`:
seeds two rows via the real `audit::append_within_tx` at schema 9,
migrates to head, then opens a [`Database`] handle on the same
file-backed path (file-backed, not `:memory:`, specifically so a
`Database` handle can be opened after the raw connection reaches head —
`Database::open`'s own migration run is then a no-op) and calls the
real `audit::verify_chain_tail`: `checked == 2`, `legacy_unhashed ==
0`, `broken_at_seq == None`.

`audit_log_fixture_below_schema_9_preserves_legacy_rows_as_legacy_not_corruption`
— the one the dispatch named as the point, not the leftover: a plain
`INSERT` with no hash columns at schema 1 (schema 9 doesn't have them
yet), migrated across migration 9's `ADD COLUMN ... DEFAULT ''`, then
one real row appended via `append_within_tx` so the legacy row has a
successor to chain against. Three *named* assertions, not implied by
the fixture's mere existence (dispatch's own explicit requirement):
`legacy_unhashed == 1` (treated as legacy, not corruption),
`checked == 1` (exactly the one post-head row is chain-checked), and
`broken_at_seq == None` (verification does not fail wholesale because
history predates hashing).

Neither duplicates `compute_hash` (private to `repos/audit.rs`,
deliberately not re-implemented — the drift risk this round's own
`check-publish-preconditions.py` precedent warned about).

## 5 — the hand-built `0.29.7` fixture (§4)

`ORIGINAL_0021_V0_29_7_SQL` is the exact blob at `68ad4f3`, embedded
verbatim (not paraphrased), including its own ineffective `PRAGMA
foreign_keys = OFF/ON` lines — left in deliberately, since removing
them would stop being a faithful replay of what actually shipped.
Applied inside a bare `tx.execute_batch` (no `FK_DISABLE_REQUIRED`
handling: that marker postdates this text and the real `apply_migration`
never recognized one for it), which is exactly how it ran in production
in 2026-05 and is why it is unsafe.

**The fixture first proves it reproduced the real bug, before testing
recovery from it**: seeds `users`/`credentials`/`clients`/`signing_keys`/
`user_totp` plus two of `users`' *other* children (`sessions`,
`refresh_tokens`) at schema 20, applies the original text, and asserts
`sessions`, `refresh_tokens`, `user_totp` **and** `credentials` are all
wiped to 0 (the found correction, §1), while `users`/`clients`/
`signing_keys` (never FK children of `users`, or not one at all for the
latter two) keep their 1 row. If this block had not failed the way it
did, the fixture would have been testing a sanitized stand-in, not the
real historical shape.

**Then the actual D1 property**: seeds one fresh row per rebuilt table
*after* reaching the 0.29.7-shaped schema 21 (representing an operator
who kept running on the damaged database before ever applying the
fix), migrates to head, and — against a separately-built, normally-
replayed `0.29.8`-origin lineage seeded identically at schema 21 — compares
`PRAGMA table_info` for all five tables, structurally identical in
both. Row counts at head: 2 for `users`/`clients`/`signing_keys`
(pre-21 survivor + post-21 row), 1 for `credentials`/`user_totp` (only
the post-21 row, the pre-21 one having been cascaded away). Final
`schema_version == MAX_SCHEMA_VERSION`; `PRAGMA foreign_key_check`
clean.

## 6 — the gate (§6)

**G22**, registered through the multi-source lane registry (RFC 094
R10), the same mechanism G13/G19/G20/G21 use — not a new, separate
mechanism:

- `rfcs/accepted/142-upgrading-is-the-least-tested-thing-we-do.md`:
  new `## Gate Matrix lane owned by RFC 142` heading with the G22 row,
  mirroring RFC 134/137's exact template.
- `contracts/gate-inputs.toml`: `[rust_components]`, `[gate_lane_sources]
  "142"`, `[gate_owners] G22 = "142"`, `[gates] G22 = "cargo +stable
  test -p sui-id-store --lib tests_rfc142 --locked"`, `[lane_profiles]
  G22` (stable only, mirroring G20's own asymmetry — no MSRV
  counterpart needed, since the logic under test doesn't depend on
  toolchain). `paths = ["**"]`, **not** the Rust-scoped set: RFC 130
  D2's `SCOPED_LANES` is a fixed enumeration (`G01`–`G09b` only);
  `generate-ci-workflow.py --write` refused the Rust-scoped list on the
  first attempt with exactly that reason, so this is corrected, not a
  choice made on first principles.
- `.github/workflows/ci.yml` regenerated via `--write`: **27 → 28
  jobs.**
- `scripts/local-matrix.sh` needed **no separate registration** — it
  derives its gate-id list from `ci.yml`'s own `bash scripts/ci-gate.sh
  <ID>` lines, which the regeneration already produced. **47 → 48
  checks**, confirmed by running it, not inferred.

**A3.4 (`check-gate-inputs.sh --all --policy contracts/gate-inputs.toml`)
passed clean on the first run** after the `paths` correction above —
which is the one thing that did *not* pass first try, and is disclosed
as such rather than folded into "first run clean."

A fallout found running the full matrix, **not** a defect in the
round's own logic: `scripts/tests/test_generate_ci_workflow.py` hardcodes
`"27 jobs"` and `tz = "UTC"`-count `5` as regression literals (its own
comments say exactly when to bump them — "G21 26 -> 27", etc.). Bumped
both to `28`/`6` with the same comment convention, matching the
project's own established pattern for this exact situation.

## 7 — the `CHANGELOG` entry (§8)

One entry, under `[Unreleased]` → `### Changed`, in operator terms (no
mention of `#[test]`, "corpus," "fixture," or any harness vocabulary).
**Caught and fixed a placement mistake before finalizing**: first
written under the already-released `## [0.86.0]` heading by copying
this round's own earlier precedent too literally — `0.86.0` is already
tagged (`acd6a00`), so editing its section would rewrite a shipped
release's notes. Moved to `[Unreleased]`, where work not yet in a
release belongs.

## 8 — what was not done, per §7

No seed SQL/fixtures/gate beyond what §1–§6 describe was the actual
RFC 142 stage boundary (this *is* that stage). No change to `migrations.rs`'s
behaviour, and no new test-only helper was needed — `run_up_to`,
`sql_for_version`, `MAX_SCHEMA_VERSION` were sufficient. Did not widen
to D2/D3/D4 — stage 3 is explicitly gated on this stage's own results.
Did not drop the dead `consents` table — stage 1 found it, this round
leaves it, per the dispatch's own explicit instruction that removing it
before this corpus exists would be backwards. No binary `.db` file
anywhere in the tree — every fixture is generated at test time, in a
`tempfile::TempDir` or `:memory:` connection, nothing committed.

## 9 — public signature changes

**None visible outside the crate's own test tree.** `tests_rfc142` is
`#[cfg(test)]`-gated (not compiled into any release artifact), and its
`pub(crate)` items (`SEEDED_TABLES`, `Range`, `derive_ranges`) are
scoped to `sui-id-store` itself, not re-exported. `crates/sui-id-store/src/lib.rs`'s
only change is the one `mod tests_rfc142;` declaration, in the same
`#[cfg(test)]` block as every sibling `tests_rfcNNN` module.

## 10 — closing

Working tree stays uncommitted, as instructed. Seven files changed (six
modified, one new), matching the committed diff exactly (confirmed by
`git status --short` after copy-back).
