# RFC 105 — the independent design review that did not happen

**This is not a review.** It is the record of a review that cannot now be
performed, why, and who carries the gap. RFC 105's header cites this file
because RFC 000 requires an `Independent design review` reference for a
security-sensitive RFC, and the honest reference is this, not a review.

**Date.** 2026-09-24
**Recorded by.** High-capability model, architect / deputy PM.
**Carried by.** `@nabbisen`, who accepted RFC 105 on 2026-09-24 with this gap
in view, on the architect's recommendation.

## What happened

RFC 105 was written on 2026-09-22 and never accepted. On 2026-09-24 the
architect named its handoff as the next thing to hand to the implementation
role, **without checking the RFC's status**. RFC 000 permits implementation only
from `accepted/`. The implementer built it, noticed, and said so in the first
lines of the review package rather than proceeding silently.

## Why the review cannot be performed now

The routing this project uses sends a design to **the role that must build
against it** — the implementation role. That role has now built RFC 105. It
cannot review its own design, under RFC 000's rule that the implementer cannot
be the sole approver.

There is no third role. So the independent design review of RFC 105 is not
merely late: **it is unavailable, permanently.** Pretending otherwise by
labelling the implementation package a review would be the precise failure this
project spent 2026-09-22 correcting.

## What exists in its place

- **A verified implementation package**, `rfc-105-audit-note-escaping-2026-09-24`:
  23 of 23 hunk hashes verified by the architect, nothing unclaimed, every gate
  re-run independently — 909 workspace tests, fmt, clippy with and without all
  features, the MSRV build, G13, G10a, G10b, G11, G15, G16.
- **Eight mutations where the handoff asked for one**, including one the
  implementer found was *not* caught (control characters that are not
  whitespace), after which they added the assertions and re-ran.
- **The implementer's own disclosures**, unprompted: the legacy hand-built notes
  the encoder does not cover, the pre-existing CSV quoting defect they declined
  to fix, `Cf` characters left unencoded, the 512-byte bound applying to the raw
  value, and a flaky test of their own making that they found and fixed.

None of that is a design review. It is evidence that the thing built matches
the thing specified, which is a different question from whether the thing
specified was right.

## What is therefore unreviewed

The **design decision itself**: percent-encoding rather than quoting, applied at
the note builder, with keys never encoded and historical rows never rewritten.
Its reasoning is in RFC 105 and in §1 of the implementation package, and it is
the architect's, checked by nobody independent.

The specific judgment a reviewer would have been asked to make: **that soundness
of operator queries is worth the legibility of a reason.** A reason with spaces
now reads `caller%20verified%20by%20call-back` in the CSV and in SQL. That is a
real cost to the humans who read audit logs, accepted on the architect's
judgment and the owner's, with no third view.

## The process fix

Twelve Proposed RFCs have handoffs in this tree. Any could be dispatched the
same way. The architect checks an RFC's status before naming its handoff as
dispatchable; this is a discipline, not a gate, and it is recorded here because
the failure it follows was not caught by anything.
