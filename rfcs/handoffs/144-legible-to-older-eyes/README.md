# RFC 144 — handoff

**RFC.** [`../../proposed/144-legible-to-older-eyes.md`](../../proposed/144-legible-to-older-eyes.md)
**Status.** **Proposed — build-prohibited** under RFC 000, which bars
implementation before acceptance. Nothing here is dispatched.

## Where acceptance stands, 2026-11-04

**`@nabbisen` accepted the design** — his word: *"Accepted."*

**The RFC stays in `proposed/` until his design review record exists**,
and that is a gate requirement rather than a formality.
`scripts/check-rfc-integrity.py` condition 9 refuses an Accepted RFC
whose `Security review` is `Required` and which carries no
`Independent design review` field, and condition 9c requires at least
one citation in that field to resolve under this directory. Moving the
file today would fail G11, and writing the field without the document
would be worse than failing it.

**He is reviewing it himself.** Asked how the field should be
satisfied, he answered: *"You design. I will review it."* So the
architect wrote the design and performs no review of it —
`ROADMAP.md` §S1c's self-review prohibition applies unmodified here,
because he did not invoke its exception as he did for RFC 143.

**Nothing is written on his behalf.** When his review exists it belongs
at `design-review-2026-11-04.md` in this directory, and the RFC's
`Independent design review` field cites it; the architect will
transcribe his findings into that file only if he asks, and will not
author the review itself.

**Two questions in the RFC remain open.** His acceptance message did
not address §5's AAA question for the semantic colours or §6's
sign-out visibility question, so both stand as written. An Accepted
RFC may carry open decisions — RFC 096 carried five — but the closure
prerequisites cannot be met while these two are unanswered.

## What exists

| document | what it is |
|---|---|
| *(the measurement)* | `.git-exclude/review-requests/ui-accessibility-for-older-users-2026-11-04.md` — the measured findings, held outside git because it is working correspondence, not a record |

The RFC's §1–§5 carry every number from that measurement, so the RFC is
self-contained; the file above is its working origin.

## How the measurements were taken

Reproducible, so a reviewer need not trust them:

- **Contrast.** WCAG 2.1 relative luminance, computed from the hex
  values in `crates/sui-id-web/src/tokens.rs` for both the `:root`
  light block and the dark overrides. `crates/sui-id-web/src/tokens/tests.rs`
  implements the same formula in Rust and is the authority; the figures
  in the RFC agree with it for every pair that test covers.
- **One correction made during measurement**, recorded because the
  method matters: dark-mode `--fg-muted` was first computed at 2.65:1 by
  pairing the **light** value `#5F5A66` against the dark surface. Dark
  mode overrides it to `#B8B2C4`, which is **8.59:1**. The error was the
  architect's and was caught before it reached the RFC. Any figure in
  the RFC for a dark pair uses the dark override.
- **Usage.** Each failing token's call sites were enumerated by grep over
  `crates/sui-id-web/src/` and read in context, which is how the dead
  `.subtle` class and the live `input::placeholder` were distinguished.

## What is open, and whose it is

Two questions in the RFC are deliberately left open rather than settled
here: how far to go on AAA for the semantic colours (§5), and whether
sign-out should be visible rather than behind the account menu (§6).

Acceptance also brings a review obligation this RFC does not pre-empt:
the header records `Security review. Required`, which on acceptance
means an `Independent design review` field with a durable reference
filed in this directory. Who performs it is settled by `ROADMAP.md` §S1
and is not restated here.

## Why `Security review. Required`

The architect judges the security surface small — this is colour, size
and a media query. But the template's alternative is *"Not required —
reason approved by NAME"*, and no such approval exists; writing one
would record an approval that was never given. `Required` is therefore
the honest value.
