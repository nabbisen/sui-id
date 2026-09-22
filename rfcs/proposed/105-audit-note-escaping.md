# RFC 105 — Audit notes: escape attribute values

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** RFC 094's audit-note builder is the single place a note is assembled.
**Closure prerequisites.** A value cannot introduce a key/value boundary; the encoding is reversible and documented beside the builder; every reader of a note — the operator guide's queries, `docs/src/reference/audit-events.md`, the audit page — agrees with it; historical rows are not rewritten.
**Tracks.** Audit integrity. Found by RFC 103 stage 3's review, finding 3.
**Touches.** `crates/sui-id-store/src/registry.rs`, `crates/sui-id-core/src/account/recovery_link.rs`, `docs/src/guides/operators.md`, `docs/src/reference/audit-events.md`, `crates/sui-id-web/src/pages/audit.rs`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/105-audit-note-escaping/README.md`](../handoffs/105-audit-note-escaping/README.md)

## Summary

An event's `note` is built as `key=value` pairs joined by spaces, and values are not escaped. Any event whose attributes carry operator- or user-supplied text can be made to look as though it carries fields it does not. Nothing is misread today only because every command writes free text before the real fields, so the last occurrence of a key is the true one — an unwritten rule that a future command appending free text last would invert.

## Why this is an RFC

This work was carried under `roadmap/audit-note-escaping/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/105-audit-note-escaping/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
