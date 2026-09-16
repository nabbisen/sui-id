# Reconcile `docs/ui-ux-contracts.md` with the code

**Authorized by.** [`ROADMAP.md`](../../ROADMAP.md) §Non-RFC work packages. Owner
authorization, 2026-09-16.
**Implementer.** Mid-capability model for phase 1 and phase 3. The architect
does phase 2, and the owner signs the contract revision.
**Baseline.** RFC 098 dispatch 15 landed, which puts the staleness banner on the
contract.

## Why this is not a documentation edit

`docs/ui-ux-contracts.md` declares itself normative: "implementation requirements
with a defined update process". The process is to increment the revision line and
update the cross-references in the relevant RFCs. RFC 098 dispatch 14 found 15
places where code and contract disagree (F1–F15), plus the unimplemented i18n
state keys in `docs/src/contributing/state-contract.md`.

Each disagreement is one of two things:
- **a code defect** — the contract is right, and the code is fixed;
- **a contract amendment** — a later accepted decision (RFC 055, 074, 088 or
  others) changed the behaviour, and the contract never followed.

Which one is a design decision. Silently rewriting the contract to match the code
would launder every defect into a requirement.

## Phase 1 — evidence (implementation role; no edits)

For each of F1–F15, and for each state-contract key group, one row:
- **the contract sentence**, quoted;
- **what the code does**, with `file:line`;
- **the decision trail** — the RFC, commit or ruling that changed the behaviour
  (`git log -S` and `rfcs/done/`). "None found" is a valid answer and must be
  stated.
- **security relevance** — does the gap weaken a dangerous-operation, step-up,
  audit or information-disclosure guarantee?

F4 (force logout), F5 (`admin delete-user --hard`), F13 (dev-mode lockout wording)
and F14 (dev bind warning) need the security column filled with particular care.

## Phase 2 — ruling (architect, then owner)

The architect classifies each row as a code defect or a contract amendment, with a
recommendation. The owner approves the contract revision: the new revision line,
and which amendments it carries.

## Phase 3 — execution (implementation role)

The contract edits go in one commit: revision incremented, cross-references
updated, staleness banner removed. Each code defect is its own commit, with tests.
`state-contract.md` follows the same ruling.

## Evidence

- Phase 1 table.
- Phase 3: per-commit tests; G10a, G10b, G12, G15; fmt, both clippy scopes,
  `cargo test --workspace` count before and after.
