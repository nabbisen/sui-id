# Documentation carries the truth; a test enforces it

**RFC.** [RFC 127](../../proposed/127-documentation-carries-the-truth.md), **Proposed** — not accepted, nothing dispatched.
**Author.** High-capability model, requirements-architect role.

## Where the two facts are today

- `crates/sui-id/tests/e2e/r120_routes.rs` — the 35 routes that answer without an
  authenticated actor, each with a stated reason, derived from `router.rs` and
  asserted in both directions.
- `crates/sui-id/tests/e2e/r122_routes.rs` — the routes carrying the `no-store`
  layer, asserted in both directions against a hand-list.

Neither has any counterpart under `docs/`.

## What a reviewer or implementer will need to settle

1. **Which `docs/` layer.** RFC 098's taxonomy puts product documentation in
   `docs/src/` and the engineering specification at `docs/` top level. An
   operator assessing exposure and an architect designing against the system are
   different readers; say which this serves, and whether it is one page or two.
2. **Whether the route table belongs in public documentation at all.** It is
   derivable from `router.rs` by anyone, and `docs/threat-model.md` already
   states security posture publicly — but that is an argument, not a conclusion,
   and it should be made rather than assumed.
3. **D3's check.** G15 reads `doc-authority.toml`. Is "this page names this test,
   and this test names this page" expressible there, or does it need its own
   small gate? **If the honest answer is that no cheap check exists, say so** —
   RFC 122 has just established that claiming a mechanical guarantee one does not
   have is worse than an honest hand-maintained pairing.
4. **The generated-versus-written question.** The route list is derived from
   `router.rs` by a test. A document could be generated from the same source. Say
   whether it should be, given that `ci.yml` is generated and the project has one
   working pattern for that already.
