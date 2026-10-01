# RFC 131 — handoff

**RFC.** [`../../accepted/131-two-gate-levels-and-one-rule.md`](../../accepted/131-two-gate-levels-and-one-rule.md)
**Status of the RFC.** **Accepted** 2026-10-01 by `@nabbisen` ("Reviewed. Accepted."), on his own design review.
**Depends on.** [RFC 130](../../accepted/130-gates-declare-their-input-scope.md), whose D5 this RFC supersedes.

## Not dispatched yet

**Do not start.** RFC 131 has no unmet implementation prerequisite — unlike
RFC 130, it answers its own policy question — but the architect has not yet
dispatched it, and the sequencing matters: **RFC 130's D1 must land first**,
because D3 here ("scoping applies to Level A only") refers to the `paths`
declarations RFC 130 introduces. Implementing 131 against a tree with no declared
scopes would have nothing to point at.

The architect will dispatch both together, with RFC 130 first.

## What will be dispatched

- **D1** — two gate levels. Level A is RFC 130's scoped set; Level B is every gate
  in `contracts/gate-inputs.toml`'s `[gates]`, on one named commit. **Level B is
  not a new list and must not become one** — the moment it is written down
  separately it can drift from `[gates]`, which is the defect this RFC removes.
- **D2** — the rule: a claim about the shipped system requires Level B on the
  exact commit it cites. Three claims invoke it: an RFC moving to `done/`, a
  release cut, a milestone closing. Each keeps the evidence it already has; add
  none.
- **D4** — `docs/src/contributing/release-process.md`'s pre-publish checklist is
  replaced by Level B plus the four packaging checks (`cargo package` verify,
  version bump with refreshed `Cargo.lock`, `CHANGELOG.md` entry, clean tree), and
  `scripts/check-gate-inputs.sh` asserts the document contains **no `cargo`
  invocation that is not a `[gates]` command**.
- **D5** — a milestone does not close over an RFC that has not closed. **Read the
  RFC's Risk section before starting this one:** ROADMAP's milestone rows do not
  name their constituent RFCs today, so the linkage may not be derivable. If it
  is not, say so and record D5 as unenforceable. **Do not approximate it** — a
  guessed milestone-to-RFC mapping is worse than an absent one, because it would
  pass a gate while meaning nothing.

## The one thing most worth attacking

D4's assertion is the load-bearing part, and it is the one that found a live
defect: the current checklist requires three cargo commands against the Gate
Matrix's 23, all three weaker, and its `cargo fmt` is exactly the command that
passed while G08 failed on `71bca90`. When implementing, check the assertion
actually catches that historical text — if it would not have failed on the
pre-change document, it does not work.
