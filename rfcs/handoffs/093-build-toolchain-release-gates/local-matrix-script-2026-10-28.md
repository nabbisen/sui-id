# The local matrix becomes a script

> ## Whose turn it is: **the implementation role's.**
> Cycle H item 4 —
> [`../094-transactional-audit/cycle-h-and-0.86.0-2026-10-28.md`](../094-transactional-audit/cycle-h-and-0.86.0-2026-10-28.md)
> §B. Scoped and authorized 2026-10-28.

**Dispatched.** 2026-10-28 JST, by the architect.
**Governing RFC.** RFC 093, **Implemented** — it owns the Gate Matrix
and `contracts/gate-inputs.toml`. This adds a local runner over the same
contract; it changes no gate and adds no lane to CI.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 1 — the problem, stated as a measurement

**There is no tracked definition of "the local matrix" anywhere.** I
grepped `docs/`, the rules and the role files for `30/30`, `FIX1` and
`CIGEN`: nothing. It exists only in whatever each dispatch happens to
enumerate, and I write those by hand.

The cost is not hypothetical:

- **Four consecutive `0.85.0` stages reported "30/30"** while the
  `gate-matrix-fixtures` job also ran **fourteen**
  `python3.14 -m unittest scripts.tests.test_*` steps that no bash
  fixture script wraps. None of those four rounds ran them.
- **The round that found that enumerated thirteen**, missing
  `test_markdown_links` — the same transcription error, one level up.
- **One of the fourteen is order-dependent.**
  `test_check_m2a_rollback_coverage` shells out to a checker that reads
  `target/rfc094-rollback-coverage.txt`, a build artefact the Rust store
  test run produces, so on a cold tree it reports a failure that is not
  one. I lost time to that deciding whether it was real.

Every one of those is a defect in my dispatches, repeated. **The fix is
to stop writing the list by hand.**

## 2 — the design question, to measure rather than guess

**What is the single source the script derives from?** Two candidates,
and I am not choosing for you:

- **`contracts/gate-inputs.toml`'s `[gates]`** — 24 entries, the
  machine-readable expansion RFC 093 owns and `scripts/ci-gate.sh`
  already reads. It does **not** contain A3.4, the four `FIX` fixture
  scripts, CIGEN, or the fourteen `unittest` steps.
- **`.github/workflows/ci.yml`** — which is itself *generated* from
  `gate-inputs.toml` (its own header says so) and is what CI actually
  runs. It contains everything, including the four and the fourteen.

**My reading is that deriving from `ci.yml` is the only choice that
cannot drift from CI by construction**, with `[gates]` consulted for the
local form of those 24 commands — because `ci.yml` invokes them as
`bash scripts/ci-gate.sh <ID>`, and `ci-gate.sh` asserts
`HEAD == $GITHUB_SHA` and a clean tree, neither of which holds locally.
**Measure that before building to it**: read `ci-gate.sh`'s assertions
and confirm they are what makes it unusable locally. If you find a
cleaner decomposition, say so and build that.

**State in your return which source you derived from and why.**

## 3 — what to build

### 3a — `scripts/local-matrix.sh`

Runs the complete set, in an order that works on a cold tree, printing
one line per check and a final count. Requirements:

- **Derives the set.** No hand-written list of gate ids, fixture
  scripts or unittest modules anywhere in it. If a lane is added to
  `ci.yml`, the next run picks it up with no edit here.
- **Ordering**: the Rust lanes that produce
  `target/rfc094-rollback-coverage.txt` run **before** the Python
  self-tests that read it. State in a comment why, naming the artefact
  — the next reader must not reorder it back.
- **Runs everything and reports everything**, rather than stopping at
  the first failure. A matrix whose value is completeness must not hide
  the second failure behind the first.
- **Prints the count it ran**, so a return can quote a number that was
  produced rather than remembered.
- **Says plainly that it is not Level B.** Put it in the output, not
  only the docs: Level B is every `[gates]` lane green via
  `workflow_dispatch` on the exact commit being tagged, and no local run
  is that. Carry the `+stable` caveat in the same block.

**Do not** add a "fast subset" mode. The thing being fixed is an
incomplete matrix; an option to run less of it reintroduces the defect
with a flag on it.

### 3b — a self-test that the derivation matches CI

`scripts/tests/test_local_matrix.py`, in this project's existing
black-box style: assert that the set the script derives equals the set
`ci.yml` actually runs. **Negative fixtures**: a synthetic `ci.yml` with
a lane the script would miss must make the test fail.

Without this, the script can drift from CI silently and we are back to
where we started with an extra file.

**Register it** the way the other thirteen are registered, so it runs in
CI — and **measure where that registration lives** rather than assuming;
`ci.yml` is generated, so the registration may belong in
`contracts/gate-inputs.toml` or in `scripts/generate-ci-workflow.py`.
Say which in your return.

### 3c — write it down

`docs/src/contributing/release-process.md` and RFC 093's handoff README
should name the script as the way to run the local matrix. **Do not
restate its command in `release-process.md`** — A3.4 asserts that file
contains no restatement of a `[gates]` command, and the script is a
wrapper over exactly those; name the script, not what it runs.

## 4 — what not to do

- **Do not change any gate, any lane, or `contracts/gate-inputs.toml`'s
  `[gates]` table.** This round adds a runner, nothing else.
- **Do not edit `scripts/ci-gate.sh`.** CI depends on its assertions.
- **Do not add a fast-subset flag** (§3a).
- **No `CHANGELOG` entry** — this is developer tooling, not a change to
  the shipped artefact. *This is the one exception to cycle H's standing
  instruction that each round writes its own entry; it applies because
  nothing here reaches an operator.*
- **No release work.** No tag, no publish, no `workflow_dispatch`, no
  Level B claim, and do not title the gate summary "Level B".

## 5 — what to return

- Which source §2 settled on, and the `ci-gate.sh` assertions you
  measured.
- Where the self-test's CI registration lives (§3b).
- **The script's own output**, verbatim, from one complete run.
- The count it printed, and whether it matches what you expected.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.
- Per test, the mutation and the assertion failure it produced —
  including the negative fixture in §3b.
- Any public signature change, named in prose. *(None expected; this is
  a script.)*

**Run the matrix with your own new script** for this round's evidence,
and say so. If it disagrees with what you would have run by hand, that
disagreement is the most interesting thing in the return.

Return it as a review request in `.git-exclude/review-requests/`. Leave
the working tree uncommitted.
