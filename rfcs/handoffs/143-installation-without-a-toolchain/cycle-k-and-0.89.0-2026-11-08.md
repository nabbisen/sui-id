# Cycle K and `0.89.0` — sui-id installs without a toolchain

> ## Authorized 2026-11-08. `@nabbisen`: *"Authorized."* (scope), *"Approved."* (the ROADMAP relabel)
> **The `release` reviewer, §5 item 2: denied**, *"It is for release stability
> and treated as a future improvement, for the configuration harms productivity
> now."* RFC 141 §2, `publish.yml`'s header and `release-process.md` now say the
> reviewer is deferred, and that the dispatch is the approval.
> **Stage 1 dispatched** — [`stage1-measure-the-three-conditions-2026-11-08.md`](stage1-measure-the-three-conditions-2026-11-08.md).

**Proposed.** 2026-11-08, by the architect, on `@nabbisen`'s *"Scope 0.89."*
**Schedule source.** `ROADMAP.md`, *The embeddable boundary, and installation*
(re-proposed 2026-11-01), which already names RFC 143 for `v0.89.0`. **This
document makes that plan concrete; it does not change it.**

## 1 — what `0.89.0` is

**One RFC, [143](../../accepted/143-installation-without-a-toolchain.md):
a person with neither `cargo` nor `git` downloads sui-id, verifies it is what
this project published, and reaches the first-run setup screen.** The RFC is
Accepted, and its feature-freeze exception was granted on 2026-11-01 (it adds
no route, no configuration key and no runtime code). Two small carried fixes
ride along (§3). **Nothing else**, deliberately. This release changes the
publish workflow, the step that cannot be undone, so it should not share a
release with product changes that would muddy what a failure means.

**What is already settled, and is not reopened here:**
- **Targets:** static (musl) builds for **both `x86_64` and `aarch64`**.
- **OpenSSL:** vendored. It is the one C dependency, reached through
  `webauthn-attestation-ca`, which sui-id never calls.
- **Integrity:** `SHA256SUMS` plus GitHub build-provenance attestation, both
  produced by the release workflow and never by a workstation.
- **Release notes:** the `CHANGELOG` section, verbatim.
- **Quick Start order:** download, verify, run first; then `cargo install`;
  then build from source.

**And three conditions that must hold before any artifact ships**, from the
design review your acceptance carried:
1. an **LDAP resolution test on musl** against a real directory host,
   because musl's resolver is not glibc's and short hostnames are the classic
   failure;
2. an **Argon2 timing comparison** against RFC 126's recorded figures, because
   musl's allocator is slower under thread contention;
3. the vendored OpenSSL's **unreachability tested, not asserted**.

## 2 — the stages

**Stage 1 — measure, change nothing.** Build both static targets and answer
the three conditions with numbers:
- **How to build `aarch64`**: GitHub's native `ubuntu-24.04-arm` runners, or a
  cross-compile. Measure, don't assume. A native runner means the artifact is
  built and *tested* on the architecture it claims.
- **OpenSSL reachability, at link level**: after linking with dead-code
  elimination, are OpenSSL's symbols still in the binary at all? If none
  survive, "unreachable" is a measured fact about the artifact. If some do,
  name the path that keeps them.
- **Argon2** on musl with the system allocator, against glibc and RFC 126's
  figures. If it is slower beyond a stated margin, measure one alternative
  allocator. **Adopting one is a new dependency, and comes back to you as a
  decision**, not a detail.
- **Resolution on musl** against a real OpenLDAP container reached by **short
  hostname** on a container network. That network's DNS is exactly the
  search-domain path that breaks, so the test reproduces the risk. The same
  check is run for SMTP and a federation provider host, since the review named
  all three.
- Artifact sizes, and whether the static binary starts and reaches the setup
  screen in a bare container.

**The output is a table and a recommendation. It goes nowhere near
`publish.yml`.** Anything that overturns the settled design comes back here
first.

**Stage 2 — build, prove and attest, without publishing anything.** The build
lives in the release workflow, on the tagged commit, **before** the six
`cargo publish` steps (RFC 143 D1), so a build failure costs nothing. It
produces:
- both artifacts, `SHA256SUMS`, and a provenance attestation per artifact,
  with the attestation action **pinned by SHA in `[actions]`** (A3.4);
- **the LDAP resolution check, run against the built musl artifact**, so
  that every release re-proves condition 1, not only this one.

**And a rehearsal path**: the same build triggered without a tag, which
builds, checks and verifies but publishes nothing and creates no Release.
RFC 143's own rule is not to build on a foundation that has never run. The
rehearsal is how the workflow runs before the release it exists for.

**Stage 3 — the GitHub Release.** It is created only after all six crates
are published. Its body is the version's `CHANGELOG` section, extracted by a
script with its own tests, and the artifacts and `SHA256SUMS` are attached.
**A failure after the artifacts are built but before publish succeeds leaves
no Release**, and the partial-publish report says so.

**Stage 4 — the documents.** `quick-start.md` reordered (download, verify,
run), with the glibc/`libssl` prerequisites moved to the `cargo` path they
belong to; `deployment.md`; `release-process.md`. **The verification
instructions say plainly what each check proves:**
- `sha256sum -c` is universal, but proves only that the download matches the
  file published beside it;
- `gh attestation verify` proves the binary came from **this repository's
  workflow at that commit**, but needs `gh`.

The operator chooses with that understood. A checksum presented as more than
it is would be the misunderstanding your second half forbids.

**Closure: after `0.89.0` is published, against the real release.** In a
clean container with neither `cargo` nor `git`, follow `quick-start.md` from
the top: download the published artifact, verify it, and reach the setup
screen. Then flip one byte and show that verification fails. **RFC 143's
closure prerequisites can only be met by a real release**, so it closes in
the `0.89.0` records, the way RFC 142 closed at `0.88.0`.

## 3 — two small carried fixes

1. **Three code comments still say K01 is the "no actor" shape**
   (`crates/sui-id-store/src/commands.rs`, at the lines cycle J stage 4
   recorded). K01 requires a human actor. A comment that misstates a
   command's actor requirement misleads the next person who copies it.
2. **The ROADMAP's cycle letters**, as a record correction. The forward plan
   calls its next three cycles J, K and L, but the cycle that shipped `0.88.0`
   was also J. **Relabel them K (`v0.89.0`), L (`v0.90.0`) and M
   (`v0.91.0`)**, contents unchanged.

## 4 — what follows `0.89.0`, so this scope is read in context

**Not decided here.** The plan's next cycle (L, `v0.90.0`) is **the
store-boundary RFC**. Its blocker, RFC 119's acceptance, **has cleared**:
RFC 119 was accepted on 2026-11-01. But that RFC is **unwritten**, and
scheduling it is the feature-freeze exception that is yours (ROADMAP,
decision 1).

Waiting beside it are **RFC 144**, the interface legible to older eyes
(Proposed, your review), and **RFC 145**, passkey sign-in (Accepted, needing
your freeze exception). RFC 145's own recommendation is to build it after
RFC 144, as one release for older users.

**The architect's recommendation, for when you get there:** write the
store-boundary RFC **during** cycle K. That uses design capacity, not
implementation capacity, so it costs `0.89.0` nothing. Then decide whether
`0.90.0` is the store boundary (embeddable was ruled *"relatively
prioritized"* on 2026-09-25) or the older-users pair. **That choice is yours,
and it is not needed today.**

## 5 — what is yours now

1. **The scope**: `0.89.0` = RFC 143, four stages, plus the two carried
   fixes. **Recommended.**
2. **The `release` environment's required reviewer.** `0.88.0` and `0.87.2`
   both published without pausing, because the environment has no reviewer.
   RFC 141 §2 and `publish.yml`'s header both describe an approval gate that
   is not there. This release adds a second outward step, the GitHub Release.
   **Either** configure a required reviewer in the repository settings (only
   you can), so that the gate the documents describe exists; **or** say no
   reviewer is intended, and the architect amends RFC 141 and the workflow's
   header to say so. **Recommended: configure it before `0.89.0`'s publish**.
   An approval gate on the one irreversible step is cheap, and the documents
   already promise one.
3. **The ROADMAP relabel** (§3, item 2). **Recommended.**
