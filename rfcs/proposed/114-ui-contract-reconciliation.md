# RFC 114 — Reconcile `docs/ui-ux-contracts.md` with the code

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFC 098 dispatch 15 has landed, putting the staleness banner on the contract.
**Implementation prerequisites.** None.
**Closure prerequisites.** Every one of the 15 disagreements is resolved as either a code fix or a signed contract amendment; the contract's revision line is incremented; no disagreement is left undeclared.
**Tracks.** Documentation authority. The owner signs the contract revision.
**Touches.** `docs/ui-ux-contracts.md`, `docs/src/contributing/state-contract.md`, `crates/sui-id-web/src/`, `crates/sui-id-i18n/`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/114-ui-contract-reconciliation/README.md`](../handoffs/114-ui-contract-reconciliation/README.md)

## Summary

`docs/ui-ux-contracts.md` declares itself normative — "implementation requirements with a defined update process". RFC 098 dispatch 14 found 15 places where the code and the contract disagree, plus unimplemented i18n state keys in `docs/src/contributing/state-contract.md`. Each disagreement is either a code defect, where the contract is right, or a contract amendment, where a later accepted decision superseded it and the contract was never updated.

## Why this is an RFC

This work was carried under `roadmap/ui-contract-reconciliation/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/114-ui-contract-reconciliation/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
