# RFC 125 — The audit chain must be verified as a chain

**Status.** Proposed
**Security review.** Required
**Independent design review.** Found by the RFC 121 design review, 2026-09-29, by the implementation role, which proved it by execution in a disposable worktree. Confirmed independently by the architect by reading, who added the deletion and truncation consequences below.
**Design prerequisites.** None. This RFC is an urgent integrity fix and takes precedence over RFCs 121–124, all of which are about this control or its display.
**Implementation prerequisites.** None.
**Closure prerequisites.** Verification detects every single-row rewrite, every deletion and every truncation within the window it reports on, including a rewrite whose own hash was recomputed; the window it covers is stated wherever a result is shown; and no document claims a property that no test defends.
**Tracks.** Audit integrity.
**Touches.** `crates/sui-id-store/src/repos/audit.rs`, `crates/sui-id-store/src/migrations/0009_audit_hash_chain.sql` (comment only), `ROADMAP.md` §S2, and whatever `docs/` states the property.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/125-the-chain-must-be-verified-as-a-chain/README.md`](../handoffs/125-the-chain-must-be-verified-as-a-chain/README.md)

## Summary

The audit log builds a hash chain correctly and **never verifies it as one**.

`verify_chain_tail` recomputes each row's hash from **that row's own
`prev_hash` column** and compares it to that row's own `hash` column. It never
compares row N's `hash` against row N+1's `prev_hash`. The linkage that makes it
a chain is written and never read.

## What that costs, stated exactly

Against the attacker the module itself names — raw SQL access, no application
code:

- **Rewriting a row is undetected.** Change the content, recompute that row's
  own hash from its own unchanged `prev_hash`, write both. One `UPDATE`. No
  other row is touched, because no other row is consulted.
- **Deleting a row is undetected.** No row's `prev_hash` is ever compared to its
  predecessor's `hash`, and sequence numbers are never checked for gaps.
- **Truncating the log is undetected**, for the same reason.

What *is* detected is a modification that does **not** recompute the row's hash
— that is, a careless attacker. That is the only case the existing test
exercises, and the test's own comment reasons about linkage the code does not
perform, which is why the gap survived review.

## Three statements in the tree are false

- `crates/sui-id-store/src/repos/audit.rs:4-10`: *"To rewrite or delete row N
  you must recompute every subsequent row's hash."* You must recompute **that
  row's**, and no other.
- `crates/sui-id-store/src/migrations/0009_audit_hash_chain.sql:5-7`: *"An
  attacker who … tries to delete or rewrite a row breaks the chain at the next
  row, which is detectable."* The next row is never examined.
- `ROADMAP.md` §S2 records the chain as tamper-evident within its trust
  boundary. It is not tamper-evident within that boundary either, against a
  competent attacker inside it.

This is the shape RFC 098 exists for — a claim the code does not support — and
it is the most consequential instance of it the programme has found, because
the claim is about the mechanism the product offers as evidence.

## Decision

**D1 — Verification compares linkage.** Row N's `prev_hash` must equal the
`hash` of row N−1. A mismatch is a break, reported at the row where it is found.

**D2 — Sequence continuity is part of verification.** A gap in `seq` within the
window is a break. Without it, deletion stays invisible however well linkage is
checked at the surviving rows.

**D3 — The window's edge is honest.** The oldest row in the window has no
predecessor *in the window*. Verification either reads one row further to check
that boundary, or reports plainly that the boundary was not checked. It does not
silently treat the edge as verified.

**D4 — The genesis row is a stated case, not an accident.** The first row's
empty `prev_hash` is legitimate; a *later* row with an empty `prev_hash` is not.
The rule says which is which, and a test covers the difference.

**D5 — Each of the three attacks has a test that fails before this RFC.**
Rewrite-with-recomputed-hash, deletion, truncation. Without them this fix is
unproven, and the existing suite has already demonstrated that a test can pass
while reasoning about a property the code lacks.

**D6 — The existing test is corrected, not left.**
`tampering_with_a_row_makes_chain_verification_fail` passes for a different
reason than its comment gives. The comment is wrong and the name overclaims;
both change, because a test that misdescribes what it proves is how this was
missed.

**D7 — No document claims more than a test defends.** The module doc, the
migration comment and `ROADMAP.md` §S2 are corrected in the same change. Where
the honest statement is narrower, it is the narrower statement that ships.

## What this RFC does not do

It does not add an external anchor. The chain remains evidence only against an
attacker outside the application's trust boundary, exactly as `ROADMAP.md` §S2
says — that limit is real, acknowledged and out of scope. **This RFC is about
the chain failing to deliver the property it claims *within* that limit.**
