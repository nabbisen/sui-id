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

- **D7** — a scheduled check comparing each crate's newest tag against the
  registry's `max_version`, reporting any tag never published. **It must send an
  explicit `User-Agent`** — without one crates.io returns a policy error that
  reads as "not published" for every crate, which is the trap
  `docs/src/contributing/release-process.md:72` already records. **Weekly, in
  `audit.yml`'s shape — not a per-push gate and not in `[gates]`**, because it
  depends on a third-party network call and `[gates]` must stay offline and
  deterministic.

## A near-miss while cutting 0.79.0 — D2 must name the workflow, not just the commit

Found 2026-10-01 by the architect, cutting the release this RFC governs.

D2 says a claim requires Level B "on the exact commit it cites". **A commit can
have several workflow runs, and a green from the wrong one is indistinguishable
at a glance.** `756e9f9` had two: `Security audit` (one job, triggered because
`Cargo.toml` changed) and `CI` (23 jobs). A lookup matching on commit SHA alone
returned the audit run first, reporting `completed / success`. Nothing about that
result says it ran one gate rather than twenty-three.

The release was nearly tagged on it. What caught it was checking the job count,
not the conclusion.

**So, when implementing D2:** the check asserts the **workflow** and the
**job set**, not merely a green conclusion on the right SHA. "Every gate in
`[gates]`" is satisfiable only by the CI run, and "green" is not evidence of
"complete" — a workflow that runs a subset reports success exactly as one that
runs everything. Match the gate IDs present against `[gates]` and fail on a
missing one.

This is the same error shape as the release checklist D4 removes: a weaker check
that looks like the stronger one from outside.

## The one thing most worth attacking

D4's assertion is the load-bearing part, and it is the one that found a live
defect: the current checklist requires three cargo commands against the Gate
Matrix's 23, all three weaker, and its `cargo fmt` is exactly the command that
passed while G08 failed on `71bca90`. When implementing, check the assertion
actually catches that historical text — if it would not have failed on the
pre-change document, it does not work.
