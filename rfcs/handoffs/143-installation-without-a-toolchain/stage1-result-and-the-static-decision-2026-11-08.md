# RFC 143 stage 1 result — the static decision comes back to you

> ## Decided 2026-11-08. `@nabbisen`: 1. *"static"*; 2. *"Your recommendation is accepted."*
> Static stands. The `openssl-src` freshness check and the release commitment
> are the architect's design for stage 2, which is how static is made safe, and
> are recorded as that, not as part of the ruling. A host with no CA bundle
> starts with one clear warning, and `ca-certificates` is the Quick Start's
> runtime requirement; no root set is compiled in. **Stage 2 dispatched** —
> [`stage2-build-check-and-attest-2026-11-08.md`](stage2-build-check-and-attest-2026-11-08.md).

**Prepared by.** The architect, 2026-11-08, on stage 1's return (baseline
`c46d621`). The measurements are the dev team's; **each finding below was
re-checked by the architect against the source or the data** before being
written here.

## 1 — what your static decision rested on, and what stage 1 found

On 2026-11-01 you settled RFC 143's Q1: *"Static sounds better to me."* The
RFC's argument for static, and the design review's condition on it, was that
the vendored OpenSSL is **unreachable**: linked only through
`webauthn-attestation-ca`, which sui-id never calls. The review required that
claim to be *"tested rather than asserted"*.

**It was tested, and it is false.**
- **OpenSSL verifies every passkey sign-in.** `webauthn-rs-core` 0.5.5's
  `COSEKey::verify_signature` (`crypto.rs:558`) builds an OpenSSL key
  (`get_openssl_pkey`) and verifies with it. It is called on every assertion
  (`core.rs:930`). SHA-256 there goes through OpenSSL's `sha` too.
- **In the static binary, 1,229 OpenSSL symbols survive thin LTO.**
- **On today's glibc build, OpenSSL is already a runtime dependency**:
  `ldd` shows `libssl.so.3` and `libcrypto.so.3`.

**The error was the architect's**, in RFC 143's own text: it searched `crates/`
for uses of "attestation", found none, and concluded OpenSSL was unreachable.
That measured the wrong thing. OpenSSL is reached through signature
verification, not through attestation. The review's condition is what caught
it.

**And the safety net the RFC relied on does not exist.** RFC 143 cited
`release-process.md`'s rule that an advisory with no reachable path does not
stop a release, which assumed OpenSSL advisories would at least *appear*.
**Measured** in the RustSec advisory database (current to 2026-11-06, 173
advisories from 2025 alone): `openssl-src`, the crate that carries vendored
OpenSSL, has **25 advisories, the newest dated 2023-02-07.** RustSec stopped
filing OpenSSL's vulnerabilities against it. **So with a static build, a new
OpenSSL vulnerability in passkey verification would not turn the `Security
audit` lane red.** The patch path becomes this project's, with nothing to tell
it to act.

## 2 — the other results, which hold up

| Condition | Result |
|---|---|
| **Argon2 on musl** | **Not slower.** Single verification: 37.5 ms on musl, 37.6 ms on glibc, against RFC 126's ~34 ms. Under contention (32 concurrent): equal within noise. No allocator change is needed |
| **LDAP by short hostname on musl** | **Pass**, with parity to glibc: LDAPS by container short name, certificate validated, bind and search completed |
| **SMTP by short hostname on musl** | **Pass**, with parity: STARTTLS, and the message was delivered |
| **Federation by short hostname** | **Resolves correctly** on musl, to the same address as glibc. The fetch is then refused by sui-id's own SSRF guard (RFC 134: private addresses are always denied, by design), so end-to-end discovery cannot be shown on a private container network on *any* build. That is not a musl issue |
| **`x86_64` static build** | Builds cleanly (needs the full `perl` package for OpenSSL's build); 24.9 MiB, `static-pie` |
| **`aarch64`** | Cross-compiles only with three toolchain workarounds, and **cannot be run** on this host. **Build it natively** on GitHub's ARM runner, where it is also tested |

**A bug, found by the smoke test, on every build:** sui-id **crashes at
startup on a host with no CA certificates**, whether HIBP is on or off. The
HIBP client is built unconditionally (`crates/sui-id/src/runtime/startup.rs:217`).
When its configured builder fails, it falls back to `reqwest::Client::new()`
(`crates/sui-id-core/src/authn/hibp.rs:105`, `:128`), which panics when no
certificates load. The glibc build crashes identically. This is a robustness
defect, fixable regardless of anything else here, and stage 2 should start
with it.

## 3 — the options for the static question

| | Static (vendored OpenSSL) | Dynamic (glibc builds, the OS's `libssl`) |
|---|---|---|
| Who patches OpenSSL | **This project**, by cutting a release | The operator's OS, on its own schedule |
| How anyone learns a patch is due | **Nothing today**, since RustSec stopped. It would need a new check (below) | The distribution's normal security updates |
| Runs on | Any Linux with a CA bundle | Only distributions with a glibc at least as new as the build host's, and `libssl3`. The CI runner's glibc is newer than Debian 12's, so building there would exclude a common server OS unless the build host is pinned older |
| Answers *"most users do not have `cargo`"* | Fully | Mostly: no toolchain, but a matching distribution |

**Static is still workable, with one addition: a freshness check.** OpenSSL
ships its security fixes as patch releases, and `openssl-src` follows each one
with a new crate version. A scheduled lane that **fails when `Cargo.lock`'s
`openssl-src` is behind the newest published patch release** detects exactly
what RustSec no longer reports, using data this project already reads. Each
failure is a security-maintenance release, which the freeze already admits.
The cost is a commitment: when OpenSSL ships a security release, sui-id cuts
one too.

**The architect's recommendation: static, with that freshness lane added in
stage 2 and the commitment written into `release-process.md`.** Three
reasons:
1. The people RFC 143 is for are the ones least able to match a glibc
   version.
2. The check costs one small scheduled job.
3. The commitment is the honest form of what static always meant. RFC 143
   understated it by calling OpenSSL unreachable.

**RFC 143's D2 text is corrected either way**: the unreachability argument is
removed, and replaced by what the decision actually rests on.

## 4 — what is yours

1. **Static or dynamic, now that OpenSSL is reachable and RustSec is silent
   on it.** Recommended: **static, with the `openssl-src` freshness lane and
   the release commitment.** Dynamic remains a sound choice if you would
   rather the OS carry OpenSSL. Then the open question becomes which glibc
   floor to build against.
2. **CA certificates on a bare host.** The crash is fixed regardless. The
   question is what a host with no CA bundle should get afterwards.
   **Recommended:** sui-id starts, logs one clear warning that outbound TLS
   (HIBP, SMTP, LDAPS, federation) cannot verify certificates until the
   system's CA bundle is installed, and the Quick Start lists `ca-certificates`
   as the one runtime requirement. **The alternative**, compiling Mozilla's
   root set into the binary, makes it self-contained. But it is a new
   dependency, and a second trust store that would silently disagree with the
   operator's own, which matters for internal LDAP and SMTP CAs. **Not
   recommended.**

**Stage 2 is held until you answer**, because what it builds depends on
answer 1.
