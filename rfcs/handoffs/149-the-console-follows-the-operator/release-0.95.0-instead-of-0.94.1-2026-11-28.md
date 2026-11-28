# The fix as `0.95.0` instead of `0.94.1`: assessment

**Raised.** By `@nabbisen`, 2026-11-28: *"How about bringing it as 0.95.0 instead ? 0.94.1 will be skipped, dropped, if
reasonable enough."*
**Assessed.** 2026-11-28, by the architect, at `ba7c4a6` (`main`) and `d36468a` (`0.94.x`).
**Held outside the repository** while no release carried the audit-export fix
([finding](audit-csv-injection-2026-11-28.md)); published with `0.95.0`.
**Outcome.** `0.95.0` was tagged at `93cc6a5` and published on 2026-11-28 (publish run `46136922636`). `0.94.1` was never
tagged or published.

**Decided.** By `@nabbisen`, 2026-11-28: *"0.95.0 now (Recommended)"*, with the four conditions below.
**Dispatched.** The release preparation, the same day, on `da5949e`.

## Verdict: reasonable, and the architect recommends it, on four conditions

## What `0.95.0` would be, measured

- **`main` since `0.94.0`**: 18 commits, 126 files. Q1 to Q4 have landed, and Q5 is reviewed and held (52 of 52).
  **RFC 149's four closure conditions are met.** So `0.95.0` would be a whole release: the console redesign, complete.
- **No schema change**: `git diff --name-only 0.94.0..HEAD -- crates/sui-id-store/src/migrations/` is empty. Taking the
  console redesign to get the fix needs no migration. That is the main measured fact behind the verdict.
- **A relying party would see almost nothing new.**
  - Discovery's output is unchanged: `STANDARD_SCOPES` is the same four scopes, now one constant.
  - One error description changes its wording, the one for a scope an app does not allow.
  - The *Confidential client* box now works.
  - RFC 152 is not built, so none of its relying-party changes would be in it.

## Against the philosophy

| | `0.94.1` as prepared | `0.95.0` now |
|---|---|---|
| **Clean, robust** | One merge commit in `main`'s otherwise linear history for ever, and a window with *"Require linear history"* off | **Linear history kept, and no protection switched off.** The fix is cherry-picked onto `main` as one commit with its test |
| **Safe, secure** | **Fastest**: minutes after the rule is off | Hours later: Q5 lands, the dev team prepares the release, then Level B (about 40 minutes) and the tag. The fix is already public on `0.94.x`, so every hour counts. That is condition 1 |
| **An operator on `0.94.0`** | A one-line patch, the easiest upgrade to take | The fix comes with the redesigned console. **No migration and no relying-party change**, so the risk is in the reading, not the upgrade. The upgrade note says so first |
| **A relying-party developer** | `0.95.0` would later carry the console and RFC 152 together | **RFC 152's changes get a release of their own (`0.96.0`), with an upgrade note only about them.** `0.95.0`'s notes can also announce D4 one release ahead, for free (RFC 152's question 3) |
| **Anyone reading the repository** | Consistent | A public `0.94.x` branch says `0.94.1` in its CHANGELOG, and no such release exists. Condition 3 removes it |

**The precedent agrees.** `0.87.1` and `0.87.2` were cut from `main`, *"slightly wider"* than the fix, rather than
working around the ancestry check (`rfcs/handoffs/094-transactional-audit/release-0.87.1-preparation-2026-11-03.md`). Here
the release is much wider than the fix, and still needs no migration.

## The four conditions

1. **Cut now, not at the end of cycle Q.** Q5 lands first. Then the release preparation goes to the dev team at once.
   Nothing else waits in front of it.
2. **RFC 152 moves to `0.96.0`, and cycle R to `0.97.0`.**
   - Cycle Q becomes RFC 149 only. Q6 to Q8 become the next cycle's stages, unchanged.
   - It changes a schedule that was already authorized, so it needed confirming too.
   - The records follow: the ROADMAP, the cycle plan, and the three-themes plan's §5.
3. **The `0.94.x` branch is deleted from GitHub once `0.95.0` is published.** The fix lives on `main` as the cherry-pick.
   Deleting a remote branch cannot be undone from here, so it is confirmed before it is done.
4. **planeter gets a two-line correction.** Our reply of 2026-11-28 said RFC 152 was *"planned for `0.95.0`"*, and it
   would now be `0.96.0`. The letter called it a plan, so this keeps it true. The draft goes to `send/draft/`.

## What it does to the prepared work

- **`0.94.1` is never published.** Nothing on crates.io or in any tag names it. Its Level B runs (CI `46104350256`, audit
  `46104352721`) become moot.
- **The fix commit `99f15dc`** is cherry-picked onto `main`, as `fix(security):` with its test
  (`e2e::audit_csv_export`). The duplicate CSV test in Q5's `q5_lists.rs` is dropped.
- **The CHANGELOG** names the fix in `0.95.0`'s *Security*, with the affected range `0.36.0`–`0.94.0`. The summary line
  names it too, as the release-summary rule requires.
- **The local merge `57dbb6b`** is discarded. It was never pushed.

## What does not change

The finding is published with the release, in the same way. The release mechanics are the same: a signed annotated tag,
`publish.yml`, and a check that every crate downloads from `static.crates.io`.
