# RFC 136 — security review

**Date:** 2026-10-05
**Reviewed by:** the architect, **which authored RFC 136.** Not independent of
its subject, as on RFCs 124, 128, 130, 132, 133, 134 and 135. Carried under
`ROADMAP.md` R1's residual.
**Ordering:** written **after** `@nabbisen`'s acceptance, because G11 refused the
move to `accepted/` without it. The better order is the one RFC 133 used, where
the review preceded the decision.

## Verdict

**No required changes.** Two things were worth checking and both hold.

### The marker's premise — checked, because it could have been false

D2 shows a marker when `registered_via = 'dynamic'`. **`clients::create` does
not write that column** — its own doc comment says so
(`crates/sui-id-store/src/repos/clients.rs:66-68`), and
`dynamic_register.rs:199` sets the field on the row struct where `create` then
**drops it**. Had that been the whole story, **a dynamically registered client
would carry `'admin'` and the marker would never appear** — the RFC's premise
would be inverted and D2 would actively mislead.

It is not the whole story. C15's transaction stamps it separately:
`crates/sui-id-store/src/commands.rs:3013` takes `row.registered_via` and
`:3021` calls `set_registered_via_within_tx` inside the same transaction, with
a comment at `:2998-3001` explaining why that call is not redundant with the row
field. **So the column is correct and the marker is trustworthy.**

**This was the one finding that would have mattered**, and it is the same
dead-struct-field shape as the `consent_policy` bug this whole line of work
began with.

### Can a registering client influence the marker?

**No.** `registered_via` is written only by `set_registered_via` (C11) and by
C15's transaction, both server-side, from a value the server chooses —
`RegistrationSource::Admin` at `admin/clients.rs:92`, `::Dynamic` at
`dynamic_register.rs:199`. Nothing in the `/oauth2/register` request body
reaches it.

### Exposure

The marker is rendered on `/admin/clients`, behind the admin middleware that
already gates the page. It shows an administrator a fact about a client in a
list they can already read in full. **No new exposure.**

## What this review does not cover

- **The enable-confirmation page**, which is out of RFC 136's scope and deferred
  until after the federation work. This review says nothing about it.
- **Whether `FirstTime` is the right policy.** D1 records the existing defaults
  and their reasoning; it does not change them, and the choice was settled
  separately on 2026-10-05.
- **It is not independent.** I wrote the RFC and this review. R1 keeps carrying
  that.
