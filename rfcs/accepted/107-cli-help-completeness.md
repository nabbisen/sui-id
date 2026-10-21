# RFC 107 — `sui-id --help` names every subcommand the binary accepts

**Status.** Accepted
**Accepted on.** 2026-10-18
**Approved by.** `@nabbisen`, 2026-10-18: *"RFC 107 is accepted."*
**Independent design review.** [`design-review-2026-10-14.md`](../handoffs/107-cli-help-completeness/design-review-2026-10-14.md) — by the implementation role, under RFC 139's dispatch §6. **Genuinely independent**, unlike most reviews in this repository: the architect authored RFC 107 and did not review it. The review confirmed the problem by reading source rather than repeating the handoff, found one factual error in the Summary (which named `setup` as missing when `cli.rs:680` carries it), and required its correction before acceptance. **That correction landed on 2026-10-15** and is marked in the Summary below, so the review's only pre-acceptance condition is discharged. Its §5 records one adjacent finding held outside this repository; the review states it is not a blocker for accepting RFC 107.
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

`print_help` omits two subactions the binary dispatches — `admin rotate-metrics-token` and `admin issue-registration-token` — and the admin dispatcher's own error message lists the admin subactions a third time. *(Corrected 2026-10-15: this sentence also named `setup`, which is not omitted — `cli.rs:680` carries it, and so did `9f6acdb`, the commit this RFC names as its own measurement point. Found by the implementation role's independent design review, which checked the claim against both trees rather than taking it. Two subactions and the `find_subcommand` defect below are still worth this RFC.)* Three hand-written lists have drifted apart. The same drift breaks dev mode: `find_subcommand` skips a flag's value only for `--config`, `--to` and `--from`, so the value after `--dev-seed` or `--dev-bind` is read as a subcommand.

## Why this is an RFC

This work was carried under `roadmap/cli-help-completeness/` until 2026-09-22, when
`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/`
and nowhere else. RFC 000 requires every `rfcs/handoffs/NNN-slug/` directory
to correspond to an existing RFC number, and leaves to each project the
question of what an RFC covers — so operational and repair work gets one here,
on the same terms as a feature.

The RFC is **Accepted** as of 2026-10-18, so it is implementation-eligible
under RFC 000 `:49-50`. The specification, its evidence requirements and its
history are in the handoff, unchanged by the move.

## Decision

See the [handoff](../handoffs/107-cli-help-completeness/README.md) for the full specification. In outline, the
closure prerequisites above state what must be true before this RFC can ship,
and the handoff states how to get there and what evidence is required.

---

**Note on the `Independent design review` field above — added 2026-10-21.**
The review it names was performed by the implementation role. It was
independent of this RFC's *author*; it was not independent of its
*implementer*, and the field's name does not say so. The governing rule in
`docs/development-specification.md` — *"The implementer cannot be the sole
approver of a security-sensitive design or its closure evidence"* — is met.
The sign-off of record is `@nabbisen`'s. What was irregular is that design
review belongs to the architect role under the project's role documents, and
these reviews were delegated away from it. The review's measured findings
stand and this RFC's acceptance is not reopened. The single statement this
note points at is *Errata: the `Independent design review` field* in
[`rfcs/README.md`](../README.md).
