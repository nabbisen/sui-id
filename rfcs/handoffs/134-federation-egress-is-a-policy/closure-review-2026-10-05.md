# RFC 134 — closure review

**Date:** 2026-10-05
**Reviewed by:** the architect, **which wrote this RFC, every one of its
dispatches, and the criteria below.** Not independent, as on RFCs 124, 128, 130,
132, 133, 135 and 136. Carried under `ROADMAP.md` R1's residual.
**Recommends closure. Does not close it.** The `Closure approved by.` field
stays empty until his own words exist to put in it.

## Level B

**`78a2decb`, run `37266203375` — 26 jobs, 0 skipped, all green.**

Verified as Level B rather than assumed: every one of the **23** entries in
`contracts/gate-inputs.toml`'s `[gates]` has a green job on that commit; the
three extra green jobs are A3.2, A3.4 and the changed-scope job. The tip
`e89986a5` is **not** Level B — it is docs-only and skipped 11 Rust lanes — so
this review cites `78a2decb`, which is the commit that first carries RFC 134
complete.

## The nine criteria

| # | Criterion | Evidence |
|---|---|---|
| 1 | every D1–D4 control at a single construction site | `runtime/egress.rs`'s one constructor; G19 refuses a second |
| 2 | the resolver rejects both edges of every vendored prefix | 52 rows × 2 edges, boundaries computed in Python's `ipaddress`, parity-asserted against the table. **I reproduced the boundary mutation**: narrowing `10.0.0.0/8` to `/9` fails on `10.255.255.255` |
| 3 | the gate fails on a second client, on `resolve`/`resolve_to_addrs`, on a per-request timeout | G19, three conditions, with negative tests |
| 4 | every discovery endpoint checked against the origin set | `ValidatedDiscovery` — private fields, one validating constructor, so an unchecked endpoint **does not compile** |
| 5 | the **transport rows** of RFC 096's matrix pass | D2's resolver corpus. Narrowed 2026-10-04 from "hostile-provider corpus", which named RFC 096's deliverable, not this one's |
| 6 | every Tier 1 bound a recorded measurement | 5 providers reached, 2 recorded unreachable, derivations beside the constants |
| 7 | D5's three Tier 2 toggles set explicitly, not inherited | the three `http1_*` calls |
| 8 | the chain-size row filed in a tier with evidence | Tier 3, approved 2026-10-05 — `tls_info` exposes only the leaf |
| 9 | the Tier 3 amendment settled | "amend", 2026-10-03; four rows after criterion 8 |

**Plus D3's maintenance path, added by the 2026-10-05 re-scope and built in step
7.** Verified in the shipped tree: `startup.rs:305` calls
`reconcile_federation_provider`, which compares and calls
`update_allowed_origins` **only on a difference**; four `tracing::warn` sites
cover the fields that are *not* reconciled; and
`docs/src/reference/configuration.md:251-256` states which fields take effect
after the provider exists and which are read only at creation.

**The defect that caused the re-scope is gone**: an administrator can add a
second origin by editing config and restarting, without deleting the provider
and cascading away every federation link to it.

## What this review does not establish

- **It is not independent.** I wrote the RFC, its six dispatches, and these
  criteria. The implementation role verified measurements on each package; that
  is corroboration, not approval and not independence.
- **Two controls rest on textual proxies.** G19 greps for
  `Client::builder()`, `resolve(` and `.timeout(`; G20's seam check greps for
  `ClassATx`. Each proves the construct is absent **as written**, not that no
  path evades it by alias or macro. Both docstrings say so.
- **The vendored IANA table is current to 2025-10-09** and nothing re-checks it.
  A registry row added later is not detected; the date in the module header is
  the only signal.
- **RFC 096-A is untouched.** JOSE, claims, state/nonce and the
  hostile-provider corpus remain unbuilt. Criterion 5 was narrowed precisely so
  this RFC would stop claiming them.

## The lifecycle, recorded rather than tidied

RFC 134 was accepted and returned to `proposed/` **three times** across
2026-10-03 to 10-05, then re-accepted each time. **Two of the three returns were
my errors**: closure criterion 5 imported another RFC's unbuilt deliverable, and
D3 shipped an origin set that was write-once. The third, D5's split by
enforceability, came from `@nabbisen`'s question and improved the design.

A fourth error of mine — "admin UI to maintain it", for a UI that does not
exist — was corrected in place rather than by a return, and was later subsumed
by the D3 re-scope that the same mistake had concealed.

**No return was caused by the implementation.** Every package was correct
against the dispatch it was given; what kept failing was my specification of it.

## Recommendation

Close RFC 134 to `done/`, `Status.` **Implemented**, `Closure reviewed on.`
2026-10-05, `Closure evidence.` this review plus Level B run `37266203375` on
`78a2decb`.
