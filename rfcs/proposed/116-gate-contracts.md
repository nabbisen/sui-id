# RFC 116 — Gate contracts: one source, one gate each

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None. The measurements this RFC rests on are in its
handoff and were taken on 2026-09-22 against `0c15eca`.
**Implementation prerequisites.** None for stages 1–2. Stage 3 waits until
stages 1 and 2 have landed, so that a change to how every lane is defined
happens on a tree whose contracts are already true.
**Closure prerequisites.** No fact stated in `ci/` exists in a second place
that is not generated from it; every surviving file in `ci/` is read by a gate
that fails when the file stops being true; the audit matrix's `class` column is
checked against the code; every lane runs through one dispatcher; and the
placement of what survives is settled.
**Tracks.** Gate integrity. Raised by `@nabbisen` on 2026-09-22: "`ci/` seems
also messy and dirty because partially duplicate of `.github/workflows/`."
**Touches.** `ci/`, `.github/workflows/ci.yml`, `scripts/check-audit-matrix.sh`,
`scripts/check-gate-inputs.sh`, `scripts/ci-gate.sh`, `scripts/check-ui-invariants.sh`,
`scripts/tests/`, `rfcs/handoffs/094-transactional-audit/command-inventory.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/116-gate-contracts/README.md`](../handoffs/116-gate-contracts/README.md)

## Summary

`ci/` holds six files. Read one by one against what they claim, they divide
three ways, and only two are sound:

- **`write-commands.toml`** — 60 KB, 99 entries, **read by no gate**. It is a
  second copy of `rfcs/handoffs/094-transactional-audit/command-inventory.md`,
  and the two have already drifted nine rows. Its header counts are stale, and
  two of its schema's columns have been empty in all 99 entries since the day
  it was generated.
- **`audit-coverage-matrix.md`** — gated, but on one column of five. G13 checks
  event *names* in both directions. Actor, target, attributes and **class** are
  unverified prose, and `class` is where a command is asserted to be Class-A,
  which is what RFC 094's guarantee rests on.
- **`gate-inputs.toml`** — its `[gates]` command table is a genuine single
  source and should be kept. Every other section of it is copied into
  `.github/workflows/ci.yml`: the runner label 20 times, the system packages
  10, the stable toolchain 8, MSRV 6, Python 6, mdbook 2, the rust components
  4, seven action SHA pins across 57 uses, and the lane list as 17 jobs.
  `scripts/check-gate-inputs.sh` exists for the sole purpose of proving the two
  copies still agree.
- **`rfc-policy.toml`** and **`doc-authority.toml`** — small, documented in
  place, exercised by tests, no duplication. These are the standard.
- **`ui-invariants.toml`** — sound, but its lane G12 is the only one still
  outside the dispatcher, the sole entry in `[gate_matrix_exceptions]`.

The rule the directory should be held to is already written inside it, in
`doc-authority.toml`'s own header: *"a machine-consumed contract... The gate
that reads it is the only thing that makes it true."*

## Why this is not a rearrangement

`@nabbisen` asked whether to plan a rearrangement first. This RFC deliberately
puts placement last, for a reason that is not stylistic: **the layout cannot be
decided until it is known which files survive.** `write-commands.toml`
duplicates `command-inventory.md`; one of the two should stop existing.
Choosing a home for a file that may be deleted, at a cost of 47 referencing
files, buys nothing and risks the churn this project has already paid for
twice.

Moving a broken, ungated registry into a better-named directory leaves a
broken, ungated registry.

## Decisions

**D1 — One source per registry.** The command inventory exists once. The
surviving copy is the one a gate can check; the other is deleted or generated
from it, never hand-maintained beside it. *Which copy survives is an open
question below.*

**D2 — Every file in `ci/` is read by a gate that fails when it stops being
true.** A file that no gate reads is not a contract; it is a document that
looks like one, which is worse, because a reader trusts it. Any file that
cannot be given a gate leaves `ci/` and becomes documentation, labelled as
such.

**D3 — The audit matrix's `class` column is checked against the code.** A row
claiming Class-A while its command is not on the Class-A seam is a false
security claim in a file the threat model and three RFCs cite. Actor, target
and attributes are checked too where the registry makes them derivable.

**D4 — `.github/workflows/ci.yml` is generated from the single table**, and a
gate checks that the committed workflow is what the table generates — the same
shape as `cargo fmt --check`. Duplication is removed rather than reconciled;
`scripts/check-gate-inputs.sh`'s eight conditions collapse to one. The
`[gates]` table stays the source, so a lane remains runnable locally exactly as
CI runs it.

**D5 — Every lane runs through the dispatcher.** G12's exception is closed, or
the exception is restated as a decision with a date rather than as a deferral.

**D6 — Placement is decided last**, on the set of files that survives D1–D5,
and is a single move with every reference updated in the same commit.

## Open questions

1. **Which copy of the command inventory survives** — the TOML in `ci/` or the
   markdown in RFC 094's handoff? The architect's recommendation is the TOML:
   it is machine-readable, it is the copy that is currently correct, and a gate
   can check it against the code, which is what D2 requires. The markdown then
   becomes prose about the inventory, carrying no rows. The counter-argument is
   that the handoff is where a reader looks first.
2. **`doc-authority.toml`'s `tolerance_minor = 2`** is marked in the file
   itself as "A starting value for `@nabbisen` to adjust: it is chosen, not
   derived." It has never been adjusted. Does it stand?
3. **`ui-invariants.toml`'s `inline-style-bound maximum = 20`** is a ratchet
   with no record of when it was last tightened. Should it ratchet on every
   improvement, and if so, by what rule?

## Risks

- **D4 changes how every lane is defined.** Sequenced last of the functional
  stages, so it lands on a tree whose contracts are already true. A generated
  workflow that is wrong fails visibly on the next push; a generated workflow
  that is subtly wrong — a missing lane — fails invisibly, so the generator's
  own test asserts the lane set round-trips.
- **D3 may find existing false rows.** That is the point, and it is the reason
  Security review is Required: the matrix is cited by `docs/threat-model.md`
  and by RFCs 094, 102 and 103. Any row it disproves is a correction to a
  shipped security claim, not a test failure to be silenced.
