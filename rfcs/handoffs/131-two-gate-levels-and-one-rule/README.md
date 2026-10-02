# RFC 131 — handoff

**RFC.** [`../../done/131-two-gate-levels-and-one-rule.md`](../../done/131-two-gate-levels-and-one-rule.md)
**Status of the RFC.** **Accepted** 2026-10-01 by `@nabbisen` ("Reviewed. Accepted."), on his own design review.
**Depends on.** [RFC 130](../../done/130-gates-declare-their-input-scope.md), whose D5 this RFC supersedes.

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

- **D7** — a check comparing each crate's newest tag against the registry's
  `max_version`, reporting any tag never published. **It must send an explicit
  `User-Agent`** — without one crates.io returns a policy error that reads as
  "not published" for every crate, which is the trap
  `docs/src/contributing/release-process.md:72` already records. **A required
  release step — not a per-push gate, not in `[gates]`, and not a schedule.**
  Corrected 2026-10-01: this bullet first read "weekly, in `audit.yml`'s shape",
  which the RFC's own D7 amendment then overturned on finding
  `.github/workflows/fuzz.yml:3-5` — a cron here *"failed eight consecutive weeks
  unnoticed"*. The bullet was not updated with the RFC, so for part of a day the
  handoff contradicted both the RFC and its own stage-4 dispatch below. **The dev
  team found it, applied RFC 000's rule that the RFC outranks its handoff, and
  reported the contradiction instead of silently choosing** — which is what to do
  with a contradiction in instructions.

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
## Dispatched 2026-10-01 — stage 4, the last

**Stage 3 must land first** —
[`../130-gates-declare-their-input-scope/README.md`](../130-gates-declare-their-input-scope/README.md)
— because D3 here refers to the `paths` declarations RFC 130 D1 introduces.

This stage carries **RFC 132's D4 with it**, which is not a liberty: D4 here adds
an assertion that `.github/CONTRIBUTING.md` and
`docs/src/contributing/release-process.md` currently **both violate**. Landing the
checker before the documents are fixed turns CI red on arrival. Landing the
documents without the checker leaves them free to drift again. They go together.

### What to build

**D1** — two levels, no more. Level A is RFC 130's scoped set. **Level B is every
gate in `[gates]`, on one named commit — and must not become a written-down
list.** The moment it is a list, it can drift from `[gates]`, which is the defect
this RFC exists to remove. Derive it.

**D2** — the rule: a claim about the shipped system requires Level B on the exact
commit it cites. Three claims invoke it — an RFC moving to `done/`, a release cut,
a milestone closing. **Each keeps the evidence it already has; add none.**

**D3** — scoping applies to Level A only. `workflow_dispatch` is how Level B is
obtained on a nominated commit, which RFC 130 D7 makes reliable.

**D4 (widened, approved 2026-10-01)** — together with **RFC 132 D4**:

1. `.github/CONTRIBUTING.md` and `release-process.md` stop restating gate commands
   and point at `scripts/ci-gate.sh <GATE_ID>` and `contracts/gate-inputs.toml`.
   `CONTRIBUTING.md` additionally gains the RFC-lifecycle paragraph and a Releases
   section that includes publishing six crates bottom-up (RFC 132 D4).
2. `scripts/check-gate-inputs.sh` asserts that **no document states a command that
   looks like a gate's but differs from it** — same subcommand and `--workspace`
   scope, different flags.
3. **`docs/src/contributing/local-dev.md` keeps its narrow commands**
   (`cargo test -p sui-id-core --lib password` and the like). Those are focused
   local iteration, not claims about the verification bar, and the assertion must
   not reject them. This distinction is the hard part of D4; get it right before
   writing the checker.

**D5** — a milestone does not close over an RFC that has not closed. **Read the
RFC's Risk section first.** ROADMAP's milestone rows do not name their constituent
RFCs today, so the linkage may not be derivable. If it is not: **say so and record
D5 as unenforceable. Do not approximate it.** A guessed milestone-to-RFC mapping
passes a gate while meaning nothing, which is worse than an absent one.

**D7** — the publish check: compare each crate's newest tag against the registry's
`max_version` and report any tag with no published counterpart. **A required
release step, not a cron** — `fuzz.yml:3-5` records a weekly schedule failing
eight consecutive weeks unnoticed here, and the publish gap exists *because nobody
was looking*, so a mechanism needing someone to look cannot fix it. **It must send
an explicit `User-Agent`**: without one crates.io returns a policy error that reads
as "not published" for every crate. Not in `[gates]` — it needs a network call and
`[gates]` stays offline and deterministic.

### The two things most worth attacking

**D4's assertion, against history.** Check it would have failed on the
*pre-change* `CONTRIBUTING.md` and `release-process.md`. If it would not have
caught `cargo fmt` where the gate is `cargo +stable fmt --all -- --check`, it does
not work — that exact difference let a G08 failure reach `71bca90`.

**D5's derivability.** Spend the time establishing whether the milestone-to-RFC
linkage exists before building anything on it. Reporting "it cannot be derived" is
a complete and correct outcome for D5.

### Protocol

As in the earlier stages: working tree only, parent commit as the stated baseline,
every hunk hashed and declared, gates through `scripts/ci-gate.sh`, and measure
any number handed to you rather than applying it.
