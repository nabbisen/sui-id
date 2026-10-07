# Developer Handoff — RFC 096-A stage 3a: the input bound, `jwks_uri`, and the JWKS fetch

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `.git-exclude/rules/project-instructions-general-common.md:44`
authorizes **both** roles to commit and push. This dispatch asks only that you
**hand the tree over uncommitted**, so it can be verified before it lands.

**Clone under `.git-exclude/tmp/clones/`, on `/home`** — not `/tmp`.

## Stage 3 is split, and here is why

My five-stage map had stage 3 as *"`jwks_uri`, the JWKS fetch, JWKS bounds and
the key-selection rules"*. **That is too much for one package.** Stage 2 ran to
35 tests for structural rules alone, and the key-selection rules in RFC 096
`:641-646` — `kid`, algorithm family, curve and size, optional `alg`, `use`,
`key_ops`, RSA ≥ 2048 with a valid exponent, P-256 for ES256, Ed25519 OKP for
EdDSA, private-key members, multi-use keys, exactly one survivor — are a
comparable body of work on their own.

**So: 3a is transport and bounds, 3b is key selection.** Six stages, not five.

## Scope

### 0. First: bound `parse_compact_jws`'s own input

**This is your stage-2 decision 6, answered.** You wrote that the signature
segment is decoded with no size limit and asked whether the caller's bound
reaches the function.

**It does, today, and only today.** `handlers/federation.rs:392` reads the token
response through `response_bounds::read_bounded_json`, and `MAX_STRING_LEN` is
**8 KiB**, so the `id_token` string is bounded before it ever reaches you.

**Two things follow, and the second is the surprising one:**

1. The bound is **external**. `parse_compact_jws` is safe because of its one
   caller. Stage 4 adds callers. A function whose memory safety depends on who
   calls it is the wrong shape for the "reviewed library" the RFC describes.
2. **RFC 096's 16 KiB decoded limit is unreachable on this path.** 8 KiB
   encoded decodes to about 6 KiB, so the transport bound is tighter than the
   RFC's and the RFC's number never fires in production.

**Do:** bound the **total encoded input** at the top of `parse_compact_jws`,
before anything else, sized to keep the RFC's rule reachable — 16 KiB decoded
is ≈ 21.8 KiB encoded, plus a signature allowance; an RSA-4096 signature is 512
bytes, ≈ 683 base64 characters. Pick the constant, name it, and say in a
comment why it is larger than `MAX_STRING_LEN`. A new error variant, or
`Oversized` with the position documented — your call, consistent with the
taxonomy you already built.

### 1. `jwks_uri` in discovery — and the trap in it

`crates/sui-id/src/http/discovery.rs:20-23` says `jwks_uri` is *"deliberately
out of scope"* and that *"when JWKS verification arrives it inherits this same
validation by construction"*. That is now.

**The trap: `jwks_uri` is REQUIRED by OIDC Discovery, and you must still make
it optional here.** `RawDiscovery` is deserialized on the **live** path
(`federation.rs:95`). A required field makes deserialization fail for any
provider whose document omits it — **which would change production behaviour,
and 096-A must not** (`:68-73`).

**Do:** `#[serde(default)] pub jwks_uri: Option<String>`, validated by the same
`ValidatedDiscovery::validate` rules as every other endpoint — absolute URL,
`https`, origin in the provider's allowed set — **when present**. Absent is
accepted here. **Stage 4 refuses a provider with no `jwks_uri`**, at the point
where verification actually needs it; that is where the refusal belongs,
because that is where it changes nothing that works today.

### 2. The JWKS fetch

On the RFC 134 federation client — `runtime::egress::build_federation_client`
— so it inherits the validating resolver, the origin checks and the response
bounds. **No new HTTP client, no new transport settings.** G19 exists to
enforce exactly that and will be run.

### 3. JWKS bounds — what you already have, and what is missing

`response_bounds` gives you three of the RFC's five limits for free. **I checked
rather than assumed; do not re-implement these:**

| RFC 096 JWKS rule | Status |
|---|---|
| at most **64 KiB** | **have it** — `MAX_RESPONSE_BYTES` |
| at most **128 members** | **have it** — `MAX_MEMBERS`, applied recursively by `check_caps` |
| 200 JSON only | **have it** — `read_bounded_json` is 200-only |
| depth at most **16** | **missing.** `serde_json`'s own limit is **128**, and `response_bounds`'s module doc says depth is "serde_json's own job". 128 ≠ 16 |
| at most **32 keys** | **missing.** `MAX_ARRAY_LEN` is **128** |
| duplicate JSON members rejected | **partly.** Duplicate *declared* keys fail; **duplicate *unknown* keys do not** — `response_bounds`'s own module doc says so at `:70-75`, and you proved it again in stage 2 |
| duplicate nonempty `kid` rejected | **missing** — JWKS-specific |

**Reuse your stage-2 `MemberNames` visitor for the duplicate-member rule.** It
is the same problem and you have already solved it; a second implementation
would be a second thing to keep right.

**Do not relax `response_bounds` to fit JWKS**, and do not widen
`MAX_ARRAY_LEN` to 32's detriment — the JWKS-specific limits belong beside the
JWKS parsing, not in the shared transport bounds that discovery also uses.

### Not in this stage

Key selection (3b). Signature verification, the error taxonomy's crypto
variants, and the cache with rotation (4). The hostile-provider corpus (5).
**No change to `decode_id_token_claims` or `handlers/federation.rs`** — the
stage-2 constraint still holds, and everything here stays unreachable from
production.

## Tests

Every bound as a rejection, at the boundary and one past it: depth 16 and 17,
32 keys and 33, 64 KiB, a duplicate declared member, **a duplicate unknown
member**, two keys with the same nonempty `kid`, two keys with an empty `kid`
(not a duplicate — the RFC says *nonempty*). Plus `jwks_uri` present and valid,
present and not https, present with an origin outside the allowed set, and
**absent, which must still deserialize** — that last one is the production
guard, so make it explicit.

## Gates

`G01`–`G08`, `G17`, `G18`, `G19`, `G21`. **G19 matters here** — this stage adds
an egress call and G19 is the gate that says federation egress is one client.

## Package

Per-hunk SHA-256 against the tip when you start, full-content hashes for new
files, gate results, entry point. **State the constant you chose for the
`parse_compact_jws` input bound and why**, since that is the one number in this
stage I left to you.
