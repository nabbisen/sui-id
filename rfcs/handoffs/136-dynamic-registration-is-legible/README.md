# RFC 136 — handoff

**RFC.** [`../../accepted/136-dynamic-registration-is-legible.md`](../../accepted/136-dynamic-registration-is-legible.md)

**Status: Accepted 2026-10-05**, with its one open question settled at
acceptance in favour of a marker on the exception. **Dispatched** —
[`step1-defaults-and-marker-2026-10-05.md`](step1-defaults-and-marker-2026-10-05.md)
is the whole of RFC 136; there is no stage 2.

## Contents

- [`security-review-2026-10-05.md`](security-review-2026-10-05.md) — no required
  changes. Verified that C15's transaction stamps `registered_via` separately
  from `clients::create`, which drops the row field; without that, D2's marker
  would never appear and the RFC's premise would be inverted.
