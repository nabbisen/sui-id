# RFC 100 — the independent design review record, 2026-11-14

**RFC.** [`../../accepted/100-master-key-rotation-recovery.md`](../../accepted/100-master-key-rotation-recovery.md)
**Reviewer.** `@nabbisen`, the accountable owner. He did not author RFC
100, and he did not write its 2026-11-14 refresh. Both are the architect's
(the original requirements-architect role on 2026-07-28, and the current
one).
**What was reviewed.** RFC 100 as of `9e2bd00`, including its *Refresh
against the code as built* section and the proposed answers to both open
questions. That review also covers the RFC's design prerequisite, the
filesystem-atomicity threat delta, which the refresh measured (the
as-built table and the three open crash prefixes).
**Outcome.** Accepted, 2026-11-14.

## The record, in his words

- Asked who would give the independent review the RFC requires:
  *"Understood. I will review it."*
- His decision: *"RFC 100 and 146 are accepted."*
- Asked whether his review covered RFC 146 as well as RFC 100, he chose the
  answer *"Yes, I reviewed both."*

## What this record does not contain

**No written findings reached the architect.** This record does not invent
any, and it says nothing about what the review examined beyond the text it
was given. The two proposed answers (`--check` instead of a dry-run; a
finding, never automatic removal) and the four design changes stand as
written, because the RFC was accepted with them.

## What acceptance does not settle

The RFC's **implementation prerequisites** still include *"the
crash-injection harness approved"*. Nothing beyond `0.91.0` stage 1 (the two
rotation fixes, dispatched ahead of acceptance) is built until that
approval exists.
