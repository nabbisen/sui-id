# Documentation carries the truth; a test enforces it

**RFC.** [RFC 127](../../done/127-documentation-carries-the-truth.md), **Accepted 2026-09-30**. Implemented — see the note at the end of this file about the commit that carries it.
**Author.** High-capability model, requirements-architect role.

## Where the two facts are today

- `crates/sui-id/tests/e2e/r120_routes.rs` — the 35 routes that answer without an
  authenticated actor, each with a stated reason, derived from `router.rs` and
  asserted in both directions.
- `crates/sui-id/tests/e2e/r122_routes.rs` — the routes carrying the `no-store`
  layer, asserted in both directions against a hand-list.

Neither has any counterpart under `docs/`.

## Dispatched for implementation 2026-09-30

**The four questions this handoff previously left open are settled in the RFC, as
D5–D8.** They were design questions — which reader the page serves, whether it is
public, how the pairing is held true, and written versus generated — and settling
them is the architect's, not the implementer's. They are not reopened here.

## What to build

**One page: `docs/src/reference/security-surfaces.md`**, reachable from
`SUMMARY.md`, holding two tables:

1. **Routes that answer without an authenticated caller** — method, path, and the
   reason each is intended to. The reasons already exist in
   `crates/sui-id/tests/e2e/r120_routes.rs`'s hand-list; they were written to be
   read and have never been read by anyone but a reviewer.
2. **Surfaces that show a secret once** — the surface, how the secret reaches the
   reader, and what keeps the response out of a cache. Sourced from
   `r122_routes.rs` and RFC 122's own enumeration.

Above them, a short statement of what the page is for: an operator assessing
exposure, and what the page does **not** cover — it states what the system
promises, not how the code keeps the promise.

**Then extend both enumeration tests (D7)** so each asserts the documented table
matches the set it derives from `router.rs`. The document becomes the checked
artefact. A route added without a documentation row then fails the same test that
already catches a route added without a reason.

## Decide and state

**How the table is parsed out of the page.** A markdown table is easy to read and
fiddly to parse; a reader's needs come first, so do not deform the page to make
the test simpler. Say what you chose and why. If the parse turns out to be
brittle, say that too — RFC 122 established that an honest limitation beats a
claimed mechanism.

## Evidence

- Both tests failing before the page exists, passing after.
- A mutation per test: add a route to `router.rs` without documenting it; remove a
  row from the page. Name the test that catches each.
- `mdbook build docs`, G10a, G10b, G15, and the full suite count before and after.


## The commit that carries this implementation is mislabelled

`59ff2b1` is titled *"docs(rfc-112): dispatch the repair for this RFC's
intermittently red e2e test"*. **It also contains the whole of RFC 127's
implementation** — `docs/src/reference/security-surfaces.md`, the `SUMMARY.md`
entry, and the two enumeration tests' new assertions.

The architect ran `git add -A` while the dev team's delivered-but-unreviewed work
was in the tree, and committed and pushed both together. The commit message
describes only the dispatch.

**The work itself is sound and was verified afterwards** against the package's
claims: 6 of 6 hunks match, `security-surfaces.md` hashes to
`8f90a472…` as claimed, nothing unclaimed, 1049 tests, every gate green. **Nothing
wrong entered the repository** — what entered it is correctly labelled nowhere.

**It is not being rewritten.** `59ff2b1` is pushed to a public repository, and
amending it is a history rewrite, which no standing authorization covers. This
note is the repair: the commit's real contents are recorded where someone looking
for RFC 127's implementation will look for it.
