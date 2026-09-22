# RFC 106 — Backup restore fails closed

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** The backup move to `sui-id-store` (`b824246`) has landed.
**Closure prerequisites.** An archive with no readable schema version is refused; `verify` fails wherever `restore` would fail, with the same message; nothing is written before it has been checked; each defect has a test that fails when its fix is removed.
**Tracks.** Operational integrity. Found by the backup move's equivalence record (F1, F2, F3, F5, K23).
**Touches.** `crates/sui-id-store/src/backup/ops.rs`, `crates/sui-id-store/src/backup/tar.rs`, `crates/sui-id-store/src/backup/types.rs`, `docs/src/guides/deployment.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/106-backup-restore-hardening/README.md`](../handoffs/106-backup-restore-hardening/README.md)

## Summary

Five defects in `crates/sui-id-store/src/backup/ops.rs`: a manifest-less archive bypasses both version refusals, because `parse_backup` fabricates a permissive manifest for pre-0.13 archives; `verify` reports a version mismatch without failing, so it passes an archive `restore` would refuse; `restore` runs no integrity check on the database bytes it writes and no validity check on the key bytes; a `create_dir_all(parent).ok()` swallows errors; and the envelope-magic check is unreachable.

## Why this is an RFC

This work was carried under `roadmap/backup-restore-hardening/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/106-backup-restore-hardening/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
