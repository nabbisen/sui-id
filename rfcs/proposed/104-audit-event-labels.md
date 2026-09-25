# RFC 104 — Audit event labels shown, and bound to the registered events

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None. The label text already exists in `crates/sui-id-i18n`.
**Implementation prerequisites.** RFC 094's `contracts/audit-coverage-matrix.md` is the registered-event source of truth, kept true against the code by G13.
**Closure prerequisites.** Every registered event has a label in every locale in `Locale::ALL`; no label names an unregistered event; a test binds the label set to the matrix so neither can drift again.
**Tracks.** Audit legibility.
**Touches.** `crates/sui-id-i18n/src/strings.rs`, `crates/sui-id-i18n/src/locale/`, `crates/sui-id-web/src/pages/audit.rs`, `crates/sui-id-web/src/pages/dashboard.rs`, `contracts/audit-coverage-matrix.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/104-audit-event-labels/README.md`](../handoffs/104-audit-event-labels/README.md)

## Summary

Translated audit-event labels shipped with RFC 002 on 2026-06-05 and no code has ever read them. The audit page and the dashboard render the raw action string — `auth.login.failure` — in every language. Unread, the labels drifted: measured against `contracts/audit-coverage-matrix.md`'s 55 registered events, 30 registered events have no label and 4 labels name events that do not exist.

## Why this is an RFC

This work was carried under `roadmap/audit-event-labels/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/104-audit-event-labels/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
