# RFC 119 — the design review record

**What this file is.** The durable record RFC 119's
`Independent design review` field cites. It exists because that field
must say **what the review was and was not independent of**, rather than
leaving the word to imply a third party — the errata in `rfcs/README.md`
§*Errata: the `Independent design review` field*.

## Who reviewed it, and under which rule

**The architect, at `@nabbisen`'s request.** His words, 2026-11-01:
*"Review the questions carefully by yourself with my fundamental
philosophy. Any question remains?"*

`ROADMAP.md` **§S1c** (ruled 2026-10-01) prohibits architect self-review
by default and permits it **where the owner asks** — *"Owner sometimes
asks architect to review some part(s) of specific design etc.
beforehand"* — and requires the record to say he asked. He asked; this
says so.

**The architect wrote RFC 119, its handoff and this record, so this
review is *not* independent of the RFC.** The field names it as the
review because §S1a places design review with `@nabbisen` or an
external architect he engages; no external architect was engaged, which
§S1a makes his option rather than a requirement.

**Why the earlier route failed, recorded so it is not repeated.** This
RFC's original review request named the *mid-capability model,
implementation role* as reviewer, routed on §S1 as it stood 2026-08-26.
Five days later §S1a settled design review as *"`@nabbisen`, or an
external architect he engages. **Not the dev team.**"* From that date
the request named someone who may not perform it, so there was nobody
to hand it to — and **RFC 119 sat at Proposed for 36 days** for that
reason alone, not for want of capacity.

## What the review changed

**All three of RFC 119's open questions are answered**, in the RFC's own
§*Answers, 2026-11-01*. Two of the three changed position under review:

- **Q1 — the RFC was too pessimistic.** It called host-driven lockout
  inflation possibly *"the first obligation that cannot be made safe"*.
  Half of it is closed by an attempt identifier stable across the host's
  retries, using machinery that already exists
  (`federation_login_attempt`, RFC 096-B1). Only *suppression* is
  unclosable, and it is not lockout-specific.
- **Q2 — the architect's first framing was wrong.** It proposed *"a
  narrower claim"*. Measurement showed the claim does not narrow, it
  **goes away**: the chain is `SHA-256(prev_hash || canonical_row_bytes)`
  with no key (`repos/audit.rs:3`), so a host with database write access
  recomputes it; and nothing in-process can keep a secret from the
  address space it runs in. `@nabbisen` reached this independently —
  *"When embedded, it seems impossible to me if sui-id can not guarantee
  the log."*
- **Q3 — settled on his framing**, *"It is contract instead of UI
  function."*

**This review also found a defect outside the RFC**, by the same
reasoning: `docs/src/guides/operators.md` claimed a SQL-level rewrite
*"never"* avoids leaving a detectable mismatch. It does, for want of a
secret in the chain. **Shipped, operator-facing, and corrected
2026-11-01** — the RFC question was hypothetical; that sentence was
load-bearing for what an operator believes after a database compromise.

## What this review does not discharge

**The closure prerequisites are untouched** and remain as written: every
security property either carried by the module, written here as a
numbered host obligation with a failure mode and a conformance test, or
named as a property an embedding cannot have. **Q2's answer puts the
audit guarantee in that third category** — the first and most
consequential instance of it.

**And this RFC still ships no code.** Accepting it authorizes the
contract and unblocks the store-boundary RFC, which its own header
forbade being written until then.
