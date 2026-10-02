# RFC 131 — Two gate levels, and one rule about claims

**Status.** Accepted
**Accepted on.** 2026-10-01
**Approved by.** `@nabbisen`, 2026-10-01: "Reviewed. Accepted."
**Security review.** Required
**Independent design review.** [Owner review 2026-10-01](../handoffs/131-two-gate-levels-and-one-rule/owner-review-2026-10-01.md) — by `@nabbisen`, who did **not** author this RFC. He stated that he reviewed it, and three of his interventions changed the design: he rejected RFC 130 D5's question as one a design should answer, identified RFC completion as a missing stage, and then capped the stage count — which collapsed five gate lists into two levels. **This field's name fits, in the ordinary sense**, unlike RFC 124's, 128's and 130's.
**Design prerequisites.** [RFC 130](../done/130-gates-declare-their-input-scope.md), accepted 2026-10-01. This RFC supersedes its D5.
**Implementation prerequisites.** None. Unlike RFC 130, this RFC answers its own policy question rather than deferring one.
**Closure prerequisites.** There is exactly one answer to "what must pass before this claim", it is the same answer for all three claims that make it, and no document states a weaker one. **A tagged version is either on the registry or recorded as abandoned, and the discrepancy is detected without anyone remembering to look** (D7). `docs/src/contributing/release-process.md` cannot drift from the Gate Matrix without a gate failing. *(The clause "a milestone cannot be closed over an RFC that has not been" was **removed as a closure condition on 2026-10-02**, on `@nabbisen`'s approval of the architect's recommendation. D5 established that it cannot be built against `ROADMAP.md`'s milestone table as structured, and the RFC may not require as a condition of its own closure something its implementation proved unbuildable. The property is not abandoned — D5 and the ROADMAP paragraph it points to record why it is unenforceable and what restructuring would make it real.)*
**Tracks.** Release stability. Raised by `@nabbisen`, 2026-10-01, as a stage model for release cycles.
**Touches.** `docs/src/contributing/release-process.md`, `scripts/check-gate-inputs.sh`, `ROADMAP.md`, `rfcs/done/130-gates-declare-their-input-scope.md` (D5 superseded).
**Amended on.** 2026-10-01 — **D4 widened on `@nabbisen`'s approval** to every document stating a verification command, after measuring that three do and all three are weaker than the gates. D7's mechanism also corrected from a weekly cron to a required release step, on finding `.github/workflows/fuzz.yml:3-5` already records that a cron in this repo failed eight consecutive weeks unnoticed.
**Amended on.** 2026-10-01 — D7 added while cutting 0.79.0, on measuring that **four tagged versions were never published** (0.76.10, 0.76.11, 0.76.12, 0.78.0). D2 governed the cut and nothing governed the publish, so a release could satisfy this RFC in full and still never reach a user. The amendment is material.
**Amended on.** 2026-10-02 — the D5 clause removed from **Closure prerequisites**, on `@nabbisen`'s approval: *"Both approved. Your recommendation is accepted."* The RFC asked, as a condition of its own closure, for a property D5 had measured to be unbuildable. Same shape as RFC 129's re-scoping of requirements that named a reviewer who does not exist: the requirement is corrected, not quietly dropped, and the reason stays in D5.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

## Summary

RFC 130 decided *when* a gate runs on a push. It deferred *what a release
requires* to its D5, which was the wrong shape: it asked the owner a policy
question that a design should answer.

This RFC answers it, and answers it with **two gate levels and one rule** rather
than a stage matrix — because the stages all turned out to require the same gates.

## Why this is two levels and not five stages

The first draft of this design had five stages: continuous, implementation
handover, RFC completion, release cut, milestone closure. `@nabbisen`, 2026-10-01:
*"Be careful for the workflows not to be too complicated. I mean too many stages
can bring confusion around management for release stability."*

Acting on that caution produced a better design, not a smaller one. Writing the
five stages out, **three of them require exactly the same gate set** — the complete
matrix on a named commit. They differ only in the *evidence* they attach, and
every one of those evidence requirements **already exists** and is already
enforced:

| Stage | Gates required | Evidence, already specified |
|---|---|---|
| RFC completion | complete matrix | RFC 000's closure metadata; G11 condition 10 |
| Release cut | complete matrix | the packaging checks in `release-process.md` |
| Milestone closure | complete matrix | ROADMAP's milestone exit gates |

So a five-stage model would have created **five gate lists to maintain where two
levels suffice**, and each list a fresh opportunity for the drift this RFC exists
to remove. The stages are real; they are just not the right unit for *gates*.

## The measurement that makes this necessary

Two documents answer "what must pass before a release" today, and the weaker one
is the one a maintainer would read.

`docs/src/contributing/release-process.md:85-101`, "Pre-publish checklist",
requires three cargo commands. The Gate Matrix has 23 gates. All three overlapping
commands are weaker than their gate:

| Checklist | Gate |
|---|---|
| `cargo fmt --all -- --check` | G08: `cargo +stable fmt --all -- --check` |
| `cargo clippy --workspace --all-targets -- -D warnings` | G07: `cargo +stable clippy … --all-features --locked`; G07b the default-features twin |
| `cargo test --workspace` | G02, G04, G05, G06 — four MSRV × features lanes, all `--locked` |

It omits fifteen gates entirely: LDAP smoke, mdBook, and every governance gate
G11–G18. **And nothing governs the file** — no gate and no contract references it.

The first row is not hypothetical. On 2026-10-01 a G08 failure reached `71bca90`
because the architect verified with `cargo fmt` while the gate runs
`cargo +stable fmt`. **The checklist's command is the one that passed while the
gate failed**, so the documented release procedure would have shipped it.

A second gap, measured while looking for the stages: **no milestone exit gate
requires its constituent RFCs to be closed.** A milestone could be declared closed
over RFCs still sitting in `accepted/`.

## Decisions

### D1 — There are two gate levels, and no more

**Level A — scoped.** Every gate whose declared scope the change touches, plus
every always-on gate. This is RFC 130, unchanged.

**Level B — complete.** Every gate in `contracts/gate-inputs.toml`'s `[gates]`,
green on one exact named commit.

Nothing else. No per-stage gate list, and **no new contract file**: Level B is not
a list to be maintained, it is "all of `[gates]`", which is already enumerated and
already checked by A3.4. A list that cannot be written down separately cannot
drift from the thing it mirrors.

### D2 — The rule: a claim about the shipped system requires Level B on the exact commit it cites

One sentence, three existing claims:

- an RFC moving `accepted/` → `done/`;
- a release cut;
- a milestone closing.

Each already carries its own evidence requirement and keeps it. This RFC adds no
evidence and removes none; it supplies the gate requirement they were all
missing, and supplies the same one to each.

**Level A ⊆ Level B, by construction** — Level A is a subset of `[gates]`. So
monotonicity needs no separate invariant to check: RFC 130's scoping can never
reduce what a claim requires, because scoping only ever selects *within* Level A.
That is the property that makes RFC 130 safe to have, and it is free rather than
enforced.

### D3 — Scoping applies to Level A only

RFC 130's `paths` declarations decide what runs continuously and on an
implementation handover. They have no effect on Level B. `workflow_dispatch` is
how Level B is obtained on a nominated commit, which RFC 130 D7 guarantees runs
everything.

### D4 — The release document stops enumerating commands, and a gate holds it there

`docs/src/contributing/release-process.md`'s pre-publish checklist is replaced by:
Level B on the commit being tagged, obtained by dispatch, **plus** the four checks
the Gate Matrix does not cover and which exist only in that document today —
`cargo package` verify build, version bump with refreshed `Cargo.lock`, a
`CHANGELOG.md` entry, and a clean tree.

`scripts/check-gate-inputs.sh` (A3.4) then asserts that the document contains **no
`cargo` invocation that is not a `[gates]` command**. A re-enumeration of a weaker
command list fails the gate rather than sitting there for three months. This is the
same shape as A3.4's existing byte-match of the Gate Matrix table and G18's
`contracts/README.md` check: the document may describe, but it may not restate the
contract in its own words.

### D5 — A milestone does not close over an RFC that has not closed

A milestone's exit gate requires every RFC it claims, in `rfcs/` under that
milestone's scope, to be in `done/` with its closure metadata present. This closes
the measured gap and makes RFC-completion-before-milestone-closure a checked
ordering rather than a matter of care.

### D6 — RFC 130's D5 is superseded

RFC 130's D5 asked whether a release requires the complete matrix, and made itself
unimplementable until answered. D2 answers it: yes, and so do RFC completion and
milestone closure. RFC 130 is amended to record D5 as superseded by this RFC, which
**unblocks RFC 130's implementation**.

### D7 — A cut that is never published is not a release

Added 2026-10-01, while cutting 0.79.0.

D2 says a release cut requires Level B on the exact commit it cites. **It says
nothing about the cut reaching anyone**, and that turned out to be the gap that
mattered. Measured against the registry:

| | |
|---|---|
| Published, newest first | 0.77.0, 0.76.9, 0.76.8, … |
| Tagged | … 0.76.10, 0.76.11, 0.76.12, 0.77.0, 0.78.0 |

**Four tagged versions were never published.** The first three are described in
`docs/src/contributing/release-process.md:72` — as a caution about *how to verify
the registry*, not as an open defect — and they are still unpublished three months
later. Then **0.78.0 repeated it**, and that was recorded nowhere until this
amendment. So a release could pass every gate, carry security fixes, be tagged,
and leave every user exposed; which is what happened to the RFC 120 and RFC 125
fixes for a week.

Publishing is the only stage a user experiences. So:

1. **A release cut is incomplete until every crate in the publication order is on
   the registry at that version**, or until the cut is recorded as abandoned in
   `CHANGELOG.md` with a reason. 0.78.0 is recorded as abandoned by this RFC's
   landing commit; 0.76.10–12 remain and are `@nabbisen`'s to abandon or publish.
2. **The discrepancy is detected without anyone remembering to look.** A check
   compares each crate's newest tag against the registry's `max_version` and
   reports any tag with no published counterpart. It **must** send an explicit
   `User-Agent`: crates.io returns a policy error without one, which reads as
   "not published" for every crate and is not — the trap the release document
   already records.
3. **It runs at release time, not on a schedule.** **Corrected 2026-10-01**, the
   same day it was written: the first draft said "weekly, in the shape `audit.yml`
   already uses". `.github/workflows/fuzz.yml:3-5` records why that is wrong here
   — *"Manual only: a weekly schedule failed eight consecutive weeks unnoticed
   (situation-fit audit, 2026-08-26) — a cron in a solo repo has no subscriber for
   a red run. Run before a release instead."* This project has already paid for
   that lesson, and D7 proposed the pattern it abandoned.

   The irony is the point: **the publish gap exists because nobody was looking**,
   so a mechanism that depends on someone looking cannot be its fix. The check
   therefore binds to a moment attention is already there — a **required step in
   the release process**, run at the cut and again after publishing, failing the
   release rather than a cron nobody reads. A schedule may be added as a redundant
   second signal, but it is never the primary.

   It is still **not a per-push gate and not in `[gates]`**: it depends on a
   third-party network call, so making it blocking would fail CI for reasons
   unrelated to the change under test, and `[gates]` must stay offline and
   deterministic.

**Why this does not contradict the "don't over-complicate" caution.** It adds no
stage and no level. The claim in D2 is unchanged; D7 only says that a claim nobody
can act on is not finished, and puts the detection somewhere that cannot silently
lapse.

### D4 (widened) — every document that states a verification command, not just one

D4 names `docs/src/contributing/release-process.md`. The 2026-10-01
governance-files audit found **three** documents stating verification commands
weaker than the gates, not one:

| Document | States | The gate |
|---|---|---|
| `release-process.md` | `cargo fmt --all -- --check` | G08: `cargo +stable fmt --all -- --check` |
| `.github/CONTRIBUTING.md` | `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test --workspace` | G08, G07/G07b, four test lanes |
| `docs/src/contributing/local-dev.md` | `cargo clippy --workspace -- -D warnings`, `cargo fmt --check` | G07 adds `--all-targets --all-features --locked` |

None carries `+stable`; none carries `--locked`. A contributor following
`CONTRIBUTING.md` passes locally and fails CI — which is exactly how a G08
failure reached `71bca90`.

**One nuance the assertion must respect:** `local-dev.md` also shows deliberately
narrow commands for focused iteration (`cargo test -p sui-id-core --lib
password`). Those are useful and must stay. So the rule cannot be "no `cargo`
command outside `[gates]`"; it must distinguish **a command presented as the
verification bar** from **a command shown for focused local work**. The simplest
honest form: a document may not state a command that *looks like* a gate's
command but differs from it — same subcommand and `--workspace` scope, different
flags.

**Approved by `@nabbisen`, 2026-10-01.** D4's assertion therefore covers all three
documents, and any future one: a document may not state a command that looks like
a gate's but differs from it. `release-process.md` and `.github/CONTRIBUTING.md`
stop restating the commands and point at `scripts/ci-gate.sh <GATE_ID>` instead;
`local-dev.md` keeps its narrow, focused commands, which are not claims about the
verification bar.

## What this is not

It is not a workflow redesign and adds no job, no stage machinery and no new
contract file. The total new vocabulary is two words — Level A, Level B — and one
sentence. If a future reader must hold more than that in mind to know what a
release requires, this RFC has failed at the thing it was written to do.

## Risk

**The rule is only as strong as the dispatch being actually run.** D2 says a claim
requires Level B on the commit it cites; nothing in CI can force a human to
dispatch it before tagging. D4 narrows this by making the release document say so
and by preventing it from saying anything weaker, but the residual is real and is
stated rather than engineered away: **a release claim is an assertion by whoever
makes it, and this RFC makes the assertion checkable, not automatic.**

The second residual is D5's scoping: "the RFCs a milestone claims" must be
derivable, and ROADMAP's milestone rows do not name them explicitly today.
Implementation must either make that linkage explicit in ROADMAP or state that it
cannot be derived — and if it cannot, D5 is unenforceable and should be recorded
as such rather than approximated.
