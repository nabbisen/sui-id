# RFC 150 — the design review record, 2026-11-16

**RFC.** [`../../accepted/150-an-administrator-can-edit-a-user.md`](../../accepted/150-an-administrator-can-edit-a-user.md)
**Reviewer.** `@nabbisen`, the accountable owner. He did not author it; the architect did.
**What was put to him.** RFC 150 as of `ea19944`.
**Outcome.** Accepted, 2026-11-16.

## The record, in his words

- His decision: *"All accepted."*, answering the architect's list, whose first
  item was to accept RFCs 150 and 151, and RFC 101 as refreshed.
- On §5 (should a username ever be changeable), in the same message: *"Partially
  agreed. However, it brings limitation: Removed names can not be used in the
  future. Never once registered. Is it for security ?"* §5 was then answered in
  the RFC with a proposal.
- On that proposal (rename allowed, old and deleted names reserved): *"Proceed."*
  The architect records this as approving the proposal. **If it meant only
  "carry on", §5 reopens**, and the username is read-only until it is
  answered.

- On §5's second question (what happens to a retired name), after the
  architect's options A–E: *"Agreed."*, to option D, reserved by default,
  released only by an administrator's deliberate action.

## A correction this record makes

§2's reason for keeping usernames fixed said the username *"is what the audit
log shows as the actor of everything they did"*. **That was false.** The audit
log records and shows the actor's internal ID (`crates/sui-id/src/http/handlers/admin/audit.rs:84`,
`crates/sui-id-web/src/pages/audit.rs:28`). The architect asserted it without
measuring it. It is corrected in the RFC.

## What this record does not contain

**No written findings reached the architect** beyond his words above. This
record invents none.
