# Developer Handoff — RFC 096-A stage 4a: signature verification

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `project-instructions-general-common.md:44` authorizes **both**
roles to commit and push. This dispatch asks only that you hand the tree over
uncommitted.

**Clone under `.git-exclude/tmp/clones/`, on `/home`. Compute the package's
hashes with the old method** — `scripts/hunk-hashes.py` is dispatched
separately and may not exist yet when you start this.

## RFC and position

**RFC 096 — Accepted.** Stage 3b landed in `911dfed`. **Stage 4 is split.**

**Why:** RFC 096's *"Cache and rotation algorithm"* (`:823-924`) is **102 lines
of normative requirements** — ETag-only revalidation, closed directives, no
stale use, custom 304 metadata defaults, cache keys carrying provider ID and
config version, per-cache freshness defaults and maxima, invalidation on
configuration mutation with post-commit eviction, and bounded provider-wide
unknown-`kid` refresh. That is not a tail on another stage.

- **4a — this one:** signature verification, the crypto error variants, and
  the two refusals earlier stages deferred. **No cache.**
- **4b:** the cache and rotation profile.

**Seven stages now.** The same judgement that split 3 into 3a/3b.

## The constraint, unchanged and still the thing most at risk

**096-A must not change what production does.** `handlers/federation.rs:404`
still calls `decode_id_token_claims`, which verifies nothing.

> **Do not change `decode_id_token_claims`. Do not touch
> `handlers/federation.rs`.** Everything here is new code beside it, reached
> only by tests. **096-B1** routes live traffic through it, and that needs RFC
> 094 M2a.

Four stages have held this line. Stage 4a is where it is most tempting to
break, because the verifier finally exists and the unsafe caller is two lines
away.

## Scope

### 1. Verification, from the pieces that already exist

`parse_compact_jws` (stage 2) gives the validated header and the raw segments.
`select_key` (stage 3b) gives exactly one `DecodingKey`. **Stage 4a joins
them** and performs the signature check with `jsonwebtoken`.

**The `Validation` must be built in one function, with a comment saying why.**
`decoding.rs:335-348` guards the empty-algorithm error, the key-family binding
**and** the `alg` allowlist behind a single `validation.validate_signature`, so
a reader who thinks they are switching off one check switches off four. That
flag's only public mutator is `#[deprecated]`, so `-D warnings` already stops
it — do not add an `#[allow]`.

### 2. The two deferred refusals — both are yours now

| Deferred from | Rule |
|---|---|
| **stage 3a** | a provider whose discovery document has **no `jwks_uri`** is refused **here**. 3a made the field `Option` because a required field would have broken live deserialization; this is the stage that needs it, so this is where its absence becomes an error |
| **stage 3b** | the header's `alg` must be in the provider's configured **`id_token_algs`**. Stage 1 validated the set at config load; 3b deliberately matched a key to whatever `alg` was. **4a enforces membership** — and it is the outer gate, so it runs **before** key selection, not after |

### 3. The error taxonomy's crypto variants

Extend the existing enums rather than starting a third. One variant per
failure, as in stages 2, 3a and 3b: `alg` not permitted by configuration,
`jwks_uri` absent, signature invalid, and key material rejected by the library.

**Map `jsonwebtoken::errors::ErrorKind` into our own taxonomy in one function.**
Its wording is written for its callers and changes between versions. **The
operator log gets the specific reason; the end user gets a generic failure** —
RFC 096's error contract (`:924`) and the operator half of the project's
standard. A detailed message to the browser is an oracle.

### 4. Signature verification completes before claims are exposed

RFC 096 `:648`: *"Signature verification completes before payload claims are
exposed outside the validator. On failure, no partially decoded claim object
reaches mapping, logs, metrics, or audit."*

**Make this structural, not a convention.** The claims type should not be
constructible from an unverified token — a private constructor, or claims
returned only by the verifying function. A test that asserts "we remembered to
verify first" is weaker than a signature that cannot be called out of order.

### Not in this stage

The cache and rotation (4b). The hostile-provider corpus (5). `oidc/jwt.rs`
stays EdDSA-only and untouched for the whole of 096-A.

## Tests

Each refusal above, plus: a valid RS256, PS256, ES256 and EdDSA token each
verifying end to end against a matching JWKS; a token whose signature is
altered by one byte; a token signed by a key that is in the JWKS but whose
`kid` is not the header's; `alg: "HS256"` with a configured set that excludes
it, refused as **not permitted** rather than as a family mismatch; and a
provider with no `jwks_uri`.

**State in the package how §4 is enforced** — which type or visibility makes an
unverified claim unconstructible — because a test alone does not establish it.

## Gates

`G01`–`G08`, `G17`, `G18`, `G19`, `G21`. Clean tree, clone on `/home`.

## Package

Per-hunk SHA-256 against the tip when you start, full-content hashes for new
files, gate results, entry point. **And say plainly whether anything you wrote
is reachable from `handlers/federation.rs`** — the answer must be no, and it is
the one claim in this stage I will check first.
