# RFC 096-A — the JOSE strategy, before any dispatch

**Date:** 2026-10-06. **A decision request, not a dispatch.**
**Revised 2026-10-06, same day, against both halves of the project's fundamental
philosophy — *"finally clean, safe and secure, and robust and sophisticated
design"* **and** *"APIs and UI/UX for users not to be confused or misunderstand
are also very important"*. The first version of this document recommended B and
said nothing at all about the second half. It now recommends C; §"The correction"
says why the first answer was wrong, and §"The operator-facing half" supplies what
was missing.**

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
already compiled into the binary. **Route corrected 2026-10-06** after the dev
team's verification: both arrive via `rustls → hyper-rustls → reqwest` and the
workspace's own direct `rustls` dependency. An earlier draft of this document
said `ring` came via `ldap3`'s `tls-rustls-ring`; `ldap` is **not** a default
feature (`crates/sui-id/Cargo.toml:26`), so that route does not exist in a
default build. The fact was right and the attribution was invented. `ring/src/signature.rs:290-292,273,263`
publicly exports `RSA_PKCS1_2048_8192_SHA256`, `RSA_PSS_2048_8192_SHA256`,
`ECDSA_P256_SHA256_FIXED` and `ED25519` — **exactly the matrix's four rows.**
Adding `rsa` and `p256` would therefore add a *third* implementation of
primitives we already have, which is the reverse of clean.

**3. `josekit` is out on how its API places the algorithm decision — and on
nothing else. Corrected 2026-10-06; the first two versions of this document had
this wrong.** I rejected josekit as *"a third native crypto stack plus an
external system library"*. **That is false, and the dev team caught it.**
`cargo tree -p sui-id -i openssl -e normal` shows `openssl` 0.10.81 already in
the default binary via `webauthn-rs → webauthn-rs-core →
webauthn-attestation-ca`. josekit would add **no new system library**, and this
binary already carries **three** native crypto stacks. Nor is there a
supply-chain argument underneath: `flate2`, `regex`, `anyhow` and `time` — the
rest of josekit's dependencies — are **all already in the tree** (via
`tower-http`, `leptos`, `leptos_hot_reload` and `webauthn-rs-core`).

**The ground that does hold, and it now carries the decision alone:** josekit
verifies through `deserialize_compact_with_selector` — the selector receives the
header and returns a `JwsVerifier` for one algorithm. **Nothing in josekit
enforces "`header.alg` is in the operator's configured allowlist" or "the key's
family matches the algorithm"; we would write both, inside the selector.** That
is precisely the hand-written policy glue that decided B against C, so rejecting
josekit for it is the same argument applied consistently. In josekit's favour,
recorded: it has **no `none` algorithm** either, so that pitfall is closed by
both candidates.

**This leaves the recommendation standing on one true reason instead of three,
two of which were false. That is a weaker position than I first presented, and
it is the honest one.**

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

## The operator-facing half

The first version of this document weighed only safety. *"APIs and UI/UX for
users not to be confused or misunderstand"* is the other half of the standard,
and for this decision "users" means the **operator of a self-hosted IdP**, who
configures an upstream provider and then has to understand why a login failed.
Three things follow, and they are part of the recommendation, not commentary on
it.

**1. The allowed-algorithm set is configuration, and its defaults decide
whether an operator can be fooled.**

| | Behaviour, and why |
|---|---|
| Key name | Use the spec's own words — `id_token_signing_algs`, next to the provider's `id_token_signing_alg_values_supported`. An operator comparing our config to their provider's discovery document should see the same vocabulary, not a synonym. |
| Omitted | A documented default of the four the matrix permits. Safe and predictable. |
| **Never** | **Do not take the allowed set from the provider's own discovery document.** That lets the party being verified choose the verification policy. It would look convenient and read as reasonable, which is what makes it the dangerous default. |
| Empty list | **Refuse to start**, naming the key and the provider. An operator who wrote `[]` meant something; silently substituting the default is exactly the misunderstanding this half of the philosophy forbids. The library agrees — `decoding.rs:335` makes an empty allowed set an error — but the operator must hear it at startup, not at the first login. |
| Contains `HS*` or `none` | **Refuse to start**, saying why: symmetric and unsigned algorithms have no meaning against a public JWKS. Rejecting it silently at verification time would leave a config file that looks accepted. |

**2. A rejected federated login needs one operator-readable reason.** The
matrix's negative rows are not just test cases; each is something an operator
will hit in production and must be able to tell apart: unknown `kid` (usually
rotation), `alg` not in the allowed set (log the alg **and** the allowed set),
signature invalid, `iss`/`aud`/`nonce` mismatch, expired or not-yet-valid (log
the skew used). **`jsonwebtoken`'s `ErrorKind` is mapped to our own taxonomy,
not surfaced raw** — a library's wording is written for its callers, not for
our operators, and it changes between versions. The **end user** sees a generic
failure; the detail goes to the operator log only, so the error is not an
oracle.

**3. After this change the codebase has two JWT verifiers, and a reader must
not confuse them.** `oidc/jwt.rs` verifies **tokens we issued** and is
EdDSA-only on purpose; the new code in `id_token.rs` verifies **tokens an
upstream provider issued**. Each module's first doc line states which direction
it serves, and the upstream one states explicitly that **it is not the issuer's
verifier and widening it does not widen that**. The whole reason C beats A is
that these two policies stay separate — a future reader who cannot tell them
apart is how they get merged back together.

**4. Three hazards in the chosen library that the operator half makes
non-negotiable.** Added 2026-10-06 after the dev team's verification pass.

**All four of `jsonwebtoken`'s protections hang on one flag.**
`decoding.rs:335-348` guards the empty-set error, the key-family binding **and**
the `alg` allowlist with the same `if validation.validate_signature`. A reader
who believes they are switching off one check switches off four. The call site
therefore constructs `Validation` in **one function, with a comment saying
exactly this**.

**Downgraded 2026-10-06, by my own re-check:** `validate_signature` is
`pub(crate)`, and its only public mutator,
`Validation::insecure_disable_signature_validation()` (`validation.rs:163`), is
**`#[deprecated]` in 10.3.0** — *"Use `jsonwebtoken::dangerous::insecure_decode`
if you require this functionality."* Under G07/G07b's existing `-D warnings`,
**nobody in our tree can switch those four checks off today without an explicit
`#[allow]`.** What survives is narrow: do not write that `#[allow]`, and do not
assume the deprecation is still there when the pin moves. I overstated this
hazard and am recording the correction rather than quietly softening it.

**The crate ships two explicitly dangerous entry points:**
`Validation::insecure_disable_signature_validation()` (`validation.rs:163`) and
`jsonwebtoken::dangerous::insecure_decode` — **defined at `decoding.rs:299` and
re-exported by `pub mod dangerous` at `lib.rs:16-17`**. (An earlier draft cited
`tests/dangerous.rs`, which only imports it; the dev team caught that. `mod
decoding` is private, so the re-export is the only public path.)

**Both are made unreachable by the build, and the mechanism is verified against
the real crate.** There is no `clippy.toml` in this workspace today. A throwaway
crate depending on `jsonwebtoken` 10.3.0, with both paths in
`disallowed-methods`, run under `cargo +stable clippy`:

```
warning: use of a disallowed method `jsonwebtoken::dangerous::insecure_decode`
warning: use of a disallowed method `jsonwebtoken::Validation::insecure_disable_signature_validation`
  = note: `#[warn(clippy::disallowed_methods)]` on by default
```

**On by default, and it resolves through the private-module re-export** — which
had to be checked, because an entry that failed to resolve would have given us
a gate that passes vacuously, the exact failure I required self-tests for on
G21. Under `-D warnings` both become build failures. A rule a developer cannot
accidentally break is worth more than one written in a doc comment, and that is
this half of the philosophy applied to the people who maintain the code.

**One feasibility fact from the same run:** the proposed dependency line
compiles, offline, against what is already cached —
`jsonwebtoken = { version = "10.3", default-features = false, features = ["aws_lc_rs"] }`.

## What I need

**Two things.**

1. **A direction on C in the form above** — the dependency change is yours, not
   mine, which is why this is still a decision request and not a dispatch.
2. **A nod to the five configuration behaviours in the table**, in particular
   *refuse to start* rather than *fall back to the default* on an empty or
   symmetric-containing list. Refusing to start is the safer reading and the
   clearer one, but it is a behaviour an operator meets at upgrade time, so I am
   not going to choose it on your behalf.

3. **Separately, and not blocking: this binary already links three native crypto
   stacks** — `aws-lc-sys`, `ring` and `openssl`, the last via `webauthn-rs`.
   Nothing in the repository records that. It is outside 096-A and I attach no
   recommendation to it yet; I am raising it because it is a real statement
   about attack surface, build time and supply chain, and I only found it
   because a false claim of mine was checked.

With 1 and 2 I can stage 096-A immediately.
