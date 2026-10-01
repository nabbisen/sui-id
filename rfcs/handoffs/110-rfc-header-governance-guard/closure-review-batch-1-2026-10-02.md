# Closure review — batch 1: RFCs 110, 128, 129, 130, 132

**Date:** 2026-10-02
**Reviewed by.** The architect (high-capability model, requirements-architect role).
**Approval required from.** `@nabbisen`. **Not yet given** — this document is the
evidence he approves or rejects, and under RFC 000 the implementer cannot be the
sole approver, so nothing here closes until he does.
**Commit cited.** `fc056df` — **Level B green on it: CI run `36878881472`, 24 of 24
jobs success**, including all eleven Rust lanes and every governance gate. RFC 131
D2's requirement for a claim about the shipped system is therefore met by the
commit this review cites, not by a local run.

**Why one document for four RFCs.** Their closure prerequisites are all documentary
or gate properties verified the same way — run the gates, read the records — and
they share one cited commit. Four near-identical files would be worse evidence,
not better. Precedent: `094-095-096-correction-review-2026-08-26.md`.

**RFC 131 was in this batch and is withdrawn from it.** See the last section; it
cannot close as its prerequisite is written, and that is `@nabbisen`'s to resolve.

## RFC 110 — An RFC header may not legislate

| Clause | Finding |
|---|---|
| No header in `proposed/`, `accepted/` or `done/` carries a reviewer-rule field or states a rule about who may review | G11 condition 14 enforces the closed six-label allowlist and the forbidden phrases; **green on `fc056df`** |
| No header cites an archived RFC as authority except by a closed, diffable allowlist entry | G11 condition 15; `contracts/rfc-policy.toml:43` carries `[archive_citations]` with its single entry `"025" = ["007"]` |
| The guard catches the clauses of **both** past incidents, demonstrated against their commits | Recorded in [`README.md`](./README.md) and [`design-review-2026-09-24.md`](./design-review-2026-09-24.md), both tracked |
| The gate is green on the tree it lands in **without editing any RFC to make it so** | G11 green across every run since; the allowlist mechanism exists precisely so RFC 025's legitimate `Supersedes` line did not have to be rewritten |

**Met.** One note for the record rather than an objection: condition 14 was exercised
against the architect's own draft on 2026-10-01, when a first attempt at RFC 093's
amendment cited archived RFC 018 in a header and the gate refused it. A guard that
catches its own author is the demonstration this prerequisite asks for.

## RFC 128 — G11's conditions derive from RFC 000, not from a retired RFC

| Clause | Finding |
|---|---|
| Every G11 condition traces to a live document — RFC 000, or the RFC that legitimately owns it | Established by D2's fourteen-row trace. **The architect independently re-verified 9 of the 14 rows against the cited files, including every row the conclusion rests on**; all RFC 000, RFC 093 and RFC 110 citations resolve and say what was claimed |
| No condition enforces a formulation whose only source is an archived RFC | Verified three ways: `git log -S"N/A" -- scripts/check-rfc-integrity.py` returns only the commit that documents the *absence*; condition 9's logic is byte-identical to its introduction at `3f2eace`; all twelve `author` occurrences in the file are prose |
| Where RFC 000 leaves something undefined, the gate does not supply a definition inherited from elsewhere, **and says so** | `scripts/check-rfc-integrity.py`'s condition-9 docstring states explicitly that it checks neither the literal `N/A` nor author-vs-reviewer identity, and why; three acceptance tests pin the three patterns live in the tree |

**Met.** **One durability point `@nabbisen` should know before approving:** D2's
full fourteen-row trace lives in `.git-exclude/review-requests/`, which is **not
tracked by git**. The conclusion and the method are recorded in the tracked
handoff, and this review restates the verified result — so the closure reference is
durable. But the row-by-row working is not in the repository, and if that matters
to him, committing the trace is small work that should precede approval rather
than follow it.

## RFC 129 — Review is scoped to who exists

| Clause | Finding |
|---|---|
| No milestone exit gate requires a reviewer who does not exist | The seven re-scoped exit gates stand as amended; re-checked on `fc056df` |
| No RFC closure prerequisite requires a reviewer who does not exist | Grepped every `accepted/` RFC and `ROADMAP.md`. The surviving matches are **historical, not requirements**: RFC 094's *"required fresh independent design review … That review is complete and recorded"* (past tense, and satisfied) and its record of what acceptance *did* register for each exclusion. Checked rather than assumed, because a stale requirement reads identically to a historical one at a glance |
| Every such statement either names a role this project has, or names the one scheduled external engagement and says why that point and no other | The M6/M7 engagement is named with its reasoning; nothing else defers to an unavailable reviewer |

**Met.** This is also the first RFC in the project whose own
`Independent design review` field is accurate in the ordinary sense of the word —
`@nabbisen` reviewed it and did not author it.

## RFC 132 — The files a stranger reads first

| Clause | Finding |
|---|---|
| Someone who has found an authentication bypass is told where to report it **at the moment they choose to open an issue** | `.github/ISSUE_TEMPLATE/config.yml` carries `contact_links` with the private advisory form as its first entry, and `blank_issues_enabled: false` |
| No template invites a secret or a third party's personal data without saying not to | `bug_report.yml` names cookies, bearer and refresh tokens, client secrets, signing- and master-key material and real users' identifiers — and says *"Paste a redacted log, not no log"*, so it does not suppress the evidence it is protecting |
| No file in `.github/` promises something the project does not do | The stale `--version` parenthetical is gone (the flag prints `sui-id 0.79.0`), and the credit promise now names `CHANGELOG.md` with an advisory only as something that may follow — consistent with `ROADMAP.md` S1d |
| **No contact route anywhere is an email address** | Scanned the whole implementing diff: the only address-shaped string in 1,676 inserted lines is `t@example.invalid`, a reserved-TLD fixture, and `check-published-versions.py`'s `User-Agent` is the repository URL |
| `CONTRIBUTING.md` describes **this** project — a behaviour change needs an RFC, a release includes publishing six crates | Both present, and the restated gate commands are replaced by a pointer to `scripts/ci-gate.sh <GATE_ID>` |

**Met**, and the last two clauses are the ones that had been false for months rather
than merely absent.

## RFC 130 — A gate declares the inputs it depends on

Added to this batch on 2026-10-02, when the measurement its prerequisite needed
finally arrived. It had been the one candidate blocked on an **observation** rather
than on work.

| Clause | Finding |
|---|---|
| A change that **can** affect the Rust build or test lanes **does** run them | CI run `36878881472` on `fc056df`: `rust scope changed: true`, all 11 scoped lanes ran, 24/24 green. That push touched `Cargo.lock` |
| A change that **cannot** affect them **does not** run them | CI run `36934836767` on `2262861`: `rust scope changed: false`, **exactly G01–G09b skipped** (11), all 13 governance lanes ran, and the run was green. That push touched `ROADMAP.md`, the G16 baseline and one handoff — no declared scope path |
| "can" is declared in a contract, checked, and **fails closed** | `paths` on all 21 lanes in `contracts/gate-inputs.toml`, zero without one; generation refuses a lane with no scope (D4); the detector fails open on every unknown-input condition (D7), verified against five of them |
| No gate's scope is narrower than the inputs it actually reads, **asserted by a gate** | A3.4 condition 9 (D8): verified by planting `.cargo/config.toml`, which failed the gate naming the path, and passed once removed |
| A changed set that cannot be determined runs the complete matrix, demonstrated for `workflow_dispatch` and an unresolvable base ref | Both demonstrated directly against the real detector, along with `schedule`, an empty base ref and an all-zero base ref |

**Met, and this is the clause that could only ever be closed by observation** — no
local run can show CI skipping a lane.

**The measured payoff, for the record:** **2.2 minutes against 31.1**, a 93%
reduction on a documentation-only push, against a predicted "about two minutes".
The prediction was made from the job timings before the mechanism existed; it held.

Worth noting what the 2.2 minutes is *not*: it is not a faster test suite. Nothing
was optimised, no cost parameter was lowered, and a push that touches Rust still
costs the full thirty-one minutes. The saving is entirely in not running eleven
lanes against a change that could not affect them — which is the whole of what
`@nabbisen` asked for when he said the question was *"which jobs to run"*, not how
to make them faster.

## RFC 131 — withdrawn from this batch: its prerequisite cannot be met as written

Its closure prerequisite contains: *"A milestone cannot be closed over an RFC that
has not been."*

That is **D5**, and D5 was **recorded unenforceable** on 2026-10-01 after measuring
`ROADMAP.md`'s "Planned RFCs" column — M0 is closed while its own cell names five
RFCs still open, M5's cell mixes a `done/` RFC with a `proposed/` one, and two rows
name no RFC at all. A checker correct against that would be a per-row lookup table
disguised as a parser, which the dispatch explicitly forbade approximating.

So RFC 131 states a closure condition that its own implementation established
cannot be built. **Three ways out, and the choice is `@nabbisen`'s, not the
architect's:**

1. **Amend the prerequisite** to record that the clause is unenforceable and why,
   dropping it as a closure condition. Cheapest and honest; it is the same shape as
   RFC 129's re-scoping of requirements that named a nonexistent reviewer.
2. **Restructure the milestone table** — a separate, uniform "RFCs required done
   before close" column distinct from "Planned RFCs" — then implement D5 against
   it. Makes the property real, and is a further RFC's worth of work.
3. **Leave RFC 131 open** until a milestone actually comes up for closure.

The architect's recommendation is (1), with the unenforceability already recorded
in `ROADMAP.md` doing the work the clause was meant to do — but **this is recorded
as a recommendation, not a decision**, and RFC 131 stays in `accepted/` until he
rules.

It is worth saying plainly that this is the RFC's own rule working on its author:
RFC 131 D2 says a claim requires its gate to be green, and RFC 131 cannot make its
own claim because one of its gates does not exist.

## What approval would mean

On `@nabbisen`'s approval, and not before, the architect adds to each of RFCs 110,
128, 129, 130 and 132: `Closure reviewed on. 2026-10-02`, `Closure approved by.` his
name and words, and `Closure evidence.` a reference to this document; moves them to
`rfcs/done/`; updates `rfcs/README.md`; and re-runs the gates, since G11's
folder-versus-`Status` conditions change behaviour when a file moves.

On approval the same applies to RFC 130.

**Five of the eighteen candidates, and the first RFC closures this project has ever
performed.**
