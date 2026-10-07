# RFC 138 — closure review

**Date:** 2026-10-08. **Recommends closure. Does not close it.** The decision
lives in RFC 138's `Closure approved by.` field, not here.

**Closed 2026-10-08**, Status Implemented, on the approval that field records.
This line is maintenance; the sentence above is still exactly what this
document does.

**Independence.** Written by the architect, which wrote RFC 138, its dispatch
and its closure criteria. **Not independent.** RFC 000 provides for that: the
accountable owner signs off. The implementation role built the tool and ran its
own comparison; that is corroboration.

## The four prerequisites

RFC 138's own words: *"One script computes every hash a package declares; its
definition of a hunk is in its own docstring; a self-test fails if a hunk's hash
changes when a later hunk is appended; and every dispatch cites the script
instead of restating the method in prose."*

| # | Result | Evidence |
|---|---|---|
| 1 | **met** | `scripts/hunk-hashes.py` prints per-hunk hashes for modified files and labelled full-content hashes for added files, in paste-ready form |
| 2 | **met** | the rule is in the module docstring, with the reason — a hunk's bytes must not depend on whether another hunk follows it |
| 3 | **met** | `test_hunk_1_hash_unchanged_when_a_third_hunk_is_appended` |
| 4 | **met** | four handoff `README.md` files now cite the script; the stage-4b dispatch was the first written against it |

## The verification that mattered

**The tool agrees with an implementation written independently from the rule**,
on the exact diff that exposed the defect:

| | hunk 1 | hunk 2 (final) |
|---|---|---|
| retired method | `fb7d54fa…` | `21e12820…` |
| the tool | **`62728fdf…`** | `21e12820…` |
| my own computation | **`62728fdf…`** | `21e12820…` |

**Self-agreement was never the question.** A tool that defines its own
correctness proves nothing; two implementations of one stated rule agreeing is
the property RFC 138 exists to restore.

## Beyond the criteria

- **They pinned the defect as a test**:
  `test_the_retired_method_disagrees_with_the_new_one_on_a_non_final_hunk`.
  Simplifying the tool back to splitting on `"\n@@"` now fails and names why.
- **14 self-tests**, covering deleted files, pure renames, renames with content
  changes, a missing final newline, the `\ No newline at end of file` marker as
  hashed bytes, added binary, CRLF, and the CLI as a real subprocess.
  `pytest scripts/tests/` goes 333 → **347**.
- **RFC 138's second finding was three instances, not one.** I named
  `check-rfc-integrity.py:479` as pointing at the retired `roadmap/`; they found
  the invariant-13 docstring summary and the longer docstring too, fixed all
  three, and **checked whether a self-test pinned the old wording** before
  changing it — leaving a comment that narrates the 2026-09-10 to 2026-09-22
  transition as history. **Correct: rewriting accurate history to match present
  wording is the error D3 exists to prevent.**

## D3 held

No declared hash in any existing package was altered. The four READMEs changed
prose only; **no issued dispatch and no submitted package was edited.**

## Level B

**`8a697b1`, run `37630784768` — 27 jobs, 0 skipped, all green.** Verified by me
to cover all **24** entries in `[gates]`, not inferred from the count.

**It could not have evidenced itself.** RFC 138's own commit, `606704a`, touches
no file under `crates/`, so its run would have been path-filtered to Level A —
the same trap as RFC 135. `8a697b1`, the stage-4a commit pushed immediately
after it, carries RFC 138's changes into a full-matrix run. **Two RFCs in two
days have now needed an unrelated commit to produce their evidence**, which is
worth knowing before a third.

## What this RFC actually fixed

**A rule nobody owned.** Per-hunk hashing is how a reviewer establishes that
the package they read describes the tree in front of them, and the method
existed only as a sentence copied between dispatches. Two honest
implementations disagreed invisibly, and the disagreement surfaced only when
someone recomputed a **non-final** hunk — which had not happened until
2026-10-07.

**The false mismatches it was producing had already been misdiagnosed twice**,
both times blamed on the architect's own scripts. Neither diagnosis was wrong;
neither was complete, because nobody examined the convention underneath.

## Recommendation

**Close to `done/`, Status Implemented**, on the owner's sign-off.
