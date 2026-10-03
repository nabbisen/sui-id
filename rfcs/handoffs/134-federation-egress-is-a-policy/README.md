# RFC 134 — handoff

**RFC.** [`../../accepted/134-federation-egress-is-a-policy.md`](../../accepted/134-federation-egress-is-a-policy.md)

**Status: Accepted 2026-10-03** on the returned text, with the Tier 3 amendment
approved in the same message. It was accepted, returned to `proposed/` for a
material improvement to D5, and re-accepted, all on 2026-10-03; the lifecycle
history in the RFC records each step.

**Not dispatched.** Implementation is permitted under RFC 000, but no dispatch
exists yet, so nothing here is work for the dev team.

## Contents

- [`security-review-2026-10-03.md`](security-review-2026-10-03.md) — three
  required changes (R1 the missing origins column, R2 the `resolve()` bypass,
  R3 the per-request timeout override), all now folded into the RFC text.
- [`d5-can-the-rows-be-measured-2026-10-03.md`](d5-can-the-rows-be-measured-2026-10-03.md)
  — **analysis, not adopted.** Answers `@nabbisen`'s question of whether D5's five
  rows can be derived from measured reality. They largely can: three stop needing
  an amendment. **Adopted 2026-10-03** — the return was approved and D5 is now
  split by enforceability into Tiers 1–3 plus one unresolved row.
