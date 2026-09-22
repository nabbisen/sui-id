# RFC 115 — Creating a user without choosing their password

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** Three forks are open and belong to `@nabbisen`, stated in the handoff: creating a second administrator against D5's refusal of administrator targets; the five-per-hour throttle against bulk provisioning; and whether `must_change` is enforced or deleted.
**Implementation prerequisites.** RFC 103 Implemented — this reuses its recovery-link issuance as the replacement for the password field.
**Closure prerequisites.** No code path lets anyone other than the account holder choose or learn a password, which is RFC 103's own prerequisite 4; `must_change` is enforced or gone; no form's `Debug` can print a password.
**Tracks.** Account integrity. Found by the T2 re-review of 2026-09-22.
**Touches.** `crates/sui-id-core/src/identity/admin/users.rs`, `crates/sui-id/src/http/handlers/admin/users.rs`, `crates/sui-id-web/src/pages/users.rs`, `crates/sui-id-store/src/commands.rs`, `crates/sui-id/src/http/handlers/me_security/forms.rs`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/115-user-creation-without-a-password/README.md`](../handoffs/115-user-creation-without-a-password/README.md)

## Summary

RFC 103 removed U06, the path that let an administrator re-set an existing user's password, and left untouched U01, which sets every local user's first one. `/admin/users/new` carries a required password field, stored with `must_change: false`, so an administrator knows a working password for every account they created until its holder changes it, and using it later is an ordinary sign-in. RFC 103's closure prerequisite 4 and threat T2 are therefore not met. Two dependent defects: `must_change` is written in three places and read by nothing, and all eight password-bearing HTTP forms derive `Debug` over their plaintext fields.

## Why this is an RFC

This work was carried under `roadmap/user-creation-without-a-password/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/115-user-creation-without-a-password/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
