# M2a is not ready to close, and the reason is a governance divergence

**Date:** 2026-10-03
**Status: the exactly-once work in §3 is dispatched.** The list divergence in §1
is **not** dispatched and stays open; it is for the project owner.
**Found while** preparing an M2a closure review after all five of RFC 094's
closure clauses were met — by checking the ROADMAP's exit criteria against the
RFC's clause list instead of writing the review against the RFC alone.

## 1. The two lists are different, in both directions

**`rfcs/accepted/094-transactional-audit-registry.md`, `Closure prerequisites`,
M2a — five clauses:**

1. every converted Class-A path uses the approved transaction seam;
2. injected append failures roll back mutation for every converted row;
3. C15 is atomic;
4. the structural gate passes over converted commands;
5. raw database access confined to `sui-id-store`, `rusqlite` depended on by no
   other crate, gate-asserted.

**`ROADMAP.md` line 71, M2a exit criteria — four:**

1. every converted Class-A row has injected-failure rollback **and exactly-once
   evidence**;
2. C15 atomic;
3. structural gate passes over converted commands;
4. the coverage matrix states conversion status per command and claims no
   unconverted command is atomic.

**Neither is a subset of the other.**

| Only in the RFC | Only in the ROADMAP |
|---|---|
| the seam clause | **exactly-once evidence** |
| `rusqlite` confinement | the coverage-matrix claim |

**This is the finding, not the exactly-once gap.** A milestone whose exit gate is
stated twice, differently, cannot be closed cleanly by anyone — and the failure
mode is silent: close against either list and it looks complete. I came within
one step of writing a closure review against the RFC's five clauses and reporting
M2a ready.

**Open, and not mine to settle:** which list governs, or whether the two are
reconciled into one. My recommendation is **one list in the RFC, with ROADMAP
pointing at it**, because a milestone row in a planning document is the wrong
place for a normative exit gate — the same reasoning that put RFC 134's
supersession in the RFC rather than in RFC 096's matrix.

## 2. Measured state against the union of both lists

| Criterion | State |
|---|---|
| seam, every converted path | **met**, gate-asserted by G20 |
| injected-failure rollback, every converted row | **met, 24 of 24** |
| C15 atomic | **met** |
| structural gate over converted commands | **met**, G20 |
| `rusqlite` confined, gate-asserted | **met**, e2e test passes |
| coverage matrix states conversion status, no unconverted command atomic | **met** — `check-audit-matrix-columns.py` fails a row claiming `A` that is not sealed-atomic and a row claiming `B` that is; "all conditions satisfied (58 table rows, 25 sealed Class-A events)" |
| **exactly-once evidence, every converted row** | **not met — 1 of 24** |

**How exactly-once was measured.** Every audit-count assertion shape in
`crates/sui-id-store/src/commands/tests/runner*`: 24 are
`assert_eq!(latest_audit_action, before_audit)` — the *rollback* assertion, that
no row was appended after an injected failure. Exactly **one** is
`assert_eq!(audit_rows, 1, "exactly one event")`.

**Rollback and exactly-once are different properties.** Rollback proves nothing
is written when the append fails. Exactly-once proves that on the **success**
path precisely one audit record exists — not zero, and not two. Twenty-four
tests prove the first. One proves the second.

## 3. Dispatched — exactly-once evidence for the remaining 23

For each converted command, on the **success** path: assert the audit tail grows
by **exactly one** row, and that the row is the expected event. Place it with the
command's existing tests.

- **Not a new test file per command** where a happy-path test already exists —
  extend it. Several already snapshot the audit tail for their rollback
  assertions; the counterpart assertion is cheap there.
- **The removal check still applies.** A test that passes when the command's
  audit append is deleted is not exactly-once evidence. Report the observed
  failure per command, as you did for the eleven.
- **If a command legitimately writes more than one audit row on success, say so
  and stop** rather than asserting whatever it currently does. That is a design
  question for me, not a number to record.
- **Extend G20** so the registration is mechanical rather than a count I re-grep:
  a second registry alongside the rollback one, with the same shrink-only
  exemption discipline, so "every converted row has exactly-once evidence"
  becomes gate-asserted rather than asserted by me reading `grep` output.

**Do not attempt §1.** The list divergence is not yours to resolve, and nothing
in §3 depends on how it is settled — exactly-once is required by one list and
harmless under the other.

## Return

As usual: per-hunk SHA-256 against a stated baseline, the removal evidence per
command, and the full local gate set — **including A3.2**, which the last package
omitted from its gate table and which turned `main` red.
