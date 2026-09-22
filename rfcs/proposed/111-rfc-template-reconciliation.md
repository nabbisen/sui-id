# RFC 111 — Reconcile `rfcs/README.md`'s template with what G11 enforces

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** The template supplies every label G11 requires; an RFC written from the template passes G11 unedited; a test binds the template to the gate's required-field list.
**Tracks.** Governance integrity. Defect exposed by RFC 093 M1b's own gate.
**Touches.** `rfcs/README.md`, `scripts/check-rfc-integrity.py`, `scripts/tests/test_rfc_integrity.py`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/111-rfc-template-reconciliation/README.md`](../handoffs/111-rfc-template-reconciliation/README.md)

## Summary

`rfcs/README.md` §Template is the normative RFC template — RFC 093's integrity contract says the parser recognises the bold, period-terminated labels from it. G11 requires seven fields on any standard RFC numbered 093 or above and the template supplies six; measuring it shows ten labels missing in total, one of them gate-enforced. A normative template that fails the gate enforcing it is a defect in a gate RFC 093 owns, exposed by that gate.

## Why this is an RFC

This work was carried under `roadmap/rfc-template-reconciliation/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/111-rfc-template-reconciliation/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
