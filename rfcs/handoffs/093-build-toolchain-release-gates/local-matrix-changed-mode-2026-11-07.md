# The local matrix learns what CI already knows: `--changed`

> ## Whose turn it is: **the implementation role's.**
> Cycle J stage 7d —
> [`../094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md`](../094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md)
> §D. Scoped and authorized 2026-11-07. It runs before stages 8–9.

**Dispatched.** 2026-11-07, by the architect.
**Governing RFC.** RFC 093, the same footing as the round that wrote the
script ([`local-matrix-script-2026-10-28.md`](local-matrix-script-2026-10-28.md)).
It reuses RFC 130 D1–D7's selection rule rather than inventing one.
**It changes no gate, no lane, no `[gates]` command and no CI job.**
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 1 — the problem, measured

A full `bash scripts/local-matrix.sh` takes **20.6 minutes**. Four test
lanes take 93% of that: **G06 5.7, G05 4.8, G04 4.5 and G02 4.2 minutes**.
Every other check together takes **1.4 minutes** on a warm cache. The
build cache already works (about four crates recompile per lane), so the
time is test execution, not compilation.

**CI already skips those lanes when they cannot be affected.** The
`changes` job (`scripts/compute-changed-scope.py`, RFC 130 D6) runs G01–G09b
only when a push touches the Rust scope that `[lane_profiles]` declares.
The local script has no such mode, so a commit that touches only an RFC, a
dispatch or a guide still waits 21 minutes for lanes CI itself would not run
on that commit.

## 2 — the rule this reverses, and why it does not reintroduce its defect

The script's header says *"No 'fast subset' mode, deliberately"*, and the
dispatch that created it (§3a) said the same. **That rule came from the
architect's own dispatch, and from nowhere else.** The defect it guarded against
was a **hand-enumerated** local set that silently left checks out (30/30
with fourteen `unittest` steps never run; then thirteen of fourteen).

`--changed` is not a hand-picked subset. **It selects by the rule CI applies,
read from the same source CI reads**, and it states what it skipped. The
header paragraph must be rewritten to say exactly that: what the old rule
guarded against, why this mode does not reopen it, and the three properties
below that keep it closed. Do not just delete the paragraph. The next reader
needs to know a deliberate rule was revisited, not overlooked.

## 3 — the design

### 3a — one chooser, not two

- **What counts as "Rust" is read once**, through `scripts/rust_scope.py`
  `load_rust_scope()`, as today.
- **The path matching moves into `rust_scope.py` as well**: one function
  that takes the changed paths and the scope, and returns the **first
  matching path** or `None`. Both `compute-changed-scope.py` (CI) and the
  local chooser call it. That leaves one copy of the selection logic.
- **The local chooser is a thin new script**,
  `scripts/compute-local-changed-scope.py`, with the **same output contract**
  as CI's: exactly `true` or `false` on stdout, and the reason on stderr.
  `local-matrix.sh` reads it the way the `changes` job reads its sibling.
- **CI's behaviour stays byte-for-byte the same.** The refactor in
  `compute-changed-scope.py` is mechanical. Its existing self-tests pass
  **unedited**. If any of them needs an edit, CI's chooser has changed, so
  stop and say so.

### 3b — what "changed" means locally

The changed set is the **union** of:

1. `git diff --name-only --no-renames <merge-base>`. This compares the base
   to the **working tree**, so it covers committed-unpushed, staged and
   unstaged edits. **`--no-renames` is required**: a rename that moves a file
   *out of* `crates/` must report the old path, or a deletion from the Rust
   scope would skip the Rust lanes.
2. `git ls-files --others --exclude-standard`, the untracked-not-ignored
   files. This is the same listing G18 uses. Untracked files are the trap
   that gave a throwaway clone 48/48 while the real tree was 47/1.

**The base** defaults to `git merge-base HEAD @{upstream}`: everything CI has
not yet run on. `--base <ref>` overrides it.

### 3c — when in doubt, run everything (RFC 130 D7, carried over)

The full matrix runs, with the reason printed, when:

- there is no upstream, or HEAD is detached, and no `--base` was given;
- `git merge-base` or either listing fails, or the root is not a git
  repository;
- the scope cannot be read (`load_rust_scope` raises);
- the lanes `ci.yml` conditions on `needs.changes.outputs.rust` do not equal
  `rust_scope.SCOPED_LANES` (§3d).

**One exception, deliberately:** an explicit `--base <ref>` that does not
resolve is a **usage error (exit 2)**, not a fallback. A person who typed a
ref made a mistake. Silently spending 21 minutes on a full run is the wrong
answer to a typo.

### 3d — which checks may be skipped: derived from `ci.yml`, cross-checked

The script already derives everything from `ci.yml`. It must derive the
skippable set the same way: **the gate ids whose `ci.yml` job carries
`if: needs.changes.outputs.rust == 'true'`**. Today that is G01, G02, G03,
G04, G05, G06, G07, G07b, G08, G09a and G09b, measured with `awk` over the
job headers. **Do not hardcode that list in the shell.**

At runtime, cross-check it against `rust_scope.SCOPED_LANES`. If they
differ, run the full matrix and name the difference (§3c). **Every other
check always runs**: G10a–G23, A3.4, CIGEN, the fixture suites, the mdBook
negative self-test and every `unittest` step. Measure in your return whether
any non-gate `echo "command=…"` step sits inside a scoped job. Today I
believe none does. If one does, it is skipped with its job, because that is
what CI does.

### 3e — the output a reader cannot misread

- **The default (no flag) is unchanged, byte for byte**, including the
  summary line returns already quote (`local-matrix: 47 check(s) run, …`).
  Prove it with a test.
- With `--changed`, one **scope line** goes under the existing banner, before
  anything runs. It is one of:
  - `scope: --changed against <ref> (<short sha>): <n> changed path(s); no Rust-scope path — 11 lane(s) will be skipped: G01 … G09b`
  - `scope: --changed against <ref> (<short sha>): Rust scope touched (first: <path>) — complete matrix`
  - `scope: --changed could not determine the changed set (<reason>) — complete matrix`
- Each skipped lane prints `=== G02 ===` then
  `result=SKIPPED (no Rust-scope change since <ref>)`, in its usual place in
  the order, so the log keeps the same shape.
- The summary reads
  `local-matrix: 47 check(s): 36 run, 36 passed, 0 failed, 11 skipped (unaffected by this change — not verified green).`
  The counts must be computed, never typed. **"Skipped" must never read as
  "passed"**: a skipped lane is unaffected by this change. It is not proven
  green.
- A skipped lane never fails the run, and it never counts as passed.
- `--list --changed` prints each entry prefixed `RUN` or `SKIP`, without
  running anything. That lets someone preview the selection, and it gives the
  tests a seam that needs no cargo.
- `--base` without `--changed` is a usage error.

## 4 — tests

Extend `scripts/tests/test_local_matrix.py`, `test_rust_scope.py` and
`test_compute_changed_scope.py`, so that no new module needs registering. If
you add a module after all, registration lives in
`contracts/workflow-template.toml`, and `ci.yml` must be regenerated (CIGEN
checks that). Use throwaway git repositories made by the test, with a
`--root`, and drive them through `--list --changed`.

**The chooser** (black-box: real git repositories, real files):

1. Only `docs/` changed → the 11 lanes print `SKIP`, everything else `RUN`.
2. A committed-unpushed `crates/**` change → nothing skipped.
3. A **staged-only** change and an **unstaged-only** change, each in
   `crates/**` → nothing skipped.
4. An **untracked** new `crates/**` file → nothing skipped.
5. A **deleted** Rust file, and a **rename out of `crates/`** → nothing
   skipped (this proves `--no-renames`).
6. `Cargo.lock`, `.cargo/config.toml` and `.github/workflows/ci.yml`, each
   changed alone → nothing skipped.
7. No upstream and no `--base` → complete matrix, with the reason in the
   scope line.
8. `--base` that does not resolve → exit 2.
9. A synthetic `ci.yml` whose scoped-job set differs from `SCOPED_LANES` →
   complete matrix, with the difference named.
10. An ignored file under `crates/` (a `*.sqlite`, per `.gitignore`) as the
    only change → skipped. CI never sees it either. State this in the test's
    name.

**Mutations: run them and report them.** A chooser that always says
"changed" passes cases 2–6. One that never says "changed" passes case 1.
Show that each of those mutations, plus dropping `--no-renames` and dropping
the untracked listing, turns at least one test red. **Name which test fails
for which mutation.** Testing the outputs while leaving the chooser
unasserted is the defect class this project has hit before.

**The output:** default-mode output is unchanged (assert the summary line
exactly). The `--changed` summary's counts add up, and `SKIPPED` lanes are
counted apart from both `passed` and `failed`.

## 5 — documentation

- `docs/src/contributing/release-process.md` §"Run `scripts/local-matrix.sh`":
  one short paragraph naming `--changed`, what it skips and on what rule,
  and that **release preparation runs the full matrix**. A preparation bumps
  `Cargo.toml` anyway, so `--changed` would select everything regardless.
  The paragraph removes any doubt. **Name the flag. Do not restate any
  `[gates]` command** (A3.4).
- The script's `usage()` and header (§2).
- **When a return may cite a `--changed` run:** whenever it quotes the scope
  line. For a Rust change, `--changed` *is* the complete matrix. For a
  docs-only change, the scope line states exactly what was not run. Put this
  sentence in `release-process.md`, so dispatches can point at it instead of
  restating it.

## 6 — constraints

- **Do not edit `scripts/ci-gate.sh`**, any `[gates]` entry, `[lane_profiles]`
  or the `changes` job. A3.4 byte-matches `[gates]`.
- **No `CHANGELOG` entry.** This is developer tooling, the same exception
  the script's first round took.
- **A pre-check clone, if you use one, belongs beside the tree on the main
  disk** (for example `/home/nabbisen/Desktop/sui-id/<name>-clone`). It must
  not go under `.git-exclude/tmp/clones/`, because a nested Cargo workspace
  breaks the excluded fixtures, and it must not go in `/tmp`, a 30 GiB tmpfs
  that a cold build overflows. **The gate run you cite is on the real tree,
  after copy-back.**
- No release work: no tag, no `workflow_dispatch`, no Level B claim.

## 7 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage7d-local-matrix-changed-mode-2026-11-07.md`
containing:

- the baseline;
- the `rust_scope.py` refactor, and proof that `test_compute_changed_scope`
  passed unedited;
- how §3d derives the skippable set, and whether any non-gate step sits in a
  scoped job;
- the ten chooser cases and the mutation table (mutation → failing test);
- per-hunk hashes;
- **on the real tree, after copy-back, two runs:** one **full**
  `bash scripts/local-matrix.sh` (the script itself changed, so the default
  path must be shown intact), with its count; then
  `bash scripts/local-matrix.sh --changed`, with its scope line and summary
  quoted verbatim and its wall-clock time. This change touches only
  `scripts/` (not `scripts/ci-gate.sh`) and docs, so the second run is the
  docs-only case, measured on this round's own diff.

**On sizing:** one round, I think: about 150 lines of Python and shell, plus
tests. If the derivation in §3d turns out to need more than an `awk` pass
over job headers, say so before building.
