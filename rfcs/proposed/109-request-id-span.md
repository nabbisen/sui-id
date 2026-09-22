# RFC 109 — Request logs keep their request ID across `.await`

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** An event emitted after an await carries `request_id`, shown by a deterministic test that fails against today's middleware; R11's explicit field stays.
**Tracks.** Diagnosability. Found by R11 Part 1 (`e39e18b`).
**Touches.** `crates/sui-id/src/http/request_id.rs`, `crates/sui-id/src/http/handlers/admin/auth.rs`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/109-request-id-span/README.md`](../handoffs/109-request-id-span/README.md)

## Summary

`crates/sui-id/src/http/request_id.rs` enters the request span with a synchronous guard and holds it across `next.run(req).await`. A synchronous guard does not follow a future across suspension, so events emitted after the handler resumes can fall outside the span and lose `request_id`. The R11 test that checked for the span failed in one run of three.

## Why this is an RFC

This work was carried under `roadmap/request-id-span/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/109-request-id-span/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
