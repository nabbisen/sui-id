# RFC 108 — Confirm screens for client disable and secret rotation; the secret leaves the URL

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFCs 030 and 059, both shipped, already require a confirm screen for every dangerous operation. This RFC implements that contract rather than deciding it anew.
**Implementation prerequisites.** None.
**Closure prerequisites.** Both operations have a confirm screen naming the impact and the reversibility; no secret appears in any URL, redirect, log line or `Referer`; a test fails if either regresses.
**Tracks.** Dangerous-operation contract.
**Touches.** `crates/sui-id/src/http/handlers/admin/clients.rs`, `crates/sui-id-web/src/pages/confirm.rs`, `crates/sui-id-web/src/pages/clients.rs`, `crates/sui-id-i18n/`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/108-client-confirm-screens/README.md`](../handoffs/108-client-confirm-screens/README.md)

## Summary

Eight dangerous operations exist and six have a confirm screen. Client disable and client secret rotation require `_confirmed=1` and a fresh step-up but have no confirm screen, so the form posts directly — against the contract RFCs 030 and 059 already set. Separately, `clients_rotate_secret_post` redirects to `…/edit?rotated_secret=<secret>`, putting the secret into browser history, any URL-recording proxy or access log, and `Referer`.

## Why this is an RFC

This work was carried under `roadmap/client-confirm-screens/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Proposed**: the design below has not been approved. The
specification, its evidence requirements and its history are in the handoff,
unchanged by the move.

## Decision

See the [handoff](../handoffs/108-client-confirm-screens/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.
