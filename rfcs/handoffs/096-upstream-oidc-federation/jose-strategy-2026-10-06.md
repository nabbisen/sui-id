# RFC 096-A — the JOSE strategy, before any dispatch

**Date:** 2026-10-06. **A decision request, not a dispatch.**
**Revised 2026-10-06, same day, after you asked whether I remembered the
project's fundamental philosophy. You were right. The first version of this
document recommended B. It now recommends C, and §"The correction" says why the
first answer was wrong — including that I had already reasoned the opposite way,
correctly, two days earlier.**

**Why this exists:** all four of 096-A's prerequisites are clear and I went to
stage it. **This is the blocker I found instead.**

## The prerequisites are genuinely met

| | |
|---|---|
| M1a complete | ROADMAP records it CLOSED 2026-07-30 |
| RFC 096 Accepted in its amended form | yes, `:496` amended 2026-10-06 |
| Hostile-provider harness approved **and built** | `r096_a_harness.rs`, `build_federation_client_for_tests` |
| Preparatory `federation.rs` split committed and reviewed | `2f07862`; `id_token.rs` is the landing spot, 67 lines |

## What stops a dispatch

**RFC 096's matrix requires four signature algorithms. This project can verify
one.**

`validation-matrix.md:17` — *"ID-token algs | nonempty subset
**RS256/PS256/ES256/EdDSA** | none, HS\*, unknown, empty"*.

Measured in `Cargo.toml`: **`rsa`, `p256`, `ecdsa`, `elliptic-curve`,
`josekit` and `jsonwebtoken` are all absent.** The only signature crate declared
at the workspace is `ed25519-dalek`.

**And the existing verifier cannot be reused as it is.**
`crates/sui-id-core/src/oidc/jwt.rs` does have `verify<C>(token, resolver)` —
but its own first line reads *"Minimal RFC 7519 JWT support, **restricted to
the EdDSA (Ed25519) algorithm**"*, and `:91` rejects anything else. **Real
upstream providers sign with RS256**: Google, Microsoft Entra, Okta and Auth0
all do. Pointed at them, that verifier rejects every token.

So 096-A cannot verify an upstream signature without either new cryptographic
dependencies or a JOSE library. **That is a technology choice with security
consequences, and my operating instructions put those with you** — and forbid me
delegating an unresolved architecture decision to the implementer, which
dispatching around it would do.

## The correction

The first version of this document recommended **B** — hand-written JOSE glue
over the `rsa` and `p256` primitive crates — and then admitted, in the same
paragraph, that *"C's library has been looked at by more people than will ever
look at ours"* and that I *"would not argue hard"*. That is not a
recommendation. It is a choice handed back up with a preference attached, and it
is the thing a decision request is supposed to remove.

**It also contradicted my own reasoning from two days earlier.** RFC 134 D5,
now in `done/`, rejected hand-writing an HTTP/1.1 parser in these words:

> Hand-written HTTP parsers are a classic source of request-smuggling and
> memory-safety defects. Trading a widely reviewed implementation for an
> unreviewed one makes the system less safe while making the matrix look
> satisfied.

Substitute "JOSE" for "HTTP" and the sentence is still true, and still about
this decision. *"I would rather they were twenty lines we can read"* is the
attractive answer, not the safe one. **Clean, safe and secure, robust and
sophisticated** does not mean our own code everywhere; on a cryptographic
verification path it means the opposite. I will say the general form of the
error plainly, because it is the useful part: **I let "code we own is code we
can audit" outrank "code the world has already audited" on a path where the
second is worth far more.**

## What I measured after the correction

Four facts, all measured today, that narrow the choice much further than the
first version of this document did.

**1. `rsa` 0.9.10 carries an unpatched advisory.** `RUSTSEC-2023-0071` (Marvin
attack, `CVE-2023-49092`), `patched = []`, and the advisory text in the local
database reads *"Still affected as of 2026-09-12: rsa 0.9.10 (latest stable)
and rsa 0.10.0-rc.18 (latest). `patched = []` is intentional."* The leak is on
the **private-key** path, so ID-token *verification* is very likely out of
scope — but we would be adding an advisory-bearing crate to a public IdP and
then suppressing the finding in our own security-audit workflow. **That is
option B's foundation, and it disqualifies B on its own.**

**2. We already ship two reviewed implementations of all four algorithms.**
`ring` 0.17.14 and `aws-lc-rs` 1.18.1 are both already in `Cargo.lock` and
already compiled into the binary — `ring` via `ldap3`'s `tls-rustls-ring`,
`aws-lc-rs` as reqwest/rustls's default provider, both of which
`Cargo.toml:138-145` already documents. `ring/src/signature.rs:290-292,273,263`
publicly exports `RSA_PKCS1_2048_8192_SHA256`, `RSA_PSS_2048_8192_SHA256`,
`ECDSA_P256_SHA256_FIXED` and `ED25519` — **exactly the matrix's four rows.**
Adding `rsa` and `p256` would therefore add a *third* implementation of
primitives we already have, which is the reverse of clean.

**3. `josekit` is out on deployment grounds, not preference.** Measured at
docs.rs: josekit 0.10.3 (released 2025-05-20) has `openssl ^0.10.68` as a normal
dependency and states *"This library depends on OpenSSL 1.1.1 or above DLL."*
For a self-hosted product that ships a binary, that is a **third native crypto
stack plus an external system library**. It fails the same test that eliminated
option B.

**4. `jsonwebtoken` 10.3.0 can be taken with no new cryptographic
implementation at all, and it enforces the matrix in its types.** With
`default-features = false, features = ["aws_lc_rs"]` it binds the provider we
already ship and pulls no `rsa`, no `p256`, no `pem`, no `simple_asn1`.
Measured in the vendored source:

| Matrix requirement | Where the crate enforces it |
|---|---|
| reject `alg: none` | `algorithms.rs:45-71` — the `Algorithm` enum **has no `None` variant**, so `none` cannot deserialize |
| reject `HS*` on an asymmetric key | `decoding.rs:341-342` — the key's `AlgorithmFamily` must match **every** allowed alg's family |
| reject unknown `alg` | `decoding.rs:347` — `header.alg` must be in `validation.algorithms` |
| require a **non-empty** allowed subset | `decoding.rs:335` — an empty `validation.algorithms` is an error |
| all four algs | `crypto/aws_lc/{rsa,ecdsa,eddsa}.rs` — RS256, PS256, ES256, EdDSA, on `aws-lc-rs` |
| JWKS by `kid` | `jwk.rs:552-561` `JwkSet::find(kid)`; `decoding.rs:210` `DecodingKey::from_jwk`, **not** feature-gated |

Those are the same four rules I argued in the first draft I would rather own as
twenty readable lines. They are already there, as types rather than as
configuration, which is stronger than what I proposed to write.

## The options, as they now stand

**A — extend `oidc/jwt.rs` to the four algorithms. Not recommended, unchanged.**
That module verifies **our own** issued tokens, and its EdDSA-only restriction
is a security property, not an accident. Widening it so the upstream path can
use it also widens what our own token verification will accept, which is how
algorithm-confusion bugs are born.

**B — hand-written JOSE glue over `rsa` and `p256`. Withdrawn.** Finding 1
disqualifies the foundation; finding 2 makes the dependencies redundant; the
RFC 134 D5 reasoning rejects the hand-written part.

**C — a reviewed JOSE library on the upstream path only. Recommended**, in the
specific form below.

## Recommendation

**C, as `jsonwebtoken = { version = "10.3", default-features = false,
features = ["aws_lc_rs"] }`, used only in `crates/sui-id/src/http/id_token.rs`.**

Four properties, each measured above, not asserted:

1. **No new cryptographic implementation enters the binary.** It binds
   `aws-lc-rs`, which reqwest/rustls already compiles in.
2. **No advisory-bearing crate enters the tree**, and no external system
   library.
3. **`oidc/jwt.rs` is not touched and stays EdDSA-only.** Our issuer path and
   the upstream-consumer path keep separate verifiers with separate policies,
   which was the one thing option A got wrong and which B and C both get right.
4. **The dangerous rules are enforced by types we did not write and cannot
   accidentally widen** — `alg: none` is unrepresentable rather than rejected.

**What we still own, and still have to test.** Choosing a library does not move
RFC 096's matrix onto someone else. The hostile-provider harness still has to
drive every negative row — `none`, `HS256`-against-the-public-key, unknown
`alg`, empty allowed set, wrong `kid`, rotated key, `iss`/`aud`/`nonce`
mismatch, expiry skew — against **our** call site and prove *our* configuration
of the library rejects them. A library that is correct and misconfigured fails
exactly as badly as our own code would, so the assertions in the harness are
what actually discharges the matrix.

**Two honest negatives, recorded rather than buried.** `jsonwebtoken` declares
`maintenance status = "passively-maintained"` in its manifest; and
`RawDiscovery` (`discovery.rs:20`) deliberately omits `jwks_uri`, so 096-A must
also add that field and the JWKS fetch — on the RFC 134 federation client, under
the validating resolver, with the response bounds already in place. Neither
changes the recommendation; both belong in the first stage's scope.

## What I need

**A direction on C in the form above.** With it I can stage 096-A immediately.
Dependency changes are yours, not mine, which is why this is still a decision
request and not a dispatch.
