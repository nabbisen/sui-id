# RFC 116 — independent design review request

**RFC.** [RFC 116 — Gate contracts: one source, one gate each](../../proposed/116-gate-contracts.md). Proposed.
**Reviewer.** Mid-capability model, implementation role. It authored neither the
RFC nor its handoff.
**Route.** Owner decision of 2026-08-26 (`ROADMAP.md` §S1): a design is reviewed
by the role that must build against it.
**Why now.** `@nabbisen` accepted this RFC on 2026-09-22. RFC 000 requires a
named independent design reviewer for a security-sensitive RFC, and G11 refuses
an Accepted RFC whose `Independent design review` field has no durable
reference. This review is what lets the acceptance be recorded.
**Baseline.** `e9514f9` or later.
**Scope.** Read-only. Change no code, no RFC text, no `ci/` file. Report
findings.

## 1. Are the measurements right?

The handoff opens with a measurement table and the command behind each number.
**Re-run every one of them.** One row per measurement: the claim, the command
you ran, the number you got, and whether it agrees. A disagreement is a
blocker, not a note — the whole RFC is built on these.

In particular, confirm or refute:

1. **Nothing reads `ci/write-commands.toml`.** Search wider than I did: scripts,
   workflows, build scripts, tests, `crates/**/build.rs`, any `include_str!`.
   A single reader anywhere changes D2's conclusion for that file.
2. **The nine-row drift** between `ci/write-commands.toml` and
   `rfcs/handoffs/094-transactional-audit/command-inventory.md`. Give the two
   id lists and their difference.
3. **G13 checks names only.** Read `scripts/check-audit-matrix.sh` and state
   exactly which columns of the matrix are verified and which are not.
4. **The `ci.yml` repetition counts** — runner 20, packages 10, stable 8, MSRV
   6, Python 6, mdbook 2, components 4.

## 2. Is D3 buildable, and how far?

D3 says the matrix's `class` column is checked against the code.

5. **Can a gate decide, from the source, whether a command is Class-A?** Name
   the exact construct it would key on — `declare_write_command!`,
   `Database::class_a`, the descriptor set — and say whether it is decidable by
   a text scan or needs the `syn` AST work RFC 094 M2b plans. If it needs the
   AST, say so plainly: that changes D3's cost and its sequence.
6. **Actor, target, attributes.** For each, say whether the registry makes the
   matrix's claim derivable today. Where it does not, say what would make it
   so, or recommend leaving that column unchecked. **An approximate check is
   worse than none** — say which of the three you would not attempt.
7. **Run the check you would build, by hand, over today's matrix.** How many of
   the 56 rows disagree with the code? If any row's `class` is wrong, that is a
   false security claim in a file the threat model and three RFCs cite: name it
   in the review, and do not correct it.

## 3. Is D4 buildable without losing what matters?

8. **Generating `ci.yml` from `ci/gate-inputs.toml`.** What in today's workflow
   is *not* derivable from the table? List every such thing — a step, a
   condition, a permission, a concurrency group. Those are what decide whether
   generation is honest or whether the generator needs its own inputs.
9. **`scripts/check-gate-inputs.sh` has eight conditions.** For each, say
   whether generation makes it unnecessary, whether it moves into the
   generator, or whether it must survive as a separate check. The RFC claims
   conditions 1, 6 and 8 are replaced outright — confirm or refute each.
10. **The local-run property.** `scripts/ci-gate.sh` must still run a lane
    exactly as CI runs it. Does generation threaten that in any way?
11. **`audit.yml` and `fuzz.yml`.** Do they read the same table? Give the
    evidence either way.

## 4. Open question 1 — which inventory copy survives

12. **Give your view, with reasons.** The RFC recommends the TOML: machine
    readable, currently correct, checkable by a gate. The counter is that the
    handoff is where a reader looks first. You are the role that will build the
    gate — say which copy you can actually check, and what the markdown parse
    would cost if it were the survivor.
13. **If the TOML survives, can `event` and `descriptor` be filled from the
    registry now?** RFC 094 Stage 1 has landed. Either they can be derived, or
    the columns should go. Say which, with the code that would supply them.

## 5. Sequence

14. **Is stage 3 correctly placed after stages 1 and 2?** The reasoning is that
    a change to how every lane is defined should land on a tree whose contracts
    are already true. Say if you would order it differently and why.
15. **Anything in this RFC that cannot be built as described**, or that would
    be better built another way. Include what the RFC does not mention and
    should.

## What to return

A review-request package under `.git-exclude/review-requests/`, containing:
- the measurement table from §1, one row per number, with your commands;
- findings ranked blocker / high / medium / low;
- your answers to items 5–15, each with the file and line you read;
- the by-hand D3 result from item 7, including any row it disproves.

Do not implement anything. This RFC is Proposed and nothing in it is
authorized to be built until it is Accepted, which this review enables.
