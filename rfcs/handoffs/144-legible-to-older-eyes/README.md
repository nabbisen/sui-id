# RFC 144 — handoff

**RFC.** [`../../proposed/144-legible-to-older-eyes.md`](../../proposed/144-legible-to-older-eyes.md)
**Status.** **Proposed — build-prohibited** under RFC 000, which bars
implementation before acceptance. Nothing here is dispatched.

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
