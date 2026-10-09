# RFC 096-A stage 8 — the upstream discovery profile

**Dispatched.** 2026-10-09 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** Read the tip with `git log -1` when you start, hash against it,
name the full SHA.
**Prior stages.** 1–5, 6a (+fix), 6b, 6c (+fix), 7 — all landed. **Eleven of
fifteen.**
**Remaining after this one.** 9, the corpus rows stage 5 could not cover.
**Open with the owner, untouched here.** `exp` boundary strictness.

## Why this stage exists

Stage 5's closure assessment checked 096-A against RFC 096 `:21` rather than
against my stage plan and found this missing: `:57` gives 096-A *"Discovery and
egress/SSRF policy; HTTPS enforcement and exact issuer binding"*, and the
closure prerequisite lists *"discovery … evidence"*. **The endpoint half is
already done** — `discovery.rs` validates that the configured issuer is
canonical HTTPS before any fetch (RFC 134 D3) and that every discovered endpoint
matches an approved origin and port, proven over TLS by
`r134_step2_endpoint_origins.rs`. What is missing is the metadata table and the
document bounds.

**This is the last of RFC 096 `:21`'s nine attack categories still open.** The
other eight are closed as of stage 7.

## Scope — the twelve-member table

RFC 096 `:537-549`, verbatim. `RawDiscovery` currently deserializes **four** of
these (`authorization_endpoint`, `token_endpoint`, `userinfo_endpoint`,
`jwks_uri`):

| Member | Rule |
|---|---|
| `issuer` | Required and byte-exact with configured canonical issuer |
| `authorization_endpoint` | Required canonical HTTPS URL; approved origin/port |
| `token_endpoint` | Required canonical HTTPS URL; approved origin/port |
| `jwks_uri` | Required canonical HTTPS URL; approved origin/port |
| `response_types_supported` | Contains exactly usable `code`; advertised extras ignored within bounds |
| `grant_types_supported` | If present, contains `authorization_code` |
| `code_challenge_methods_supported` | Required and contains `S256` |
| `authorization_response_iss_parameter_supported` | Required and exactly `true` |
| `token_endpoint_auth_methods_supported` | Contains the configured method; `none` required for public clients |
| `id_token_signing_alg_values_supported` | Intersects configured algorithms; runtime uses the intersection only |
| `subject_types_supported` | Contains `public`; pairwise-only providers are outside M4 |

Two of these are not bookkeeping and deserve their own attention:

- **`authorization_response_iss_parameter_supported` must be *exactly* `true`.**
  Not truthy, not `"true"` — the same discipline as 6c's `email_verified`. This
  is the defence against an authorization-response mix-up across providers.
- **`id_token_signing_alg_values_supported` must intersect the configured
  algorithms, and *"runtime uses the intersection only"*.** That sentence ties
  back to stage 1's `id_token_algs` and stage 4a's allowlist: after this stage,
  the effective set is the intersection, not the configuration alone. **Say in
  your package how you expose that intersection** without changing stage 4a's
  behaviour for callers that do not pass one — this is the one member with a
  consequence outside discovery, and I would rather see your design than name
  one.

**`jwks_uri` stays `Option`** and that is deliberate, not an oversight.
`discovery.rs:20-24` documents why, and stage 4a refuses with `NoJwksUri` before
any network access. The RFC fails the whole document; the code fails at the
point of use, same outcome on the login path. Recorded in stage 5's assessment
so it is not re-litigated — **do not change it**, and do not add it to
`required_spec_claims`-style required handling.

## The document bounds — measured, and a real conflict to resolve

RFC 096 `:530-533`, verbatim: *"The decoded body is at most 64 KiB, depth 16,
128 object members per object, 32 array members, and 2,048 bytes per string;
duplicate keys at any depth are rejected."*

`crates/sui-id/src/http/response_bounds.rs` already exists and is what `jwks.rs`
uses. Its constants, read directly:

| Bound | RFC discovery | `response_bounds` | |
|---|---|---|---|
| body bytes | 64 KiB | `MAX_RESPONSE_BYTES = 64 * 1024` | **matches** |
| object members | 128 | `MAX_MEMBERS = 128` | **matches** |
| array members | 32 | `MAX_ARRAY_LEN = 128` | **4x too loose** |
| bytes per string | 2,048 | `MAX_STRING_LEN = 8 * 1024` | **4x too loose** |
| depth | 16 | `jwks::MAX_JWKS_DEPTH = 16` | matches, but lives in `jwks` |

**So `check_caps` cannot be reused as-is**, and its signature is
`pub fn check_caps(value: &serde_json::Value) -> Result<(), BoundsError>` — no
parameters; it hardcodes those constants. **Do not change the constants**:
they are JWKS's and the shared transport's, and tightening them would silently
re-scope stage 3a.

Resolve it and tell me which you chose and why. The obvious options are to
parameterise `check_caps` with a caps struct and have both callers pass their
own, or to leave it alone and give discovery its own checker. I lean to
parameterising, because two near-identical walkers is the duplication this RFC
keeps rejecting — but `check_caps` is on stage 3a's proven path and I would
rather you weigh the risk of touching it than take my lean as settled.

**Depth and duplicate-key detection already exist, privately, in `jwks.rs`:**
`nesting_depth(&Value) -> usize` at `:159` and
`first_repeated_member(&[u8]) -> Result<Option<String>, JwksError>` at `:172`.
Both are private to that module. **Do not write a third copy of either** —
promote, share, or explain why neither fits. Note `first_repeated_member` reads
the raw bytes because a parsed `Value` has already lost repeats, which is the
same reason stage 6a's payload scan reads bytes.

Discovery's rule is *"duplicate keys at **any depth**"*. `jwks.rs:16-17` says
its own scan covers *"anywhere in the document"* — check whether that is
genuinely recursive before relying on it, and say what you found.

## Also in this section

`:528-530`: only a **200** response with `application/json` or an equivalent
`+json` media type is parsed, and the RFC 8414-style
`/.well-known/openid-configuration/tenant/a` form is **rejected** for a path
issuer — discovery appends the suffix *after* the issuer path. `:551-554`:
*"Discovered endpoints do not expand trust"*; a discovery redirect, origin
substitution, scheme downgrade, credential-bearing URL or noncanonical URL
**fails the whole document**; unknown bounded metadata is ignored.

Check which of those the existing `discovery.rs` and the RFC 134 transport
already enforce, and state the answer per clause rather than reimplementing. I
expect several are already covered; I have not measured each one, and I would
rather you tell me than have me guess in this dispatch.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The row-to-test table** for all twelve members, with both the accepting and
   rejecting side of every bound — 6c's fix set that standard and stage 7 held
   it.
2. **The bounds decision** above, with your reasoning.
3. **The `id_token_signing_alg_values_supported` intersection design.**
4. **A clause-by-clause statement** of what `:528-530` and `:551-554` already
   enforce versus what this stage adds.
5. **Mutation evidence**, the mutation that isolates each rule. If one survives,
   report it as stage 7 did.
6. **Per-hunk SHA-256** against the tip you named.
7. **Gate evidence** from a throwaway clone with its own `target/`.
8. **Anything you think is wrong with this dispatch.** Five stages running you
   have corrected something of mine. The bounds table and the two function
   locations above are measured as of today; the clause coverage in the last
   section is explicitly *not*.

**When this lands, all nine of `:21`'s attack categories are closed** and only
stage 9's corpus collection stands between 096-A and its closure review. Do not
claim the closure; it is mine to write.
