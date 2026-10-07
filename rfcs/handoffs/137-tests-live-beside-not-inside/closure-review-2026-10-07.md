# RFC 137 — closure review

**Date:** 2026-10-07. **Recommends closure. Does not close it.** The decision
lives in RFC 137's `Closure approved by.` field, not here.

**Closed 2026-10-07 in `f61c2cd`**, Status Implemented, on the approval that
field records.

*Both lines above were added after the fact. This document originally opened
*"Recommendation: close to `done/`, Status Implemented"* — no false claim, but
no way for a reader to tell from the evidence document that the decision had
happened, and no pointer to where it lives. Found by applying the
post-closure verification pass to all three of today's closures; the pass is
the implementation role's invention, and this is the defect it caught on its
first use against a document written before it existed.*

**Independence.** This review was performed by **the architect, which wrote
RFC 137, all five of its dispatches, and the closure criteria it is being
measured against.** It is therefore **not independent of the work**. RFC 000
provides for exactly this case: with no independent role available, the
accountable owner signs off. `@nabbisen` holds that role here. The implementation role verified its own
measurements separately and those are recorded in each package; that is
corroboration, not independence.

## The closure prerequisite, clause by clause

The RFC's own words: *"Every `crates/*/src/**` file's test module lives in a
sibling file; a gate fails on an inline one; the gate's exemption list is
empty; and every crate's test count is **identical** before and after."*

### 1. Every `crates/*/src/**` test module lives in a sibling file — **met**

Swept independently of the gate, matching `#[cfg(test)]` followed by an inline
`mod`, allowing intervening attributes:

| Crate | Files with an inline test module |
|---|---|
| `sui-id` | **0** |
| `sui-id-core` | **0** |
| `sui-id-i18n` | **0** |
| `sui-id-shared` | **0** |
| `sui-id-store` | **0** |
| `sui-id-web` | **0** |

**All six crates, zero.**

### 2. A gate fails on an inline one — **met**

**G21**, `scripts/check-inline-tests.py`, registered in all three required
places in `contracts/gate-inputs.toml` with `paths = ["**"]`. **Eight
self-tests** in `scripts/tests/test_check_inline_tests.py`, all passing,
covering detection, an inline module behind intervening attributes, a
compliant sibling not being flagged, an exempted module, a stale exemption, an
exemption naming a missing file, an unsorted list, and the real tree.

**The gate was shown non-vacuous by mutation**, not by inspection: breaking
`INLINE_MOD_RE` to match nothing fails **3 of the 8** —
`test_an_inline_test_module_is_detected`,
`test_an_inline_module_with_intervening_attributes_is_detected` and
`test_an_exempted_inline_module_passes`.

**Corrected 2026-10-07, and the reason is worth more than the number.** This
review first said *4 of the 8*, which was the figure measured on 2026-10-06
when the exemption list held 50 entries. **Re-running the mutation today gives
3, and the test that stopped detecting it is `test_a_real_tree_passes`.**

The cause is this RFC's own success. With 50 exempted inline modules in the
tree, a detector that finds nothing makes all 50 exemptions **stale**, and the
stale-entry rule fails the run — so the real-tree test caught the mutation.
**With the list empty, a blind detector and a correct one give the same answer
on the real tree**, and that test can no longer tell them apart.

**G21 is still non-vacuous**: the three fixture-based tests detect the mutation
decisively, and they are the ones that matter, because G21's job from here is
to catch a *new* inline module rather than to re-prove the migration. But the
real-tree test is now a tautology under mutation, permanently, and anyone
reading it as evidence of the detector working should know that.

### 3. The exemption list is empty — **met, and without amendment**

`contracts/inline-test-exemptions.toml` holds **zero** entries.

**This nearly was not met.** Stage 2 deferred `http/id_token.rs` on the grounds
that RFC 096-A would rewrite it, which would have left the list at one and the
criterion unmet. The alternative was amending the prerequisite from "empty",
which is a material change to a prerequisite and returns an Accepted RFC to
`proposed/`. **Stage 5 withdrew the deferral instead** — two tests — so the
criterion is met as written.

### 4. Every crate's test count identical before and after — **met**

| Stage | Crate(s) | Measure | Before → After |
|---|---|---|---|
| 1 | `sui-id-web`, `-shared`, `-i18n` | test attributes per crate | **3/20/19 → 3/20/19** |
| 2 | `sui-id` | attributes; `cargo test --list` | **178 → 178**; **687 → 687** |
| 3 | `sui-id-core` | attributes; `--list` | **173 → 173**; **174 → 174** |
| 4 | `sui-id-store` | attributes; `--list` | **328 → 328**; **336 → 336** |
| 5 | `sui-id` (`id_token.rs`) | attributes, stage-5 scope | **163 → 163** |

**`--list` counts matter more than attribute counts** and D3 says so: a move
that silently drops a module compiles and passes with fewer tests. Both were
taken at every stage from stage 2 on.

**Beyond the counts**, every relocated module was proven **token-identical** by
extracting its body from the baseline commit by brace-matching, de-indenting,
and diffing: 13 of 13 in stage 2, 22 of 22 in stage 3, 19 of 19 in stage 4,
1 of 1 byte-identical in stage 5. The residual differences were rustfmt
rejoining lines at the shallower indent, indentation inside `\`-continued
string literals — identical in value, confirmed by compiling a two-line
program rather than by assertion — and, once, a trailing comma rustfmt dropped
when joining a signature.

## Level B

**`5ed86e9`, run `37557473108` — 27 jobs, 0 skipped, all green.**

Verified to satisfy RFC 131 D2 rather than asserted: every one of the **24**
entries in `contracts/gate-inputs.toml`'s `[gates]` has a job in that run, and
nothing was skipped. The three remaining jobs are `Compute changed scope`,
`A3.2 gate-matrix negative self-tests` and `A3.4 gate-inputs.toml enforcement`.

**An earlier green run was rejected for this purpose.** `4f93823` passed, but
with **11 skipped** jobs — it touched only a test file, so RFC 130's path
filtering removed the Rust lanes. That is Level A. The distinction is the whole
point of RFC 131, and a closure citing it would have been wrong.

**The exemption list is empty at `5ed86e9`**, checked at that commit and not
merely in the working tree.

## What went wrong, recorded because the RFC is otherwise a clean story

**Four of the five stages hit an error of mine, not the implementer's.**

1. **The scope was miscounted three times.** The RFC said 77 inline modules; the
   measured original was **60**. My greps counted files already in the
   compliant form. Later I told the RFC stage 3 was "11 of 18" files needing
   `#[path]`; it was **13 of 18**, because I grepped `lib.rs` alone and two
   modules are declared in `identity/admin.rs`. **The second error was inside
   the amendment whose subject was measuring before asserting.**
2. **The stage-2 method did not compile.** I wrote "exactly the stage-1
   method"; `crates/sui-id/src/lib.rs` declares its modules with `#[path]`, so
   rustc resolves their children against the containing directory and all 13
   files failed with `E0583`. The dev team diagnosed it, applied
   `#[path = "<stem>/tests.rs"]`, **declared it rather than absorbing it**, and
   asked rather than deciding the architecture.
3. **Stage 3's dispatch assumed one test module per file.** Four of eighteen
   hold two. My per-file test counts survived only because I counted
   attributes rather than modules — luck, not method.
4. **Stage 5's gate run died of my own fix.** The `CARGO_TARGET_DIR` guard
   added in `61e1068` forces the build inside the clone; the clone was on a
   30 GB tmpfs shared with another project. I closed one failure mode and
   opened another. The clone location is now stated in dispatches.

**What the implementation role contributed beyond what was asked:** a
negative test of the `#[path]` rule in both directions, proving the form
**necessary** and not merely sufficient; an unstripped body comparison that
would catch a change inside a raw string literal where my whitespace-stripped
method would not; and, in stage 4, reporting that its own first comparison had
covered 10 of 19 modules rather than silently rerunning it.

## Recommendation

**Close to `done/`, Status Implemented.** All four clauses are met on measured
evidence, the gate that holds the result is registered and non-vacuous, and the
one deferral that would have left a clause unmet was withdrawn rather than
legislated away.
