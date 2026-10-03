# RFC 134 — handoff

**RFC.** [`../../accepted/134-federation-egress-is-a-policy.md`](../../accepted/134-federation-egress-is-a-policy.md)

**Status: Accepted 2026-10-03, not yet dispatched.** Implementation is permitted
under RFC 000, but no dispatch has been written. Nothing here is work for the dev
team until one exists.

## Contents

- [`security-review-2026-10-03.md`](security-review-2026-10-03.md) — three
  required changes (R1 the missing origins column, R2 the `resolve()` bypass,
  R3 the per-request timeout override), all now folded into the RFC text.
- [`d5-can-the-rows-be-measured-2026-10-03.md`](d5-can-the-rows-be-measured-2026-10-03.md)
  — **analysis, not adopted.** Answers `@nabbisen`'s question of whether D5's five
  rows can be derived from measured reality. They largely can: three stop needing
  an amendment. Adopting it returns RFC 134 to `proposed/` under RFC 000, so it
  waits on his ruling and **the RFC is unchanged**.
