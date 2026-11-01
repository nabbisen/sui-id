# RFC 143 — the design review record

**What this file is.** The durable record RFC 143's
`Independent design review` field cites, per the errata in
`rfcs/README.md` §*Errata: the `Independent design review` field`*: the
field must say what the review was and was **not** independent of.

## Who reviewed it, and under which rule

**The architect, at `@nabbisen`'s request.** His words, 2026-11-01:
*"Review the questions carefully by yourself with my fundamental
philosophy. Any question remains?"* — `ROADMAP.md` §S1c's owner-invoked
exception to the self-review prohibition, which requires the record to
say he asked. He asked.

**The architect wrote this RFC, so the review is not independent of
it.** §S1a places design review with him or an external architect he
engages; none was engaged.

## What the review found, and it corrected the RFC's own premise

**D2 asserted that a static musl build "needs nothing".** That was
wrong, and it was the RFC's argument for musl.

Measured 2026-11-01: **OpenSSL is the only C dependency, and it arrives
through one feature sui-id never uses.** `webauthn-rs-core` 0.5.5
depends on `webauthn-attestation-ca` **unconditionally** — not behind
`webauthn-rs`'s `attestation` feature. Setting
`default-features = false` on `webauthn-rs` was tried and **openssl
survived the dependency tree**. Everything else is already pure Rust:
`rusqlite` is `bundled`, TLS is rustls throughout (`ldap3`
`tls-rustls-ring`, `reqwest` `rustls`, `tokio-rustls`). `attestation`
matches **once** in `crates/`, as the `attestationObject` JSON field
name in a test fixture.

So a static build needs `openssl`'s `vendored` feature, which compiles
OpenSSL into the artifact — **turning Q1 from a list of triples into a
posture question**, since this project then becomes the patch path for
a C library rather than the operator's OS.

## The two conditions the review attaches, which were not in the RFC

`@nabbisen`, 2026-11-01: *"Static sounds better to me. Is there any
concern, loss or demerit around it?"* — there are four, and **two are
conditions rather than footnotes**:

1. **musl's resolver is not glibc's**, and this product resolves
   operator-supplied hostnames for **LDAP**, SMTP and federation
   providers. musl bypasses NSS and has historically differed on search
   domains and large-response TCP fallback. *"Works on glibc, fails on
   musl"* against an internal LDAP server named by short hostname is
   the classic instance. **An LDAP resolution test against a real
   directory host is required before the artifact ships** — otherwise
   the easier install arrives by breaking directory sign-in for exactly
   the self-hosters it was built for.
2. **musl's allocator is slower under thread contention**, and Argon2 is
   allocation-heavy. **An Argon2 timing comparison against RFC 126's
   recorded figures is required**, not an assertion that it is fine.
   Mitigable with a different allocator in the static build.
3. The OpenSSL patch path becomes this project's — an advisory means
   cutting a release while operators wait.
4. Minor: larger artifact, an aarch64 cross-build toolchain, worse
   backtraces.

**Mitigating (3), and it needs testing too.** The linked OpenSSL
appears unreachable, and `release-process.md` already holds that *"an
advisory with no reachable path into the built artifact is recorded and
does not stop a release."* **That reachability claim must be tested
rather than asserted** — and if it holds, the better long answer may be
neither option: make attestation-ca optional upstream so the C
dependency disappears.

## What this review does not discharge

The closure prerequisites stand as written. **This RFC ships no runtime
code**, which is also the architect's argument that it is an authorized
exception to the feature freeze — an argument `@nabbisen` accepted on
2026-11-01, and which the record notes was not the architect's to
infer.
