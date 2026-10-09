# RFC 096-A — completion assessment

**Written.** 2026-10-09 JST, by the architect.
**Subject.** RFC 096's stage **096-A** only — validation and transport.
**What this is.** The architect's assessment that 096-A's evidence is complete,
which RFC 096 `:21` makes the input to `@nabbisen`'s acceptance:
*"…are accepted by `@nabbisen`, on the architect's assessment."*
**What this is not.** It does **not** accept 096-A, and it does **not** close
RFC 096. Closure in this RFC is **per stage**; 096-B1, 096-B2 and 096-C remain,
and the RFC stays **Accepted** in `rfcs/accepted/`. Nothing here moves it.

**Not independent, and that is on the record.** The architect wrote this RFC's
fifteen dispatches, its stage plan, and the criteria assessed below. This
assessment is therefore **not** independent of the work it judges, and
`@nabbisen` is the approver — which is what RFC 000 requires when no independent
role exists. The implementation role verified each package's own measurements
and corrected the architect in **six consecutive stages**; that is corroboration,
recorded beside the findings, and it is neither approval nor independence.

## Level B

**CI run [`37911563157`](https://github.com/nabbisen/sui-id/actions/runs/37911563157)
on commit `56c91de`** — conclusion `success`, **27 jobs, 0 skipped, 0 failed**.
Verified to cover **all 24 entries** in `contracts/gate-inputs.toml`'s
`[gate_owners]` (RFC 131 D2): G01–G06, G07, G07b, G08, G09a/b, G10a/b, G11–G21.
The three remaining jobs are `A3.2 gate-matrix negative self-tests`,
`A3.4 gate-inputs.toml enforcement` and `Compute changed scope` — the two suites
that sit outside `[gates]` are green on this run as well.

Corroborated locally on the same commit: the full 24-gate matrix, 24 pass / 0
fail, plus `cargo test --workspace` (lib 544, e2e 499, `compile_fail` 6),
`clippy --workspace --all-targets -D warnings`, and `fmt --check`. The `+stable`
lanes are confirmable only in CI, so the run above is the authoritative record
and the local run is not a substitute for it.

## The fifteen stages

| Stage | Commit | Subject |
|---|---|---|
| 1 | `9baf106` | `id_token_algs` configuration and the JOSE dependency |
| 2 | `5ed86e9` | compact-JWS structure and header hygiene |
| 3a | `2526313` | the input bound, `jwks_uri`, the bounded JWKS fetch |
| 3b | `911dfed` | JWKS key selection |
| 4a | `8a697b1` | signature verification; claims made unconstructible |
| 4b | `42cee33` | cache directives, age and freshness |
| 4c | `2a69ee6` | 200/304 revalidation and version binding |
| 4d | `3cab7df` | flights, cooldowns, authority on the read paths |
| 5 | `2a6c35d` | the hostile-provider corpus |
| 6a | `259e1fc` | the required identity claims (+ its returned fix) |
| 6b | `c773c5b` | the time claims |
| 6c | `9895766` | the bounded optional claims and the capability (+ fix) |
| 7 | `959f732` | the nonce rule |
| 8 | `9bfda9e` | the upstream discovery profile (+ fix) |
| 9 | `56c91de` | the remaining corpus rows |

**The plan was nine stages and the work was fifteen.** Stage 5's closure
assessment checked 096-A against `:21` rather than against the stage list and
found two whole areas of `:57-61` unimplemented — claim validation with the
nonce rule, and the discovery profile. Both omissions were the architect's stage
plan, not the RFC, which had required them throughout. No amendment was needed;
the missing stages were dispatched.

## The six evidence areas, and the corpus

`:21` scopes 096-A's prerequisite to *"discovery, transport, JOSE, claims,
state/nonce, and cache/rotation evidence, including the hostile-provider
corpus."*

| Area | Where | Layer |
|---|---|---|
| **Discovery** | stage 8: the eleven metadata rules of `:539-549`, the document bounds of `:530-533`, the canonical-URL rule of `:553`; `discovery::tests` 53 tests | 9 of 11 rows live; 2 cross-checks + tightened bounds not live — see limitations |
| **Transport** | RFC 134's validating resolver and egress policy, plus stage 3a's bounds; `r134_step1/2/3`, `r096_a_stage3a_jwks` over real TLS | Live |
| **JOSE** | stages 2, 3a, 3b, 4a: compact structure, header hygiene, key selection, algorithm constraint, signature verification | Validation layer |
| **Claims** | stages 6a, 6b, 6c: `iss`/`sub`/`aud`/`azp`, `exp`/`iat`/`nbf`, the eight bounded optional claims, the sealed capability of `:687-689` | Validation layer, dormant |
| **State / nonce** | stage 7: the nonce *rule* — constant-time digest comparison against a supplied digest. `:65-66` assigns the durable one-time-ness to 096-B1 | Validation layer, dormant |
| **Cache / rotation** | stages 4b, 4c, 4d: directives and freshness, 200/304 and version binding, single flight, cooldown, no stale acceptance | Validation layer |
| **Hostile-provider corpus** | stages 5 and 9: every negative row of `validation-matrix.md`'s Configuration, DNS/TLS/HTTP/JSON, Discovery, Claims, and Cache sections mapped to a named test, each row carrying the layer that proves it | Mixed, stated per row |

A live happy path is explicitly not a substitute for hostile-provider evidence
(`:1026`). It is not being offered as one: the corpus is negative-case driven
throughout, and the three live tests added in stage 9 are themselves two
positive derivations plus one negative.

## The nine attack categories

`:21` requires these *"each handled as designed"*. **"As designed" for 096-A is
defined by the RFC itself at `:68-73`:** 096-A *"does not, on its own, close the
shipped defect, and must not be described as doing so… Until then 096-A is a
reviewed library that no production request reaches."* So a category is handled
as designed when the validation exists and is proven — not when it is routed.
Routing is 096-B1's, by the RFC's own construction.

| Category | Assessment |
|---|---|
| Algorithm confusion | **Handled.** Stages 1, 2, 3b, 4a: configured allowlist, outer `alg` gate before key selection, key-family and curve binding, `none`/HS* refused at config time |
| Hostile endpoints | **Handled.** RFC 134 D2's validating resolver denies loopback and special-use ranges on the production client; stage 8 enforces approved origin and port, canonical URL, no credentials, no query, no fragment |
| Oversized responses | **Handled.** `response_bounds`: 64 KiB on bytes actually read, 200-only, JSON media type, member/array/string caps, proven over real TLS against a lying `Content-Length` |
| Rotating keys | **Handled.** Stages 4c, 4d: `CacheKey` binds provider, version and activation generation; a refreshed JWKS omitting a `kid` makes it fail rather than falling back; single flight, 30s cooldown, 60s forced window |
| Missing or mismatched nonce | **Handled.** Stage 7: required, string-typed, digest compared in constant time; duplicates caught by stage 6a's payload scan |
| Issuer errors | **Handled, at both levels.** Stage 6a byte-exact on the ID token's `iss` with no normalisation; stage 8 byte-exact on the discovery document's own `issuer`, which was previously never read at all |
| Audience errors | **Handled.** Stage 6a: `aud` as string or 1–8 unique strings containing the client ID, with the multi-audience `azp` rule |
| Token substitution | **Handled.** Defeated by binding all three of `aud`, `iss` and the nonce — complete as of stage 7 — plus stage 6b's `iat` lower bound, which refuses a token minted before this attempt began even when everything else is valid |
| Time errors | **Handled, with one divergence pending the owner's decision.** `exp`, `iat`, `nbf`, NumericDate integer discipline including the exponential form, the fixed symmetric 60-second skew, and both boundary sides tested. **The divergence:** `jsonwebtoken` accepts at `now <= exp + 60` while `:660` states the strict `now < exp + 60s`, so the RFC refuses one instant that the implementation accepts. `:661` and `:662` state `iat`'s and `nbf`'s bounds non-strictly, so `exp` is the only one of the three written strictly. Raised as a decision request, recommendation to amend `:660`; see `exp-boundary-strictness-2026-10-08.md`. **It does not block this assessment** — the divergence is one instant inside a 60-second skew the RFC deliberately grants — but the category is not *literally* satisfied until `:660` and the code agree |

## Limitations, stated rather than footnoted

**1. Stages 6a–7 are entirely dormant.** `identity_claims`, `time_claims`,
`nonce_claim`, `optional_claims` and `identity_capability` have **zero**
production callers; the only non-test references are type imports between
sibling 096-A modules and two `compile_fail` fixtures. Every validator —
`validate_identity_claims`, `validate_time_claims`, `validate_nonce`,
`validate_optional_claims`, `construct_identity_capability` — has zero
non-declaration references in `crates/sui-id/src`. **This is `:72-73` working as
written, not a defect**, and it is why the shipped defect stays open until
096-B1.

**2. Discovery is the one area partly live, and only partly.**
`ValidatedDiscovery::validate` has exactly one non-test caller,
`handlers/federation.rs:98` — a file 096-A may not modify (`:63-66`). So nine of
the eleven metadata rules took effect the moment stage 8 landed, while the two
configured-value cross-checks (`token_endpoint_auth_methods_supported`,
`id_token_signing_alg_values_supported`) and the **tightened document bounds**
in `validate_discovery_bytes` have zero non-test callers and are implemented but
inert. A stage authorised to edit `fetch_discovery` must wire them.
**Discovery's category therefore closes at the validation layer, not on the live
path.**

**3. 096-A does not close the shipped defect**, and this assessment does not
claim it does (`:68-69`).

## Records required at `:1022-1026`

Supplied for 096-A: **exact clean commit** `56c91de`, tree clean, `main` level
with `origin/main`; **commands and output** in each of the fifteen review
results under `.git-exclude/reviewed/`, with per-hunk SHA-256 verification of
every returned package against its named baseline; **fixture identity** via
those hashes and the `compile_fail` `.stderr` pins; **clock** — the fixed
symmetric 60-second skew, `jsonwebtoken`'s `leeway = 60`, and a boundary test
that resamples rather than widening its margin; **cache constants** — 30s
failure cooldown, 60s forced-refresh window measured from dispatch, 32-key and
16-depth JWKS bounds, the `ResponseCaps` pair.

Belonging to later phases, and **not** offered here: migration report and
quarantine, sanitized telemetry samples, the IANA registry snapshot, and the
representative public upstream integration with owner-supplied credentials
outside CI — `:21` places that last one in **096-B1's** prerequisite, not
096-A's. The evidence list at `:999-1021` spans the whole RFC; only the `:21`
sentence governs this stage.

## Not 096-A's, re-confirmed

Preflight and provider activation (096-B2); callback and attempt state, mapping,
MFA and durable effects (096-B1, and RFCs 094/102/103/105/112/115); the
token-response envelope (096-B1's production wiring). Stage 5 had also assigned
the Claims rows and discovery document validation elsewhere; both were wrong and
both are corrected above.

## Recommendation

**096-A's evidence is complete and I recommend `@nabbisen` accept it**, with the
three limitations above recorded as part of what is being accepted rather than
as caveats to it, and with the `exp` boundary question settled either way at his
convenience. The alternative — settling `:660` first and then accepting — is
equally sound and costs only the ordering.

This document recommends acceptance. It does not grant it. `:21` reserves that
to `@nabbisen`.
