# RFC 146 — the independent design review record, 2026-11-14

**RFC.** [`../../accepted/146-totp-step-selection-is-proved.md`](../../accepted/146-totp-step-selection-is-proved.md)
**Reviewer.** `@nabbisen`, the accountable owner. He did not author RFC 146,
which is the architect's.
**What was reviewed.** RFC 146 as of `9e2bd00`, with the scoping note it
formalises
([`../../../verification/verus-scoping-note-2026-11-14.md`](../../../verification/verus-scoping-note-2026-11-14.md)),
whose §2 holds the proof's source and the four recorded runs.
**Outcome.** Accepted, 2026-11-14.

## The record, in his words

- His decision: *"RFC 100 and 146 are accepted."*
- Asked whether his review covered RFC 146 as well as RFC 100, he chose the
  answer *"Yes, I reviewed both."*
- In the same answer he raised the project's long-term position on Verus:
  *"I wonder if it is better for us to update documentation to define what
  is verus's position and how verus is used in the project from view of long
  term development and maintenance."* The architect's proposal for that is
  in `verification/README.md`. It is not part of this RFC.

## What this record does not contain

**No written findings reached the architect.** This record does not invent
any.

## What acceptance does not settle

The build: RFC 146's implementation prerequisites include M7, the
real-environment soak, complete, with no feature-freeze exception.
