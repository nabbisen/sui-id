# Developer Handoff — RFC 096-A stage 3b: JWKS key selection

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `project-instructions-general-common.md:44` authorizes **both**
roles to commit and push. This dispatch asks only that you hand the tree over
uncommitted so it can be verified before it lands.

**Clone under `.git-exclude/tmp/clones/`, on `/home`.**

## RFC and position

**RFC 096 — Accepted.** Stage 3a landed in `2526313`. **This is 3b of six**,
and it is the last stage before signature verification.

**The stage-2 constraint still holds and is the thing most at risk here:** no
change to `decode_id_token_claims`, no change to `handlers/federation.rs`,
nothing reachable from production. Stage 4 wires verification; 096-B1 routes
traffic.

## Scope — RFC 096 `:641-646`, the selection rules

> *"The selected key must match `kid`, algorithm family, curve/size, optional
> `alg`, `use=sig` when `use` is present, and contain `verify` when `key_ops`
> is present. Accepted keys are RSA at least 2,048 bits with a valid exponent,
> P-256 EC for ES256, and Ed25519 OKP for EdDSA. Private key members and
> incompatible/multi-use keys are rejected. One and only one compatible key
> must result."*

**Input:** the `Jwks` from stage 3a, the header's `kid`, and the provider's
configured `id_token_algs` from stage 1. **Output:** one key, or a refusal
naming the rule.

## What the library does and does not do — measured, so you do not have to

`jsonwebtoken`'s `jwk` module gives you the types. **It does not give you the
policy.** I checked the vendored 10.3.0 source:

| Need | Library |
|---|---|
| Parse a JWK into typed parameters | **yes** — `jwk::Jwk`, `AlgorithmParameters::{RSA, EllipticCurve, OctetKeyPair}`, `CommonParameters { public_key_use, key_operations, key_algorithm, key_id }` |
| `use` and `key_ops` as enums | **yes** — `PublicKeyUse::{Signature, Encryption}`, `KeyOperations::{Sign, Verify, …}` |
| Curve as an enum | **yes** — `EllipticCurve::{P256, P384, P521, Ed25519}` |
| Convert the chosen key to verification material | **yes** — `DecodingKey::from_jwk`, not feature-gated |
| **Reject a private key** | **NO.** The JWK structs carry no `d`/`p`/`q` fields **and do not use `deny_unknown_fields`**, so a private JWK **deserializes silently as its public half.** |
| **Enforce RSA ≥ 2048 or a sane exponent** | **NO** — `RSAKeyParameters` is `{ key_type, n, e }` as base64url strings, unchecked |
| **Enforce "exactly one"** | **NO** — selection is entirely ours |

**So the split is: the library converts, we decide.** Use `DecodingKey::from_jwk`
on the single survivor — do not hand-build key material — but every rule below
is yours.

## The rules, each its own refusal

**One error variant per rule**, as in stages 2 and 3a, so an operator can tell
which one fired.

| Rule | Reject when |
|---|---|
| `kid` match | no key in the set has the header's `kid` |
| **private key members** | the raw JWK object contains any of `d`, `p`, `q`, `dp`, `dq`, `qi`, `oth`. **You must check the raw JSON**, because the typed struct silently drops them — see the table above. This is the one rule the library's shape actively hides |
| `use` | `use` is present and is not `sig` |
| `key_ops` | `key_ops` is present and does not contain `verify` |
| multi-use | `use` and `key_ops` are both present and disagree, or `key_ops` contains both `sign` and `verify` |
| `alg` on the key | the key's `alg` is present and is not the header's `alg` |
| algorithm family | the header's `alg` family does not match `kty` — `RS*`/`PS*` need RSA, `ES256` needs EC, `EdDSA` needs OKP |
| curve | `ES256` with a curve other than `P-256`; `EdDSA` with a curve other than `Ed25519` |
| RSA size | the decoded `n` is shorter than **256 bytes** (2,048 bits) |
| RSA exponent | `e` is absent, or decodes to an even value, or to a value less than 3. **State the rule you implement in a comment** — "valid exponent" is the RFC's phrase and it is not self-defining |
| **exactly one** | zero candidates survive, **or more than one does**. Two survivors is a refusal, not a pick-the-first |

**`id_token_algs` is the outer gate.** The header's `alg` must already be in
the provider's configured set — stage 1 validated the set, stage 4 enforces
the membership. Do not re-implement that here; this stage matches a key to the
header's `alg`, whatever it is.

## Tests

Every row above as a refusal, plus: the happy path for each of RS256, PS256,
ES256 and EdDSA; a set where two keys share the header's `kid`, which stage 3a
already refuses as a duplicate — assert it still cannot reach here; a key with
`use: "enc"`; a key with `key_ops: ["sign"]`; **an RSA private JWK, which must
be refused and not silently accepted as its public half**; RSA `n` at exactly
256 bytes (accepted) and 255 (refused); `e` of 65537 (accepted), 2 (refused),
1 (refused).

The RSA-private-key test is the important one. **Write it so that it fails if
someone later replaces the raw-JSON check with the typed struct.**

## Gates

`G01`–`G08`, `G17`, `G18`, `G19`, `G21`. Clean tree, clone on `/home`.

## Package

Per-hunk SHA-256 against the tip when you start, full-content hashes for new
files, gate results, entry point. **State the exponent rule you implemented**,
and **say explicitly how the private-key check reads the JWK** — raw JSON or
typed — because that is the rule whose correct implementation is the
non-obvious one.
