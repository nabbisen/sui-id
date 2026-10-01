# RFC 130 — A gate declares the inputs it depends on, and runs when they change

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None. This RFC does not depend on RFC 093's prose being correct — see "Why RFC 093 is cited as practice, not as authority".
**Implementation prerequisites.** Owner acceptance of D5, which is a policy question this RFC deliberately does not answer.
**Closure prerequisites.** A change that cannot affect the Rust build or test lanes does not run them, and a change that can does — where "can" is declared in a contract, checked, and fails closed. No gate's scope is narrower than the inputs it actually reads. A release or milestone claim can still obtain a complete-matrix green on one nominated commit, by a mechanism that does not depend on which paths that commit touched.
**Tracks.** Development throughput. Raised by `@nabbisen`, 2026-10-01, on observing that two runs in one day each took over thirty minutes.
**Touches.** `contracts/gate-inputs.toml`, `contracts/workflow-template.toml`, `scripts/generate-ci-workflow.py`, `scripts/check-gate-inputs.sh`, `.github/workflows/ci.yml` (generated).
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

## Summary

CI takes 31 minutes on every push. **72% of recent pushes could not have been
affected by the jobs that account for all of it.** The fix is not to make those
jobs faster and not to split the workflow file: it is for each gate to declare
which inputs it depends on, so a change outside those inputs does not run it.

## The measurement

From run `36829988539` (23 jobs, all green):

| | |
|---|---|
| Wall clock | 31.1 min |
| Slowest single job | G04, **31.1 min** |
| `needs:` edges between jobs | **none** |
| Spread between first and last job start | **8 seconds** |
| Jobs finishing in ≤ 2 min | **19 of 23** |

Two things follow, and the second is the point of this RFC.

**The wall clock already equals one job.** Nothing queues, nothing waits on
anything. So no rearrangement of the existing jobs — into stages, into domains,
into separate workflow files — can reduce it. That option is measured and
rejected, not merely unchosen.

**Almost all of it is four jobs.** G04 31m, G02 30m, G05 30m, G06 23m. Everything
else is ≤ 2 min. And the cost is test *execution*, not compilation:
`cargo build --workspace --all-targets` (G01) completes in **2 minutes** on the
same runner. The full suite runs in **158 seconds** on a 32-core machine against
~30 minutes on a 4-core runner, so the suite is CPU-bound on hardware rather than
pathological. **This RFC therefore proposes no change to any test, and no change
to any cost parameter** — in particular not to Argon2's 64 MiB / t_cost 2, where
the cost is the defence and a suite exercising a weaker parameter than production
would no longer test what ships.

## How often the slow jobs are not needed

Reconstructed from the commit range between each pair of consecutive CI runs on
`main` — the range GitHub actually evaluates for a push, not per-commit:

| | |
|---|---|
| Pushes measured | 29 |
| Touched `crates/**` or `Cargo.*` | 8 |
| **Touched neither** | **21 (72%)** |

Of the last 80 commits, 60 (75%) touch no Rust at all; they are overwhelmingly
`rfcs/`, `contracts/`, `scripts/` and `ROADMAP.md`. At ~31 minutes a run, the 21
pushes represent roughly eleven hours of runner time and twenty-one half-hour
waits for documentation changes.

Neither of the two runs on 2026-10-01 that prompted this RFC is an example: both
pushed ranges did touch `crates/`. The case rests on the base rate, not on them.

## Why RFC 093 is cited as practice, not as authority

RFC 093 says, at `rfcs/done/093-build-toolchain-release-gates.md:313`, *"M1 closes
only on one commit for which the complete matrix is green"* — which reads as
permission for exactly this change. **This RFC does not rely on that sentence.**

`@nabbisen`, 2026-10-01: *"We had better be doubtful about what the ex-architect
wrote."* That caution is well founded and this RFC measured how far it reaches:

- RFC 093 was accepted 2026-07-17 and its prose has been found imprecise twice in
  one day — condition 9's "an identifiable independent reviewer" and condition
  10's "dated independent closure metadata", neither of which the code ever
  enforced (RFC 128 D1/D2).
- **RFC 000 first appeared at its path on 2026-07-17, the same date**, and shares
  **295 identical non-trivial lines** with the retired RFC 018 (622 and 654 lines
  respectively) — including RFC 018:216-217, the source of the `N/A` prohibition
  that RFC 128 found was never enforced.

So the document RFC 128 re-derived condition 9 *from* is of the same vintage and
substantially the same text as the document it re-derived it *away from*. That
does not disturb RFC 128's measured finding, which is about the code and stands
without any document. It does mean **a sentence in RFC 093 or RFC 000 is not a
sufficient basis for a decision**, and this RFC does not treat one as such.

What this RFC relies on instead: the existing, working precedent in this repo.
`.github/workflows/audit.yml:36-47` is already path-scoped, and already includes a
self-reference to its own workflow file. D5 then puts the one genuinely normative
question to the owner rather than quoting it from a July document.

## Decisions

### D1 — Scope is declared per gate, in the contract that already holds gate inputs

`contracts/gate-inputs.toml` is the machine-readable expansion of the Gate Matrix
and is already named for gate *inputs*; today it records each gate's command and
setup. It gains a `paths` key per gate, and `scripts/generate-ci-workflow.py`
emits it. The contract stays the source of truth; the workflow stays generated;
`scripts/check-gate-inputs.sh` (A3.4) and `generate-ci-workflow.py --check`
continue to be what makes it a checked invariant rather than a convention.

### D2 — Only G01–G09b are scoped. Every other gate always runs

The always-run set is nineteen jobs whose measured total is about two minutes, so
scoping them buys nothing measurable. It also carries a real risk: a governance
gate that silently does not run while a document still claims it does. G11, G13,
G15, G16, G17 and G18 are exactly the gates that guard that class of claim, and
they are the cheapest jobs in the matrix. **They are never scoped.**

A docs-only push then costs about 2 minutes instead of 31. A Rust push is
unchanged.

### D3 — The Rust scope includes what changes what the lane *does*, not only what it compiles

`crates/**`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain*`, `scripts/ci-gate.sh`,
`contracts/gate-inputs.toml`, `contracts/workflow-template.toml`, and
`.github/workflows/ci.yml`.

The last four are the ones a careless filter omits. A change to the dispatcher or
to the gate's own recorded command alters what the lane verifies, so the lane must
re-run even when no Rust source moved.

### D4 — A gate with no declared scope is a generation error, not an implied "always"

`generate-ci-workflow.py` exits non-zero. This is the same shape as G18's rule
that a blank reason in `contracts/contract-paths.toml` is exit 2: the absence of a
declaration must be loud, because the quiet default is the dangerous one. A gate
may declare `paths = ["**"]` to mean always, and that is a visible, diffable
statement rather than an omission.

### D5 — What a release requires is the owner's to state, and this RFC does not state it

Per-push scoping means a commit may be green without the Rust lanes having run on
it. `workflow_dispatch` is already on the workflow, so the complete matrix can
always be run on a nominated commit, and D1–D4 do not remove that.

**The question this RFC does not answer:** whether a release or milestone claim
requires the complete matrix on the commit it cites. RFC 093 says it does, but
per the section above that sentence is not treated here as authority. If the
answer is yes, the mechanism already exists and the release process must name it.
If the answer is something else, that is `@nabbisen`'s to state and this RFC
should be amended to record it. **This RFC is not implementable until D5 is
settled**, which is why it is an implementation prerequisite above.

### D6 — One workflow with a `changes` job, not separate workflow files per domain

The question that prompted this RFC was whether to split `ci.yml` by domain or by
stage. Rejected, for three measured or checkable reasons:

1. **Domain is the wrong axis.** G08 and G11 are different domains, both cheap,
   both always-run; G02 and G05 are the same domain and both expensive. The axis
   that predicts cost is input scope, which is what D1 declares.
2. **A workflow that does not trigger reports nothing, not success.** Under branch
   protection a required check from a skipped workflow blocks a pull request
   permanently. `main` carries no protection today, so this is latent rather than
   live — but it is a trap laid for a change the owner is considering. A job
   skipped by `if:` inside one workflow does not have it.
3. **Generated surface.** `ci.yml` is generated and freshness-checked
   (`ci.yml:564-566`). Several workflow files multiply what the generator emits
   and what must be checked fresh, for no measured gain.

So: a first job computes the changed paths and the scoped lanes condition on its
output. It adds one `needs:` edge and about ten seconds to the lanes that do run.

## What this is not

It is not a performance change. No test is modified, no cost parameter is lowered,
no cache is added — `target/` caching was measured as worth about 2 of the 31
minutes and is out of scope. The 31 minutes is not reduced for a change that needs
the lanes; it is not spent for a change that cannot.

It also does not address running the same suite four times across the
MSRV × features matrix. Those four lanes are concurrent and so cost no wall clock,
and whether the matrix is the right shape is RFC 093's subject, not this one's.

## Risk

**A wrong scope is a gate that silently does not run while a document still claims
it does.** That is the failure class RFC 110 and RFC 128 exist to catch, one layer
down, and this RFC creates the opportunity for it where none existed before.

Three things hold it closed: the scope lives in a contract rather than in YAML
(D1), an absent declaration fails generation rather than defaulting to anything
(D4), and the set permitted to be scoped is restricted to eleven build/test/lint
lanes whose inputs are mechanically identifiable, with every governance gate
excluded by rule (D2). The residual risk is a `paths` list that is too narrow for
a lane that is in scope — which D3 addresses by naming the non-obvious entries,
and which the closure prerequisite above states as a property to be demonstrated,
not assumed.
