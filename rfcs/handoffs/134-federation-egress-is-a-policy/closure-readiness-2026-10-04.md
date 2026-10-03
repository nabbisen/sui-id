# RFC 134 — closure readiness

**Date:** 2026-10-04
**Status: not a closure review.** All implementation has landed; **two closure
criteria are open and neither is the dev team's.** Nothing here is dispatched.
**By:** the architect, which wrote this RFC — so a closure review, when it comes,
will not be independent, as on RFCs 124, 128, 130, 132, 133. `ROADMAP.md` R1.

## The nine criteria

| # | Criterion | State |
|---|---|---|
| 1 | every control in D1–D4 at a single construction site | **met** — `egress.rs`'s one constructor |
| 2 | the resolver rejects both edges of every vendored prefix | **met** — 52 rows × 2 edges, the boundary mutation reproduced by me |
| 3 | the gate fails on a second egress client, on `resolve`/`resolve_to_addrs`, and on a per-request timeout | **met** — G19 |
| 4 | every discovery-supplied endpoint checked against the provider's origin set | **met** — `ValidatedDiscovery`, unconstructible unchecked |
| 5 | **the hostile-provider corpus in RFC 096's matrix passes** | **open — and the criterion is defective. See below.** |
| 6 | every Tier 1 bound is a recorded measurement with corpus and headroom | **met** — 5 providers reached, 2 recorded unreachable, derivations beside the constants |
| 7 | D5's three Tier 2 toggles set explicitly, not inherited | **met** |
| 8 | **the chain-size row filed in a tier with evidence** | **open — the owner's** |
| 9 | the Tier 3 amendment settled either way | **met** — "amend", 2026-10-03 |

**Seven of nine met.** Both open items are decisions, not code.

## Criterion 5 is a defect in a prerequisite I wrote

**RFC 134 cannot satisfy it, because it is not RFC 134's work.**

The hostile-provider corpus is **RFC 096's** deliverable. Measured in
`rfcs/accepted/096-upstream-oidc-federation-validation.md`:

- line 961 assigns "hostile-provider and token-substitution corpus" to **096-A**;
- line 964 assigns the "Hostile-provider matrix" to **096-C**;
- line 21's 096-A closure prerequisite names it among "discovery, transport,
  **JOSE, claims, state/nonce**, and cache/rotation evidence".

RFC 134's scope is transport and endpoint selection. **JOSE, claims and nonce
are not in it and were never dispatched under it.** I wrote this criterion into
RFC 134's closure prerequisites and it imports another RFC's unbuilt deliverable
— so as written, RFC 134 can never close on its own work.

**This is the same error class as D3's "admin UI": I wrote a requirement without
checking what it actually referred to.** Third time in this RFC.

**Two ways out, and the choice is not mine**, because narrowing a closure
prerequisite changes what must hold before closure, which RFC 000 line 43 makes
a material change returning the RFC to `proposed/`:

1. **Narrow criterion 5 to RFC 134's own scope** — "the transport rows of RFC
   096's matrix pass", which the D2 resolver corpus already satisfies. Honest
   about what this RFC built, and accepts the return to `proposed/` and
   re-acceptance that a material change costs.
2. **Leave it, and close RFC 134 only when 096-A ships.** Costs nothing now,
   but parks a completed, shipped RFC in `accepted/` indefinitely behind work
   that is not started — and `ROADMAP.md` already records that 23 RFCs sat in
   `accepted/` with none ever moving to `done/`.

**My recommendation is (1).** The work is done and the record should say so; a
criterion that cannot be met by the RFC that carries it is not a bar, it is a
mistake. The return cost is real and I would pay it, as with D5.

## Criterion 8 — the chain-size row

Unchanged and unblocked by anything above. `tls_info` exposes only the leaf
certificate, so the row cannot be Tier 1; a `rustls` `ServerCertVerifier` could
measure the intermediates, and the recommendation is not to, because it puts our
code in the certificate-verification path to enforce a byte bound. Filing it in
Tier 3 would make the approved amendment cover four rows rather than three,
which is more than was approved — hence it waits.

## What is not blocked

Nothing. There is no implementation work left in RFC 134 and no dispatch
pending. The dev team is free for other work.
