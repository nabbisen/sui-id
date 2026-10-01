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
