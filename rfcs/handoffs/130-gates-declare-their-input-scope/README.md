# RFC 130 — handoff

**RFC.** [`../../accepted/130-gates-declare-their-input-scope.md`](../../accepted/130-gates-declare-their-input-scope.md)
**Status of the RFC.** **Accepted** 2026-10-01 by `@nabbisen` ("Accepted."), amended the same day on its security review to add D7 and D8.

## Nothing is dispatched yet, and this is why

**Do not start.** RFC 130 is Accepted, so it is no longer build-prohibited by RFC
000 — but its own **Implementation prerequisites** name a condition that is not
met: **D5 is unanswered.**

D5 asks whether a release or milestone claim requires the complete matrix on the
commit it cites. That is `@nabbisen`'s to state. It is not a detail that can be
settled during implementation, because the answer decides whether
`workflow_dispatch` must be wired into the release process as a required step or
merely remains available. Building the scoping first and discovering the answer
later is how a release ends up citing a commit whose Rust lanes never ran.

The architect has **not** recorded an answer, and nothing in this file should be
read as one.

## What will be dispatched, once D5 is settled

Stated here so the shape is visible, not as an instruction to begin.

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
