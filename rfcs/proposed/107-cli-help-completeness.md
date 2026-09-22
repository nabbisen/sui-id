# RFC 107 — `sui-id --help` names every subcommand the binary accepts

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** Every subcommand the binary dispatches appears in `--help`; the lists cannot drift again without a test failing; a flag's value is never mistaken for a subcommand.
**Tracks.** Usability and operator trust.
**Touches.** `crates/sui-id/src/cli.rs`, `crates/sui-id/src/main.rs`, `docs/src/reference/configuration.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/107-cli-help-completeness/README.md`](../handoffs/107-cli-help-completeness/README.md)

## Summary

`print_help` omits three subcommands the binary dispatches — `setup`, `admin rotate-metrics-token` and `admin issue-registration-token` — and the admin dispatcher's own error message lists the admin subactions a third time. Three hand-written lists have drifted apart. The same drift breaks dev mode: `find_subcommand` skips a flag's value only for `--config`, `--to` and `--from`, so the value after `--dev-seed` or `--dev-bind` is read as a subcommand.

## Why this is an RFC

This work was carried under `roadmap/cli-help-completeness/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/107-cli-help-completeness/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
