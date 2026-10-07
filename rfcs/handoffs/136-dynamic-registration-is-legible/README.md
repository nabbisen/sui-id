# RFC 136 — handoff

**RFC.** [`../../accepted/136-dynamic-registration-is-legible.md`](../../accepted/136-dynamic-registration-is-legible.md)

**Status: Accepted 2026-10-05.** Its one open question was settled at
acceptance in favour of a marker on the exception.

**Corrected 2026-10-07.** This file previously said step 1 *"is the whole of
RFC 136; there is no stage 2"*. **That was wrong.** RFC 136 has three closure
prerequisites and step 1 delivered two: the recorded defaults and the marker.
The third — *"a test fails if the distinction stops being shown"* — was never
dispatched, and a sweep of the code found no such test anywhere.
`sui-id-web` has three tests in the whole crate, all about colour contrast.
**A line asserting completeness is what stops anyone looking**, which is why
the correction is recorded here rather than the sentence simply deleted.

## Open work

**None.** All three closure prerequisites are met;
[`closure-review-2026-10-07.md`](closure-review-2026-10-07.md) recommends
closure and awaits nothing from the implementation role.

## Landed

- [`step2-the-marker-test-2026-10-07.md`](step2-the-marker-test-2026-10-07.md)
  — **landed `7cd3298`.** The missing test, at both render sites
  (`pages/clients.rs:37` and `:280`), asserted against the i18n string rather
  than a literal, and proven by mutation: removing the badge from either site
  fails that site's test and only that one. `sui-id-web` went from 3 tests
  to 7.

- [`step1-defaults-and-marker-2026-10-05.md`](step1-defaults-and-marker-2026-10-05.md)
  — prerequisites 1 and 2: both consent-policy defaults recorded with their
  reasoning, and the self-registered marker in the administrator-facing list.
  Reviewed 2026-10-05, accepted with no required changes — correctly, **for
  what it was dispatched to do**.

## Contents

- [`security-review-2026-10-05.md`](security-review-2026-10-05.md) — no required
  changes. Verified that C15's transaction stamps `registered_via` separately
  from `clients::create`, which drops the row field; without that, D2's marker
  would never appear and the RFC's premise would be inverted.
