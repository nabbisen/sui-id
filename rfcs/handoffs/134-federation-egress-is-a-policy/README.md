# RFC 134 — handoff

**RFC.** [`../../accepted/134-federation-egress-is-a-policy.md`](../../accepted/134-federation-egress-is-a-policy.md)

**Status: Accepted 2026-10-05** on D3's re-scope. **D3's maintenance path is now
dispatchable and is not built** — the origin set is still write-once at provider
creation, and changing it still means deleting the provider, which cascades away
every federation link to it.

**Steps 1–6 have landed, but RFC 134 is no longer complete.** D3's re-scope adds
a maintenance path that is not built and not dispatched. Closure is blocked on two items, neither of
them the dev team's — see
[`closure-readiness-2026-10-04.md`](closure-readiness-2026-10-04.md). Step 5 needed no implementation; the
architect answered it.

| Step | State |
|---|---|
| 1 — D1, D4, D5 Tier 2 | **landed** `981b228` |
| 2 — D3 | **landed** `842b75b` |
| 3 — D2 | **landed** `2425374` |
| 4 — D5 Tier 1 | **landed** `a89ea83` |
| 5 — chain-row determination | answered, no work |
| 6 — D5 Tier 3 | **landed** `a89ea83` |

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
  — **landed `981b228`.** D1, D4 and D5 Tier 2: the single egress constructor, the G19
  gate over the three bypass routes, and the `http1_*` strictness toggles set
  explicitly. Carries the architect's answer to step 5 as information only.
- [`step2-endpoint-origin-validation-2026-10-03.md`](step2-endpoint-origin-validation-2026-10-03.md)
  — **landed `842b75b`.** D3: the provider's allowed-origin set, and a
  `ValidatedDiscovery` newtype that makes an unchecked discovery endpoint
  impossible to use. Carries a correction to RFC 134 D3's "admin UI", which does
  not exist — providers are seeded from config at startup.
- [`step3-validating-resolver-2026-10-03.md`](step3-validating-resolver-2026-10-03.md)
  — **landed `2425374`.** D2: the vendored IANA prefix table and a
  `reqwest::dns::Resolve` implementation that rejects the whole answer on any
  bad address and returns exactly one. Carries four verified facts the
  implementation would otherwise have to rediscover — chiefly that resolvers
  return port `0`, and that `is_global()` is still unstable on 1.99.0.
- [`steps4-6-response-bounds-2026-10-03.md`](steps4-6-response-bounds-2026-10-03.md)
  — **landed `a89ea83`.** D5 Tier 1 (response bounds derived
  from a measured provider corpus) and Tier 3 (the approved three-row
  amendment's evidence). Records that two of the matrix's four JSON caps — depth
  and duplicate keys — already hold in `serde_json`/`serde` and should be
  evidenced rather than built. **The certificate-chain row is excluded**; its
  tier is still open.
