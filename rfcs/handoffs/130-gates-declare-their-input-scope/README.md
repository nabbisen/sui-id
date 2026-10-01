# RFC 130 — handoff

**RFC.** [`../../accepted/130-gates-declare-their-input-scope.md`](../../accepted/130-gates-declare-their-input-scope.md)
**Status of the RFC.** **Accepted** 2026-10-01 by `@nabbisen` ("Accepted."), amended the same day on its security review to add D7 and D8.

## Not dispatched yet — but no longer blocked

**Status corrected 2026-10-01.** This file previously said *do not start*, because
D5 was unanswered. **D5 is superseded** by [RFC 131](../../accepted/131-two-gate-levels-and-one-rule.md)
D2, which answers it by design rather than by asking the owner. RFC 130 has **no
unmet implementation prerequisite.**

It is still not dispatched: the architect dispatches RFC 130 and RFC 131 together,
**RFC 130's D1 first**, because RFC 131 D3 refers to the `paths` declarations D1
introduces. Wait for that dispatch.

**D7 and RFC 131 D2 must not be split across dispatches.** D7's fail-open rule is
what makes `workflow_dispatch` reliable, and RFC 131 D2 relies on
`workflow_dispatch` to obtain the complete set on a nominated commit. Implementing
one without the other leaves a release able to cite a commit whose Rust lanes
never ran.

## What will be dispatched

- **D1** — a `paths` key per gate in `contracts/gate-inputs.toml`, emitted by
  `scripts/generate-ci-workflow.py`.
- **D2** — only G01–G09b carry a scope. Every other gate, and every governance
  gate without exception, always runs.
- **D3** — the Rust scope: `crates/**`, `Cargo.toml`, `Cargo.lock`,
  `rust-toolchain*`, `scripts/ci-gate.sh`, `contracts/gate-inputs.toml`,
  `contracts/workflow-template.toml`, `.github/workflows/ci.yml`.
- **D4** — a gate with no declared scope is a generation error, not an implied
  "always".
- **D6** — one workflow with a `changes` job, not per-domain workflow files.
- **D7** — the detector **fails open**: any condition under which the changed set
  is not known with certainty runs the complete matrix.
- **D8** — A3.4 asserts no build-affecting file exists outside the declared Rust
  scope.

## Two things to carry from today, before this one starts

**State the baseline commit in anything carrying line numbers or hashes.** Twice
on 2026-10-01 a derived number was wrong where the artifact was right: a false
"undeclared hunk" finding by the architect, and the architect's own shift
arithmetic in RFC 128 D7. The dev team caught the second by **measuring instead of
applying the reviewer's number**, which was correct and is the standard here.

**D7 and D8 exist because a review of the architect's own RFC found them.** When
this is implemented, the thing most worth attacking is D3's path list and the
detector's failure modes — not the generator plumbing, which is mechanical. If a
scope can be made too narrow, say so and stop; a silently skipped gate is worse
than a slow one, and the thirty-one minutes this RFC exists to save is not worth
one unenforced security property.
## Dispatched 2026-10-01 — stage 3

**Do stages 1 and 2 first** —
[`../132-the-files-a-stranger-reads-first/README.md`](../132-the-files-a-stranger-reads-first/README.md).
Stage 4 follows this one, in
[`../131-two-gate-levels-and-one-rule/README.md`](../131-two-gate-levels-and-one-rule/README.md),
and depends on D1 here existing.

All of D1–D4 and D6–D8, as one stage. They are not separable: D4 fails generation
when a scope is missing, so it only makes sense once D1 gives gates a scope to
declare; and D7 and D8 are the two properties that keep the whole thing from being
a silent hole.

### What to build

**D1** — a `paths` key per gate in `contracts/gate-inputs.toml`, emitted by
`scripts/generate-ci-workflow.py`. The contract stays the source of truth; the
workflow stays generated and freshness-checked.

**D2** — **only G01–G09b carry a scope.** Every other gate always runs. Do not
scope G11, G13, G15, G16, G17 or G18 on cost grounds even though some read
`crates/` — they guard documentary claims and the whole cheap set measures about
two minutes.

**D3** — the Rust scope: `crates/**`, `Cargo.toml`, `Cargo.lock`,
`rust-toolchain*`, `scripts/ci-gate.sh`, `contracts/gate-inputs.toml`,
`contracts/workflow-template.toml`, `.github/workflows/ci.yml`. The last four are
the ones a careless filter omits: they change what a lane *verifies*, not what it
compiles.

**D4** — a gate with no declared scope is a **generation error**, exit non-zero.
`paths = ["**"]` is how a gate says "always", visibly. An omission must never
default to anything.

**D6** — one workflow with a cheap `changes` job and `if:` conditions. **Not
separate per-domain workflow files**: a workflow that does not trigger reports
*nothing*, not success, which would block a pull request permanently under branch
protection.

**D7** — the detector **fails open**. Any condition where the changed set is not
known with certainty runs the complete matrix: absent, zero or unresolvable base
ref; any non-`push` event; any diff error. `workflow_dispatch` must run everything
— RFC 131 D2 depends on that being reliable.

**D8** — `scripts/check-gate-inputs.sh` asserts no build-affecting file exists
outside the declared Rust scope. The candidate set, verified absent from the tree
on 2026-10-01: `.cargo/config.toml`, `rust-toolchain.toml`, `clippy.toml`,
`rustfmt.toml`, `deny.toml`, any `build.rs`, `.sqlx/`. Adding one of those later
must break CI loudly rather than quietly narrow a lane's trigger.

### What is most worth attacking

**Can a scope be made too narrow?** That is the whole risk, and the generator
plumbing is mechanical by comparison. Specifically:

- Try to construct a change that affects a Rust lane but matches no path in D3.
  If you find one, **stop and report it** — do not widen the list and move on
  quietly, because the same reasoning gap will produce the next omission.
- Try to make the detector report an empty changed set. Force-push, first push on
  a branch, a merge commit, a multi-parent range, `workflow_dispatch`. Each must
  run the full matrix, not nothing.

**A silently skipped gate is worse than a slow one.** The thirty-one minutes this
RFC saves is not worth one unenforced security property.

### Expected effect, so you can tell whether it worked

A documentation-only push drops from ~31 minutes to about 2. A push touching
`crates/**` is unchanged. Measured baseline: 72% of the last 29 pushes touched
neither `crates/` nor `Cargo.*`.

### Protocol

As in stage 1's dispatch: hand over a working tree, state the **parent** commit as
baseline, declare every hunk's hash, and run gates through `scripts/ci-gate.sh`.

One more, specific to this stage: **when you check a CI result, assert the
workflow and the job set, not the conclusion.** A commit can have several runs —
`756e9f9` had `Security audit` (one job) and `CI` (23 jobs), both reporting
`success`. A release was nearly tagged on the wrong one. Green is not evidence of
complete.
