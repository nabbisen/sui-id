# RFC 105 — Audit notes: escape attribute values

**Status.** Implemented (v0.79.0)
**Closure reviewed on.** 2026-10-02
**Closure approved by.** `@nabbisen` (accountable owner), 2026-10-02: "Both approved." — batch 3 and RFC 131's fork together. The closure review was performed by **the architect, which wrote this RFC**, and is therefore **not** independent of it; `@nabbisen` is the approver, which is what RFC 000 requires when no independent role exists. The implementation role verified the review's named tests separately and returned no findings; that corroboration is recorded beside the review and is not approval.
**Closure evidence.** [Closure review batch 3, 2026-10-02](../handoffs/105-audit-note-escaping/closure-review-batch-3-2026-10-02.md), with [independent verification](../handoffs/105-audit-note-escaping/closure-verification-batch-3-2026-10-02.md)
**Accepted on.** 2026-09-24
**Approved by.** `@nabbisen`, 2026-09-24, **with the design-review gap below in
view and carried by him.**
**Independent design review.** [None was performed, and none now can be — the record of the gap](../handoffs/105-audit-note-escaping/design-review-gap-2026-09-24.md).
This RFC was dispatched for implementation while still Proposed, through the
architect's error; the role that would have reviewed the design has now built
it and cannot review its own work. The field cites the record rather than a
review, because there is no review to cite. What is unreviewed is named there:
the choice of percent-encoding over quoting, and the judgment that sound
operator queries are worth a less legible reason.
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
