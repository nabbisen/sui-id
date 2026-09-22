# RFC 112 — Refuse to run against a database this build does not understand

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** Runs in parallel with the backup move, which takes `migrations::MAX_SCHEMA_VERSION` as it is.
**Closure prerequisites.** A binary refuses to open a database newer than it understands, with a message naming both versions; a failed version read is a refusal, not a re-run; both have tests.
**Tracks.** Data integrity. Found by RFC 098 dispatch 14.
**Touches.** `crates/sui-id-store/src/migrations.rs`, `crates/sui-id-store/src/errors.rs`, `docs/src/guides/deployment.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/112-schema-version-fail-closed/README.md`](../handoffs/112-schema-version-fail-closed/README.md)

## Summary

`migrations::run` skips every migration at or below the stored `schema_version` and returns `Ok`, so a binary built before migration N starts happily against a database at N and reads and writes tables whose shape it does not know. `restore` already refuses a backup newer than `MAX_SCHEMA_VERSION`; the server applies no such rule to the database it opens. A failed read of the stored version re-runs every migration.

## Why this is an RFC

This work was carried under `roadmap/schema-version-fail-closed/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/112-schema-version-fail-closed/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
