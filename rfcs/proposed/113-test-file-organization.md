# RFC 113 — Test modules live in their own files

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** The rule this enforces is a project instruction, not a design decision made here.
**Implementation prerequisites.** None.
**Closure prerequisites.** No file under `crates/src` carries an inline `mod tests` block, measured by the command recorded in the handoff; the count is pinned so it cannot grow again.
**Tracks.** Code organisation.
**Touches.** `crates/**/src/*.rs` — test modules only; no production behaviour changes.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/113-test-file-organization/README.md`](../handoffs/113-test-file-organization/README.md)

## Summary

`.git-exclude/rules/project-instructions-rust.md` §Testing Guidelines requires that under `src/`, test modules are separate files — `src/some_mod.rs` plus `src/some_mod/tests.rs` — and names an inline `#[test]` module inside `src/some_mod.rs` as the anti-pattern. Files under `crates/` still carry inline `mod tests { … }` blocks. `commands.rs` and its runner split are done; `registry.rs` is next.

## Why this is an RFC

This work was carried under `roadmap/test-file-organization/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/113-test-file-organization/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
