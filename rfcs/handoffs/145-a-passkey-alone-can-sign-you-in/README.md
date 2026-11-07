# RFC 145 — handoff

**RFC.** [`../../proposed/145-a-passkey-alone-can-sign-you-in.md`](../../proposed/145-a-passkey-alone-can-sign-you-in.md)
**Status.** **Proposed — build-prohibited** under RFC 000, which bars
implementation before acceptance. Nothing here is dispatched.

## Where it stands, 2026-11-07

Written by the architect on `@nabbisen`'s request. The RFC carries four
open questions (§Q): the `acr`/`amr` values relying parties receive, the
default of the operator switch, whether the password lockout also closes
the passkey path, and **whether it is an authorized exception to the
feature freeze**. The freeze admits four kinds of work, and this is none
of them. The recommendation is to schedule the build after RFC 094 M2b
closes, with that exception granted explicitly.

**Security review is Required.** Before the RFC can move to `accepted/`,
an independent design review must exist in this directory and be cited
from the RFC's `Independent design review` field. G11 (conditions 9 and
9c) enforces this. The architect wrote this design and cannot be that
reviewer, unless the owner invokes `ROADMAP.md` §S1c's exception, as he
did for RFC 143 and RFC 144.

## The stages, once accepted

Four, in the RFC's own order: a measuring prototype with no product
change, then store, then core and HTTP, then interface. Stage 1 exists
because two design points rest on reading the library rather than running
it: the modal mediation, and the `residentKey: "preferred"` edit. Its
result comes back here before stage 2 is dispatched.
