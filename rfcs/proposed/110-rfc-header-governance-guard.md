# RFC 110 — An RFC header may not legislate

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFC 000 is the source of the rule being guarded and is not amended by this RFC.
**Implementation prerequisites.** None.
**Closure prerequisites.** An RFC header cannot state a rule about who may review, and cannot cite an archived RFC as authority; a genuine exception is visible rather than invisible; the gate is green on the tree it lands in without editing any RFC to make it so.
**Tracks.** Governance integrity.
**Touches.** `scripts/check-rfc-integrity.py`, `scripts/tests/test_rfc_integrity.py`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/110-rfc-header-governance-guard/README.md`](../handoffs/110-rfc-header-governance-guard/README.md)

## Summary

Twice, a governance rule has been written into RFC headers, attributed upward to an authority that does not contain it, and left unchecked. In July a vendor-independence rule went into four headers attributed to an owner ruling that cannot be evidenced, blocking seven RFCs for four weeks. On 2026-09-22 eleven headers (093–103) were found asserting a reviewer bar RFC 000 does not contain, two of them citing the disposed RFC 018. The clauses are removed; nothing prevents a third occurrence.

## Why this is an RFC

This work was carried under `roadmap/rfc-header-governance-guard/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/110-rfc-header-governance-guard/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
