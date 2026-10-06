# RFC 096-A — the JOSE strategy, before any dispatch

**Date:** 2026-10-06. **A decision request, not a dispatch.**
**Why now:** all four of 096-A's prerequisites are clear and I went to stage it.
**This is the blocker I found instead.**

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
`josekit` and `jsonwebtoken` are all absent.** The only signature crate is
`ed25519-dalek`.

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

## The options

**A — extend `oidc/jwt.rs` to the four algorithms.** Cheapest in new code.
**I recommend against it.** That module verifies **our own** issued tokens, and
its EdDSA-only restriction is a security property, not an accident. Widening it
so the upstream path can use it also widens what our own token verification
will accept, which is how algorithm-confusion bugs are born.

**B — a separate upstream verifier in `id_token.rs`, on vetted primitive
crates** (`rsa`, `p256`), with the JOSE glue written here. **Our issuer path
stays EdDSA-only, untouched.** The algorithm policy the matrix demands —
reject `none`, reject `HS*`, reject unknown, require a non-empty allowed subset
— is written explicitly where it can be read and tested, rather than configured
into someone else's abstraction. Cost: more of our own code in a
security-critical path, and two new production dependencies.

**C — a JOSE library for the upstream path only** (`josekit` or similar). Least
of our own crypto glue; the parsing and algorithm binding are someone else's
reviewed work. Cost: a larger dependency and its transitive surface, and the
matrix's policy expressed as configuration rather than as code we own.

## My recommendation, and the honest uncertainty in it

**B.** The dangerous part of JOSE is not the arithmetic, which `rsa` and `p256`
do well; it is the **algorithm binding** — `alg: none`, an `HS256` token
verified against a public key as an HMAC secret, a `kid` that selects the wrong
key. Those are exactly the rules RFC 096's matrix enumerates, and I would rather
they were twenty lines we can read than a configuration flag.

**Where I am genuinely unsure:** B puts us in the business of JOSE parsing,
which is a category with a long history of subtle bugs, and C's library has been
looked at by more people than will ever look at ours. If you weigh
supply-chain surface lower than I am weighing it, C is defensible and I would
not argue hard.

**Not recommended: A**, for the reason above.

## What I need

A direction. With it I can stage 096-A; without it the first stage would hand
the implementer the decision, which is the one thing a handoff must not do.
