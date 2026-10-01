# RFC 130 — A gate declares the inputs it depends on, and runs when they change

**Status.** Accepted
**Accepted on.** 2026-10-01
**Approved by.** `@nabbisen`, 2026-10-01: "Accepted."
**Security review.** Required
**Independent design review.** [Security review 2026-10-01](../handoffs/130-gates-declare-their-input-scope/security-review-2026-10-01.md) — **by the architect, who authored this RFC, and therefore not independent**; carried by `@nabbisen` under `ROADMAP.md` R1's residual. He approved this RFC; he did not state that he reviewed its design, and this field does not claim he did. **The field's name overstates the document**, which says so in its own first lines.
**Amended on.** 2026-10-01 — D5 superseded by [RFC 131](./131-two-gate-levels-and-one-rule.md) D2, on `@nabbisen` declining D5's question as one a design should answer. This RFC is now implementable.
**Amended on.** 2026-10-01 — on that review, which returned two required changes: the change detector must fail open (D7) and the scope's completeness must be checked rather than reviewed once (D8). The amendment is material: as accepted, the RFC specified the optimisation without the two properties that keep it from becoming a silent hole.
**Design prerequisites.** None. This RFC does not depend on RFC 093's prose being correct — see "Why RFC 093 is cited as practice, not as authority".
**Implementation prerequisites.** None, as of 2026-10-01: [RFC 131](./131-two-gate-levels-and-one-rule.md) D2 supersedes D5 and answers it. **RFC 131's D1 is sequenced after this RFC's D1**, so this one is implemented first.
**Closure prerequisites.** A change that cannot affect the Rust build or test lanes does not run them, and a change that can does — where "can" is declared in a contract, checked, and fails closed. **No gate's scope is narrower than the inputs it actually reads, and that is asserted by a gate rather than by a reading** (D8). **A changed set that cannot be determined runs the complete matrix** (D7), demonstrated for `workflow_dispatch` and for an unresolvable base ref. A release or milestone claim can still obtain a complete-matrix green on one nominated commit, by a mechanism that does not depend on which paths that commit touched.
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

### D5 — **Superseded 2026-10-01 by [RFC 131](./131-two-gate-levels-and-one-rule.md) D2.** The question was the wrong shape

D5 asked `@nabbisen` whether a release or milestone claim requires the complete
matrix on the commit it cites, and made this RFC unimplementable until he
answered. **He declined the question, correctly:** *"What do I have to decide ?
It's a kind of design for stable release cycles. First, define stages such as time
to cut release, implementation and test completed etc. Then design which job(s)
should be passed at the time."*

He was right. D5 handed the owner a policy question that a design is supposed to
answer, which is the opposite of this project's workflow (`ROADMAP.md` S1a: the
architect designs, the owner accepts). RFC 131 does the design and answers it:
**a claim about the shipped system requires every gate on the exact commit it
cites** — for an RFC closing, a release cut and a milestone closing alike.

The substance D5 identified was real and survives in RFC 131 D3: per-push scoping
means a commit may be green without the Rust lanes having run on it, and
`workflow_dispatch` is how the complete set is obtained on a nominated commit.
What was wrong was asking rather than designing.

**This RFC is implementable.** D7's fail-open requirement is what makes
`workflow_dispatch` reliable for that purpose, so D7 and RFC 131 D2 depend on each
other and must not be split across dispatches.

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

### D7 — The change detector fails open; "I could not tell" means run everything

Added on the security review of 2026-10-01.

D6's detector must specify what it does when the changed set cannot be computed,
because the cases where it cannot are ordinary: `workflow_dispatch` has no base
ref to diff against, a force-push leaves `github.event.before` unresolvable, a
first push on a branch gives the all-zeroes SHA, and a multi-parent range can
report a narrower set than the push introduced.

A detector that read any of those as "nothing relevant changed" would skip every
Rust lane on a change that may be entirely Rust — and would defeat D5's own
escape hatch, since `workflow_dispatch` is the mechanism D5 relies on.

**So: any condition under which the changed set is not known with certainty runs
the complete matrix.** Absent, zero or unresolvable base ref; any non-`push`
event; any error from the diff. Scoping is an optimisation applied when the inputs
are known. It is never the fallback.

### D8 — Scope completeness is asserted by a gate, not established by a review

Added on the security review of 2026-10-01.

The review measured D3's list against the tree and found it complete: no
`.cargo/config.toml`, no `rust-toolchain.toml`, no `clippy.toml`, no
`rustfmt.toml`, no `deny.toml`, no `build.rs`, no `.sqlx/`. Toolchains are pinned
in `contracts/gate-inputs.toml`, which D3 already covers.

Complete-by-absence is fragile: any of those files appearing later would sit
outside the declared scope and silently narrow a lane's trigger. So
`scripts/check-gate-inputs.sh` (A3.4) asserts that no file in that candidate set
exists outside the declared Rust scope, failing with the path and the gate it
would affect. Adding `.cargo/config.toml` must break CI loudly rather than quietly
shrink what G01–G09b run on.

This is the standing counterpart to D4: D4 makes an absent declaration loud at
generation time, D8 makes an outgrown declaration loud afterwards.

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
