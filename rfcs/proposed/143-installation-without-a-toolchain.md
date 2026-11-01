# RFC 143 — Installing sui-id without a toolchain, and trusting what you installed

**Status.** Proposed
**Security review.** Required — this RFC creates a **new distribution channel
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
| `x86_64-unknown-linux-musl` | **The one that actually answers the question asked.** The glibc builds still need `libssl.so.3` and a matching glibc; a static musl build needs nothing, which is what "most users do not have a workstation to build" really requires. |

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

1. **D2's target set** — three artifacts, or musl plus `x86_64-gnu`? The
   architect leans the latter.
2. **Whether this is an authorized exception to the feature freeze.** The
   architect's argument that it is: it adds **no product surface** — no route,
   no configuration key, no runtime code — and lands entirely in release
   machinery and documentation. It is closer to the freeze's admitted
   "repairs required to keep approved gates executable" than to a feature.
   **But the freeze's exceptions are a closed list and this is not on it**, so
   the exception is his to grant, not the architect's to infer.
