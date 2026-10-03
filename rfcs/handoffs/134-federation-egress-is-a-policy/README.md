# RFC 134 — handoff

**RFC.** [`../../accepted/134-federation-egress-is-a-policy.md`](../../accepted/134-federation-egress-is-a-policy.md)

**Status: Accepted 2026-10-03** on the returned text, with the Tier 3 amendment
approved in the same message. It was accepted, returned to `proposed/` for a
material improvement to D5, and re-accepted, all on 2026-10-03; the lifecycle
history in the RFC records each step.

**Step 1 is dispatched** — see below. Steps 2, 3, 4 and 6 are not. Step 5 is
answered by the architect and needs no implementation.

## Contents

- [`security-review-2026-10-03.md`](security-review-2026-10-03.md) — three
  required changes (R1 the missing origins column, R2 the `resolve()` bypass,
  R3 the per-request timeout override), all now folded into the RFC text.
- [`d5-can-the-rows-be-measured-2026-10-03.md`](d5-can-the-rows-be-measured-2026-10-03.md)
  — **analysis, not adopted.** Answers `@nabbisen`'s question of whether D5's five
  rows can be derived from measured reality. They largely can: three stop needing
  an amendment. **Adopted 2026-10-03** — the return was approved and D5 is now
  split by enforceability into Tiers 1–3 plus one unresolved row.
- [`step1-constructor-and-gate-2026-10-03.md`](step1-constructor-and-gate-2026-10-03.md)
  — **dispatched.** D1, D4 and D5 Tier 2: the single egress constructor, the G19
  gate over the three bypass routes, and the `http1_*` strictness toggles set
  explicitly. Carries the architect's answer to step 5 as information only.
