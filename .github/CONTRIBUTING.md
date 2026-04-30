# Contributing

Thanks for considering a contribution. sui-id is intentionally small; the
review philosophy reflects that.

## Before you start

For anything beyond a typo, please open an issue first and describe what you
want to change and why. This isn't a barrier — most things will get an
encouraging "yes, please send a PR" — but it saves both sides from writing
code that won't merge.

## Code style

- `cargo fmt` before pushing.
- `cargo clippy --workspace --all-targets` should be clean.
- `cargo test --workspace` should pass.
- Workspace-wide lints in `Cargo.toml` are real: `unsafe_code` is forbidden,
  `unwrap_used` and `expect_used` are warnings. The few existing `expect`
  calls have a comment explaining why they cannot fail in practice.
- Public items get rustdoc. Private items only get a comment when their
  intent isn't obvious from the name.
- Tests live next to the code they test (`mod tests` or
  `path/to/file/tests.rs`). Integration tests live in the relevant crate's
  `tests/` directory.

## Property-based tests

A handful of modules use [proptest](https://crates.io/crates/proptest)
to express invariants directly — crypto round-trips, the CIDR matcher,
PKCE S256 derivation, password hashing, and the redirect-URI exact-match
rule. These run as part of the regular `cargo test` suite under tight
case caps (`cases: 4` for Argon2-driven tests, `cases: 256–512` for
cheap ones) so the suite finishes in a reasonable time.

To run the properties under wider coverage — recommended before a
release, and as a periodic CI job — override the case count from the
environment:

```bash
PROPTEST_CASES=4096 cargo test --workspace
```

When a property fails, proptest writes a regression file under
`crates/<crate>/proptest-regressions/`. **Commit it.** The regressed
input becomes part of the test suite forever, so the same shrunk
counter-example is replayed on every future run.

When adding a new property, give the test a sentence-long doc comment
that says what invariant the property captures. Property tests that
just paraphrase the production code as `assert_eq!` aren't useful;
the ones that earn their place are the ones that pin down a rule that
*could* be broken by a future refactor.

## Commit hygiene

One concern per commit. Squash WIP commits before opening a PR. Imperative
mood in the subject line ("add X" not "added X"). Reference the issue
number in the body if relevant.

## What we look for in a PR

- A description of the problem the change solves.
- A test that exercises the new behaviour. For bug fixes, a test that fails
  on `main` and passes on the branch.
- A note in `CHANGELOG.md` under the `[Unreleased]` section if the change
  is user-visible.
- An update to `docs/` if the change affects operators or integrators.

## What we won't merge

- Code without tests for behaviour we can practically test.
- Changes that break the single-binary, single-file deployment model
  without a clear migration story.
- Adding a new dependency without justification, especially if it pulls in
  a transitive `unsafe` graph.
- Cosmetic-only refactors that don't fix a real problem.

## Releases

Releases go through `CHANGELOG.md` and a Git tag. There is no release cadence;
we tag when the unreleased section has accumulated enough that an operator
would want it.

## Code of conduct

Be kind. Argue with the idea, not the person. The project ships software for
people who depend on it; that responsibility extends to how we treat each
other on the way.
