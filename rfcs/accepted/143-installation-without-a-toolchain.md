# RFC 143 — Installing sui-id without a toolchain, and trusting what you installed

**Status.** Accepted
**Accepted on.** 2026-11-01
**Approved by.** `@nabbisen`, 2026-11-01: *"Accepted."* — settling both open questions below. **Q1 on static**, his words being *"Static sounds better to me"*, and the review attaches two conditions to it that were not in this RFC: an **LDAP resolution test on musl** and an **Argon2 timing comparison** against RFC 126's recorded figures, both before any artifact ships. **Q2, the feature-freeze exception, granted** — the architect's argument was that this adds no product surface, and the record notes that extending a closed exception list was never the architect's to infer.
**Security review.** Required
**Independent design review.** [Design review record, 2026-11-01](../handoffs/143-installation-without-a-toolchain/design-review-2026-11-01.md). **By the architect, at `@nabbisen`'s request** — *"Review the questions carefully by yourself with my fundamental philosophy"* — `ROADMAP.md` §S1c's owner-invoked exception, and the record says he asked. **The architect wrote this RFC and is therefore not independent of it**; the review's main finding is that this RFC's own D2 premise was wrong. §S1a makes engaging an external architect his option, and none was engaged. — this RFC creates a **new distribution channel
for an authentication server's binary**, which is a supply-chain surface. The
convenience is the easy half; the integrity is the RFC.
**Design prerequisites.** None.
**Implementation prerequisites.** [RFC 141](../accepted/141-publishing-is-a-workflow-not-a-credential.md)
in use — the first release published through its workflow is `0.87.0`. This
RFC extends that workflow rather than adding a second release path, and doing
so before the first has run once would be building on an untested foundation.
**Closure prerequisites.** A person with neither `cargo` nor `git` can install
a release of sui-id, verify that what they received is what this project
published, and reach the first-run setup screen — following only
`docs/src/getting-started/quick-start.md`. Every artifact is produced by the
release workflow, never by a workstation; its integrity is verifiable without
trusting the download host; and a tampered or truncated artifact fails the
documented verification step rather than installing.
**Tracks.** Distribution. Raised by `@nabbisen` on 2026-11-01: *"Offer much
easier installation and Quick Start. Most users do not have `cargo`,
workstation to build. Many of them do not have even `git`."*
**Touches.** `.github/workflows/publish.yml`, `docs/src/getting-started/quick-start.md`,
`docs/src/guides/deployment.md`, `docs/src/contributing/release-process.md`,
and a new contract entry. **No crate, and no runtime code.**
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

## Summary

Today sui-id can only be installed by compiling it. `quick-start.md` offers
two paths and both need a Rust toolchain: `cargo install sui-id`, which needs
`rustup` because the 1.95 MSRV is newer than distribution-packaged toolchains,
plus `build-essential pkg-config libssl-dev`; or `git clone` and
`cargo build --release`. **A prospective operator with neither `cargo` nor
`git` cannot install sui-id at all**, by any documented route.

This RFC adds prebuilt release artifacts and a Quick Start that uses them.

**The reason it is an RFC and not a workflow tweak** is that it makes this
project a binary distributor for the first time. An identity provider's binary
is a high-value target: whoever replaces it owns every account it serves.
Convenience that ships without verifiable provenance would be a net loss, and
"download this and run it" is the single most attacked instruction in
software distribution.

## Background — what exists and what does not

- **`cargo install sui-id` works** and will continue to. It is the right path
  for anyone who already has the toolchain, and nothing here replaces it.
- **No release artifact has ever existed.** `gh api repos/nabbisen/sui-id/releases`
  returns `0`: there are 137 tags and no GitHub Releases, so no binary, no
  checksum file, no notes.
- **RFC 036 "Distribution readiness" (Implemented, v0.37.0) is not this
  work**, despite the name. It was README corrections and `Cargo.toml` package
  metadata. **Recorded because the title is misleading**: a reader looking for
  this theme finds 036 and stops.
- **`publish.yml` already is the release machinery** (RFC 141): it verifies
  Level B and release identity before anything is minted, runs behind the
  `release` environment's required reviewer, and holds the credential-free
  Trusted Publishing path. The artifacts belong there, behind the same gate,
  not in a second workflow with its own trust story.

## Design

### D1 — the artifacts are built by the workflow, never by a workstation

Every published artifact is produced inside the same `publish.yml` run that
publishes the crates, from the commit the tag names, after
`check-publish-preconditions.py` has passed. **No artifact may be uploaded
from a developer machine** — that is the credential-and-provenance argument
RFC 141 already made for tokens, applied to bytes.

**This also fixes an ordering question**: the artifacts must be built *before*
the six `cargo publish` steps, so a build failure costs nothing, rather than
leaving the registry updated and the release half-published.

### D2 — which targets, and the honest reason for each

Three, chosen to cover the stated problem rather than to look thorough:

| Target | Why |
|---|---|
| `x86_64-unknown-linux-gnu` | The deployment guide's own assumption, and the overwhelming majority of self-hosted Linux. |
| `aarch64-unknown-linux-gnu` | Small-ARM self-hosting is where "I have no build toolchain" is most often literally true. |
| `x86_64-unknown-linux-musl` | **The one that actually answers the question asked.** The glibc builds still need `libssl.so.3` and a matching glibc; a static build needs neither. |

> **Correction, 2026-11-01, before this RFC was reviewed.** An earlier draft
> said a static musl build "needs nothing". **It needs vendored OpenSSL**, and
> the measurement is this: `webauthn-rs-core` 0.5.5 depends on
> `webauthn-attestation-ca` **unconditionally** — `[dependencies.webauthn-attestation-ca]`
> with no `optional`, so it is not behind `webauthn-rs`'s `attestation`
> feature. Setting `default-features = false` on `webauthn-rs` was tried and
> **openssl survived in the tree**. Everything else is already pure Rust:
> `rusqlite` is `bundled`, and TLS is rustls throughout (`ldap3`
> `tls-rustls-ring`, `reqwest` `rustls`, `tokio-rustls`), so **OpenSSL is the
> single C dependency and it arrives through one unused feature of one crate.**
> `openssl`'s own `vendored` feature exists (`vendored = ["ffi/vendored"]`), so
> a static build is feasible — by compiling OpenSSL into the artifact.
>
> **That is a supply-chain consequence, not a build detail, and it is why Q1
> below is a real question rather than a preference.** A dynamically linked
> artifact is patched by the operator's own OS when an OpenSSL advisory lands.
> A statically linked one is patched by **this project cutting a release** —
> so shipping static means accepting that latency for a C library.
>
> **The argument that this is acceptable here**: sui-id uses no WebAuthn
> attestation API at all — one match for `attestation` across `crates/`, and
> it is the `attestationObject` JSON field name in a test fixture. So the
> linked OpenSSL is, as far as that measurement goes, unreachable, and
> `release-process.md` already holds the rule that *"an advisory with no
> reachable path into the built artifact is recorded and does not stop a
> release."* **That reachability claim should be tested, not assumed**, before
> it is relied on.
>
> **Tested 2026-11-08 (stage 1): the claim is false, and the paragraph above is
> superseded.** OpenSSL verifies **every passkey sign-in**: `webauthn-rs-core`
> 0.5.5's `COSEKey::verify_signature` builds an OpenSSL key for each assertion,
> and 1,229 OpenSSL symbols survive in the static binary. Searching for
> "attestation" measured the wrong thing. **And the advisory rule cannot carry
> it**: RustSec has filed no `openssl-src` advisory since 2023-02-07, so a new
> OpenSSL vulnerability would not reach the `Security audit` lane at all.
> **Static was re-decided on those facts.** `@nabbisen`, 2026-11-08: *"static"*.
> What it now rests on, as designed for stage 2: a scheduled check that **fails
> when `openssl-src` is behind its newest patch release**, since OpenSSL ships
> its security fixes as patch releases. And a release commitment: an OpenSSL
> security release means a sui-id release. That is what static always meant;
> this text had understated it. Record —
> [`../handoffs/143-installation-without-a-toolchain/stage1-result-and-the-static-decision-2026-11-08.md`](../handoffs/143-installation-without-a-toolchain/stage1-result-and-the-static-decision-2026-11-08.md).

**No macOS or Windows target.** sui-id is a server, the deployment guide
documents systemd, and shipping an artifact this project neither tests nor
documents running would be a claim it cannot support. **Stated as a decision,
not an omission** — if that is wrong, it is wrong on purpose and can be
revisited with evidence.

**Open question for the approver — Q1**: whether musl should be the *only*
Linux target. One static artifact is simpler to document, simpler to verify,
and removes the glibc-version trap entirely; three artifacts cover more
existing habits. The architect leans **musl plus `x86_64-gnu`**, dropping
`aarch64-gnu`, and wants this settled before implementation rather than during.

### D3 — integrity, which is the point of the RFC

**`SHA256SUMS` for every artifact**, generated in the workflow and attached to
the release, and **GitHub's build provenance attestation** for each binary
(`actions/attest-build-provenance`), which binds the artifact to the workflow,
the repository and the commit that produced it.

**The verification step is part of the documented install, not an appendix.**
Quick Start tells the operator to verify before running, with the exact
command, and says what a mismatch means: **do not run it, and report it**.
A checksum nobody is told to check is decoration.

**What attestation gives that a checksum does not**: a checksum published
beside the artifact by whoever published the artifact proves only internal
consistency. Attestation lets an operator verify the binary came from *this
repository's workflow at that commit*, without trusting the download page.

**Pin the attestation action by 40-hex SHA and record it in
`contracts/gate-inputs.toml`'s `[actions]` table** — A3.4 condition 1 and 2,
the same requirement RFC 141 met for `crates-io-auth-action`.

### D4 — Quick Start leads with the artifact, and `cargo install` stays

`quick-start.md`'s Install section is reordered: **download, verify, run**
first; `cargo install sui-id` second, for people who have the toolchain;
`git clone` third, for people building from source. Today's order is backwards
relative to the audience the document is for.

**The glibc/`libssl` prerequisites move with the path they belong to** rather
than standing above all three as they do now, where they read as requirements
for everyone — including the musl path that has none.

### D5 — what a release's notes say

`publish.yml` creates the GitHub Release and attaches the artifacts and
`SHA256SUMS`. Its body is **the `CHANGELOG` section for that version,
verbatim**, not a second hand-written summary that can drift from it. The
`CHANGELOG` is already written to be read by an operator; a release note
generated from it inherits that and cannot disagree with it.

## Alternatives rejected

- **A container image instead of binaries.** A reasonable answer to a
  different question: it needs a runtime the stated audience may not have
  either, and `getting-started/overview.md` currently sells sui-id on being
  *one binary* against *"JVM / Node / container runtime"*. Shipping a container
  as the primary path would contradict the project's own pitch. Worth
  revisiting as an addition, never as the answer to this.
- **An install script (`curl … | sh`).** Rejected. It trains operators to pipe
  a network response into a shell, on a project whose whole subject is not
  trusting what you did not verify. The same convenience is available by
  documenting three commands the operator can read.
- **A distribution package (`.deb`/`.rpm`) or a third-party repository.**
  Each is a separate trust root, a separate signing story and a separate
  maintenance burden per distribution, for an audience this project cannot
  currently measure. Not now; not never.
- **Building artifacts in a separate workflow.** Rejected on D1: a second
  release path means a second trust story, and the one `publish.yml` already
  has was expensive to build.

## Open questions for the approver

1. **D2's target set, and it is now a posture question rather than a list.**
   The correction above means the choice is **static (vendored OpenSSL, this
   project patches it) against dynamic (the operator's OS patches it)**, not
   simply which triples to build. The architect's recommendation: **ship both
   architectures, `x86_64` and `aarch64`** — small-ARM self-hosting is exactly
   the population that cannot build — and **static**, on the unreachability
   argument above, **conditional on that unreachability being tested rather
   than asserted**. What remains yours is whether sui-id should become the
   patch path for a C library it links but does not call.
2. **Whether this is an authorized exception to the feature freeze.** The
   architect's argument that it is: it adds **no product surface** — no route,
   no configuration key, no runtime code — and lands entirely in release
   machinery and documentation. It is closer to the freeze's admitted
   "repairs required to keep approved gates executable" than to a feature.
   **But the freeze's exceptions are a closed list and this is not on it**, so
   the exception is his to grant, not the architect's to infer.
