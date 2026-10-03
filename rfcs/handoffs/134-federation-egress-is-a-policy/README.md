# RFC 134 — handoff

**RFC.** [`../../proposed/134-federation-egress-is-a-policy.md`](../../proposed/134-federation-egress-is-a-policy.md)

**Status: returned to `proposed/` on 2026-10-03** under RFC 000's
return-for-review rule, after an owner-approved material change to D5. It was
Accepted earlier the same day; the design was not found wrong, it was found
improvable. **Implementation is prohibited while it sits in `proposed/`**, and
nothing here is work for the dev team.

## Contents

- [`security-review-2026-10-03.md`](security-review-2026-10-03.md) — three
  required changes (R1 the missing origins column, R2 the `resolve()` bypass,
  R3 the per-request timeout override), all now folded into the RFC text.
- [`d5-can-the-rows-be-measured-2026-10-03.md`](d5-can-the-rows-be-measured-2026-10-03.md)
  — **analysis, not adopted.** Answers `@nabbisen`'s question of whether D5's five
  rows can be derived from measured reality. They largely can: three stop needing
  an amendment. **Adopted 2026-10-03** — the return was approved and D5 is now
  split by enforceability into Tiers 1–3 plus one unresolved row.
