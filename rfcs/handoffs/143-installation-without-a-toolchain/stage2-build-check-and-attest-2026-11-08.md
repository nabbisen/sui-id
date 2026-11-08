# RFC 143 stage 2 — build, check and attest, and publish nothing

> ## Whose turn it is: **the implementation role's.**
> Cycle K stage 2 —
> [`cycle-k-and-0.89.0-2026-11-08.md`](cycle-k-and-0.89.0-2026-11-08.md) §2.

**Dispatched.** 2026-11-08, by the architect.
**Governing RFC.** [RFC 143](../../accepted/143-installation-without-a-toolchain.md),
**Accepted**. Static re-decided on stage 1's facts —
[`stage1-result-and-the-static-decision-2026-11-08.md`](stage1-result-and-the-static-decision-2026-11-08.md).
**Baseline.** The tip of `main` when you start — `git log -1`, record it.
**Read stage 1's review package first**:
`.git-exclude/review-requests/cycle-k-stage1-measure-the-three-conditions-2026-11-08.md`.
This dispatch builds on its commands and does not restate them.

## 0 — four parts, one return

**A** is a bug fix in the product, **B** a dependency change, **C** the
release workflow, and **D** a scheduled check. The GitHub Release (stage 3)
and the documents (stage 4) are **not** this stage.

**You cannot run a GitHub workflow, and must not.** Write C and D
uncommitted, test what can be tested locally, and return. **The architect
commits, then dispatches the rehearsal** (§C3) **and reports the run**, and
fixes iterate from there.

## A — no client built at startup may panic for want of CA certificates

**Measured in stage 1:** with no CA bundle on the host, sui-id **panics at
startup on every build**, whether HIBP is enabled or not:
- `HttpHibpClient::new()` (`crates/sui-id-core/src/authn/hibp.rs:101-106`,
  and the twin in `with_timeout`, `:124-129`) falls back to
  `reqwest::Client::new()`, which panics when no certificates load. It is
  built unconditionally (`crates/sui-id/src/runtime/startup.rs:217`,
  `crates/sui-id/src/main.rs:352`).
- **The next client in line would do the same.** `build_federation_client()`
  ends in `.expect("failed to build federation HTTP client")`
  (`crates/sui-id/src/runtime/egress.rs:32-35`), and `AppState` builds it
  unconditionally (`crates/sui-id/src/runtime/state.rs:95`). The smoke test
  stopped at HIBP only because HIBP is built first. The comment there calls
  failing fast intended, but **a crash for an operator who has no federation
  configured is not failing fast, it is failing for no reason.**

**Measure first: every HTTP client or TLS context built at startup**, not just
these two (SMTP and LDAP too, if they build anything eagerly). Run each with an
**empty CA bundle** (`SSL_CERT_FILE` pointing at an empty file, and
`SSL_CERT_DIR` at an empty directory; `rustls-native-certs` 0.8.5 honours
both), and record which panic.

**The required behaviour, settled 2026-11-08** (*"Your recommendation is
accepted"*):
- **sui-id starts.** No client construction panics for want of certificates.
- **One warning, logged once at startup, in operator words**, saying that no
  CA certificates were found, so outbound TLS cannot verify servers, and
  naming what that breaks: **HIBP checks, SMTP, LDAPS and federation**. Also
  say how to fix it (install the system's CA bundle, e.g. `ca-certificates`).
- **Say plainly what HIBP then does.** HIBP treats an unavailable service as
  **allowed** (`hibp.rs:254`, fail-open), so on such a host **no password is
  breach-checked**. The warning must say so, because that is the consequence
  an operator would least expect.
- Each feature that needs TLS then fails **at use**, with its own error,
  not at startup.
- **No root set is compiled in** (the ruling).

**Tests:** an e2e or integration test that starts the app with an empty CA
bundle and asserts no panic, the warning (text and level), and that a HIBP
check returns `Unavailable` rather than crashing. **Mutation:** restore
`reqwest::Client::new()` in the fallback, and the test goes red.

`CHANGELOG.md` `[Unreleased]`: a `### Fixed` entry in operator words.

## B — vendored OpenSSL for musl builds only

Land stage 1's measured form, from its package, in `crates/sui-id/Cargo.toml`:
a `[target.'cfg(target_env = "musl")'.dependencies]` table enabling
`openssl`'s `vendored` feature. Confirm again that the glibc build's `ldd` is
unchanged, and quote the `Cargo.lock` diff. **G23's manifest rule** (no
`test-support` on production tables; `rusqlite` only in the store) **must stay
green**: this table is a production table, and it carries neither.

## C — the release workflow builds, checks and attests

### C1 — where

The build belongs to the release, on the tagged commit, **before** the six
`cargo publish` steps (RFC 143 D1), so a build failure costs nothing.
**Recommended shape:** a reusable workflow (`release-artifacts.yml`,
`on: workflow_call` **and** `workflow_dispatch`) that `publish.yml` calls,
and that the rehearsal can run on its own. If you see a better shape, say why.

### C2 — what it does

1. **Build** `x86_64-unknown-linux-musl` on `ubuntu-24.04` and
   `aarch64-unknown-linux-musl` **natively on `ubuntu-24.04-arm`**, from stage
   1's commands. On the native runner the `aarch64` binary is PIE; stage 1's
   was non-PIE only because of the cross-toolchain workaround, so check and
   report it.
2. **Prove each binary is static** (`file`, plus `ldd` on its own
   architecture), and **run it**: start it with a minimal config and get the
   setup redirect, on each architecture, in a bare `debian:stable-slim` with
   only `ca-certificates`.
3. **The LDAP check, on the built musl binary, every release.** This is RFC
   143's condition 1, re-proved each time rather than once: an OpenLDAP
   **service container** reached by its **short name**, LDAPS with a generated
   CA, and a directory sign-in through the built binary. Assert on the log
   lines stage 1 used (*"user-source cascade matched"*), not on the HTTP
   status, which is a 200 failure page for a non-admin by design. **Add the
   SMTP check the same way** if it costs little; say if it does not.
4. **`SHA256SUMS`** over every artifact.
5. **Build-provenance attestation** per artifact, `actions/attest-build-provenance`,
   **pinned by 40-hex SHA and recorded in `contracts/gate-inputs.toml`'s
   `[actions]` table** (A3.4 conditions 1 and 2). It needs `id-token: write`
   and `attestations: write`. **Grant them to that job only**, never
   workflow-wide.
6. **Upload** the artifacts and `SHA256SUMS` as workflow artifacts, so stage
   3 can attach them to the Release.

**Pin every action by SHA.** Container images used by the LDAP check are
**pinned by digest**.

### C3 — the rehearsal

`workflow_dispatch` with no tag: it builds, runs, checks and attests the tip
of `main`, publishes nothing, and creates no Release. **It attests too**,
because the attestation step is the one most likely to fail for a permissions
reason, and the rehearsal exists to find that before a real release. Its run
summary says plainly that it is a rehearsal. **The architect runs it**; you
cannot (§0).

## D — the `openssl-src` freshness check

**Why** (stage 1): RustSec has filed no `openssl-src` advisory since
2023-02-07, so a new OpenSSL vulnerability would not reach `Security audit`.
OpenSSL ships security fixes as patch releases, and `openssl-src` follows each
one.

**Add a job to `.github/workflows/audit.yml`** that fails when `Cargo.lock`'s
`openssl-src` is **behind the newest published patch release of the same
OpenSSL series**. Measure the crate's version scheme first: `300.x.y+3.x.y`
embeds the OpenSSL version, so define "the same series" from it and say how.

**Putting it in `audit.yml` is the point.** `check-publish-preconditions.py`
already refuses a publish unless `Security audit` succeeded on the exact
commit, and step 0 and step 5 already read that workflow. **So a stale
OpenSSL blocks a release with no new rule.** Write a self-test for the
comparison (behind, current, ahead, and a series change), in the project's
existing script-test style, registered in `contracts/workflow-template.toml`
if `ci.yml` needs regenerating.

`docs/src/contributing/release-process.md` gains **one paragraph**: what the
check does, that a failure is a security-maintenance release (which the freeze
admits), and that **an OpenSSL security release therefore means a sui-id
release**. That is the commitment static carries.

## E — the carried fix

The three code comments in `crates/sui-id-store/src/commands.rs` that call K01
the "no actor" shape (cycle J stage 4 recorded them at `:1020`, `:6865` and
`:6922`; re-find them). K01 requires a human actor. Correct the claim; touch
nothing else.

## F — constraints and evidence

- **No tag, no `workflow_dispatch`, no publish.** The architect runs the
  rehearsal.
- `bash scripts/local-matrix.sh --changed`, quoting its scope line. A is a Rust
  change, so it will be the complete matrix. Run it on the real tree. A
  pre-check clone, if you use one, goes beside the tree on the main disk.
- **A workflow you cannot run is still checkable**: run `actionlint` if it is
  available, and A3.4 (pinning) on the real tree.

## G — what to return

`.git-exclude/review-requests/cycle-k-stage2-build-check-and-attest-2026-11-08.md`:
- the baseline;
- **A**: which startup clients panicked under an empty bundle, the fix, the
  warning's exact text, the test and its mutation;
- **B**: the `Cargo.lock` diff, and the glibc `ldd`;
- **C**: the workflow files, with the permissions per job, and what you could
  check locally;
- **D**: the version-scheme analysis and the self-test;
- **E**: the corrected comments;
- per-hunk hashes; the matrix scope line and summary;
- **what the rehearsal must show for you to call C done**, so the architect's
  run is judged against criteria set before it ran.
