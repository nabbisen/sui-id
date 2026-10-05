# RFC 136 — handoff

**RFC.** [`../../accepted/136-dynamic-registration-is-legible.md`](../../accepted/136-dynamic-registration-is-legible.md)

**Status: Accepted 2026-10-05**, with its one open question settled at
acceptance in favour of a marker on the exception. **Not dispatched** — no
implementation package has been written yet.

## Contents

- [`security-review-2026-10-05.md`](security-review-2026-10-05.md) — no required
  changes. Verified that C15's transaction stamps `registered_via` separately
  from `clients::create`, which drops the row field; without that, D2's marker
  would never appear and the RFC's premise would be inverted.
