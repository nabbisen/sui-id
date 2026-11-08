# RFC 143 stage 1 — measure the three conditions, change nothing

> ## Whose turn it is: **the implementation role's.**
> Cycle K stage 1 —
> [`cycle-k-and-0.89.0-2026-11-08.md`](cycle-k-and-0.89.0-2026-11-08.md) §2.

**Dispatched.** 2026-11-08, by the architect.
**Governing RFC.** [RFC 143](../../accepted/143-installation-without-a-toolchain.md),
**Accepted**, freeze exception granted 2026-11-01. Building is permitted.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 0 — what this stage is, and what it is not

**Measurement only.** No change to `publish.yml`, `ci.yml`, any contract, any
crate, or any document. The output is a table and a recommendation that the
architect turns into stage 2. **If a measurement overturns what RFC 143 settled,
stop and report it**, because the settled design is yours to change only by
saying so.

**Settled, and not reopened:** static musl builds for both `x86_64` and
`aarch64`; vendored OpenSSL; `SHA256SUMS` plus build-provenance attestation;
the release notes as the `CHANGELOG` section verbatim.

## 1 — build both static targets

`x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, release profile,
from the baseline. Record the exact commands and toolchain.

- **`aarch64`: cross-compile it here.** GitHub's native `ubuntu-24.04-arm`
  runner is the other option, and it is the better one if it works, because
  the artifact is then also *tested* on the architecture it claims. **But it
  can only be reached by a workflow run, which §4 forbids in this stage**, so
  it is stage 2's rehearsal to measure, not yours. What this stage needs is
  the cross-compile's cost and any obstacle: toolchain, linker, vendored
  OpenSSL's C cross-build. Then stage 2 can compare the two with evidence.
- **OpenSSL:** enable `openssl`'s `vendored` feature for these builds only,
  and say exactly how: a feature, a `[target.…]` table, or a build-time flag.
  **Do not change `Cargo.toml` on `main`**: record the change needed, and put
  any experimental edit in a scratch copy.
- Report each artifact's **size**, and `file`/`ldd` output proving it is
  static.

**Where to build:** a clone beside the tree on the main disk, never under
`.git-exclude/tmp/clones/` and never in `/tmp` (30 GiB tmpfs; a cold build
plus cross toolchains will not fit).

## 2 — the three conditions, each with a number

### 2a — OpenSSL is unreachable: measure it in the artifact

The claim is that sui-id links OpenSSL only through `webauthn-attestation-ca`
and never calls it. **Test it at link level.** In the built static binary, do
any OpenSSL symbols survive dead-code elimination (`nm`, or `objdump -t`,
looking for `SSL_`, `EVP_`, `X509_`, `OPENSSL_`)?
- **None survive**: "unreachable" is a measured fact about the artifact.
  Report the command and its empty output.
- **Some survive**: name the path that keeps them (which crate, which call).
  That is the finding the review asked for.

Also state whether the release profile uses LTO or `--gc-sections`, since the
answer depends on it.

### 2b — Argon2 on musl, against RFC 126's recorded figures

RFC 126 recorded **about 34 ms per verification**, from a **release** build
(an earlier measurement of 693 ms turned out to be a debug build). So compare
**release against release**, on the same machine:
- one verification, median over at least 50 runs: glibc build, then musl
  build with the system allocator;
- **under contention**: N concurrent verifications, with N = the worker
  count the server actually uses. That is where musl's allocator is known to
  hurt, so it is the comparison that matters.

**If musl is more than 25% slower under contention**, measure **one**
alternative allocator in the static build, and say which and why. **Do not
adopt it.** A new dependency is decided before it lands, never adopted inside
a measurement round.

### 2c — resolution on musl, by short hostname

The review's case: *"works on glibc, fails on musl"* against an internal LDAP
server named by short hostname.
- Start an **OpenLDAP container** on a user-defined container network, reached
  by its **short name** (container DNS uses exactly the search-domain path
  that breaks).
- Run the **musl** binary in a container on the same network, configured
  against it, and **sign in through the directory**. Run the glibc build the
  same way as the control.
- **The same for SMTP** (a mail-catcher container, one send) **and a
  federation provider host** (any container answering the discovery URL by
  short name).

**The result is pass or fail per binary per protocol, with the failure output
if any.** If containers are not available to you, say so, and say what you
ran instead. Do not substitute a resolution test that skips the short-name
path.

## 3 — and one smoke test

The static binary, in a **bare** container (`debian:stable-slim`, or
`scratch` if it truly needs nothing), with a minimal config: does it start and
serve the first-run setup screen? This is RFC 143's closure condition in
miniature, run early.

## 4 — constraints

- **Change nothing tracked.** Scratch edits stay in the clone, and the
  architect reviews the recommended changes before any land.
- **No new dependency adopted**, allocator included.
- **Never on `main`**: no `workflow_dispatch`, no new workflow, no tag.
- **Container images**: `debian` is an official image; the LDAP and mail ones
  (for example `osixia/openldap` or bitnami's, and `mailpit`) are not, so use
  well-known ones, **pinned by digest**, and name each with its tag and digest
  in the return.

## 5 — what to return

A review request at
`.git-exclude/review-requests/cycle-k-stage1-measure-the-three-conditions-2026-11-08.md`:
- the baseline;
- how each target was built, with commands;
- sizes, and the static proof;
- **2a**: the symbol check and its output;
- **2b**: the timing table (glibc and musl, single and contended) against 34
  ms, plus an allocator measurement if 2b triggered one;
- **2c**: pass or fail per binary per protocol, with the container setup;
- **3**: did it reach the setup screen;
- **your recommendation for stage 2**: build `aarch64` natively or
  cross-compiled; allocator or not; anything the release workflow must do
  that RFC 143 did not foresee.

**On sizing:** one round, mostly waiting on builds. If the aarch64
cross-toolchain is the long pole, report `x86_64` first, and say so.
