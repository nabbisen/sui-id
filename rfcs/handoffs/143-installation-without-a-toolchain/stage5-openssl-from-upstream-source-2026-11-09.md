# RFC 143 stage 5 — OpenSSL 3.5 LTS, from upstream source

> ## Whose turn it is: **the implementation role's.**
> Decided 2026-11-09 —
> [`the-vendored-openssl-is-out-of-support-2026-11-09.md`](the-vendored-openssl-is-out-of-support-2026-11-09.md).
> **This document is the dispatch.** `0.89.0` stays held until this is rehearsed.

**Dispatched.** 2026-11-09, by the architect.
**Governing RFC.** [RFC 143](../../done/143-installation-without-a-toolchain.md),
**Accepted**, D2 as amended 2026-11-09.
**Baseline.** The tip of `main` when you start (`git log -1`; record it).

## 0 — the prototype, so you are not starting from nothing

**The architect built option A on 2026-11-09**, in a sibling clone at
`57dcd2f`. The scripts are in `.git-exclude/tmp/proto-openssl35/`:
- `proto-build.sh` and `proto-probe.sh`;
- `openssl_probe.rs`;
- `manifest-and-lock.diff`.

It ran in `rust:alpine` (rustc 1.99.1, musl-native), x86_64 only.

| Step | Result |
|---|---|
| Source | `openssl-3.5.9.tar.gz` from the URL that openssl-library.org/source/ links: `https://github.com/openssl/openssl/releases/download/openssl-3.5.9/…`. SHA-256 `603f5602e2eef00d77fbd429d34dcd5822bb301757a1bc9cdb24c670f1eb859a`, equal to the published `.sha256` |
| OpenSSL | `./Configure linux-x86_64 no-shared no-tests --prefix=/opt/openssl --libdir=lib --openssldir=/etc/ssl`, then `make install_sw` → `libcrypto.a`, `libssl.a`, `OpenSSL 3.5.9 29 Sep 2026` |
| sui-id | With the `vendored` table removed: `OPENSSL_DIR=/opt/openssl OPENSSL_STATIC=1 cargo build --release -p sui-id --features ldap --target x86_64-unknown-linux-musl`, 3m01s. `static-pie linked`. `strings` finds `OpenSSL 3.5.9 29 Sep 2026` and nothing else |
| `Cargo.lock` | **The only change**: `openssl-src 300.6.1+3.6.3` removed (11 lines) |
| The crypto it is for | `openssl_probe` through the same static library: **ES256** (P-256/SHA-256, as `COSEKey::verify_signature` builds it) verifies, and a flipped signature fails; **RS256** verifies; **EdDSA** verifies |
| Runs | In bare `debian:stable-slim` with `ca-certificates`: `303 → /setup` |

**What the prototype did not prove**: Ubuntu's `musl-gcc` (CI's linker, unlike
Alpine's native `gcc`); **aarch64**; and the workflow itself. That is this
stage's job. Alpine had `linux-headers`. **`musl-gcc` on Ubuntu has no kernel
headers on its include path**, which OpenSSL's build is known to need.
**Read `openssl-src`'s own `build.rs`** (how it configures for musl) **before
choosing flags.** It has solved this already. Cite what you take from it.

## A — the pinned input

A new contract, **`contracts/openssl.toml`**: `series = "3.5"`, `version =
"3.5.9"`, `sha256 = "603f…859a"`, and the URL template. **One place, read by
the workflow and by the check (D)**, so a bump is one reviewed edit.
Register it wherever the contract registry requires. If a change to it must
trigger the release rehearsal or CI lanes, make sure it does (the scope rules,
`compute-changed-scope.py`).

**Pinning a new version** is a documented procedure (§E). The hash is taken
**after** checking OpenSSL's PGP signature (`.asc`) once, against OpenSSL's
published signing keys. **Measure whether that is practical here**, and write
the procedure from what you ran.

## B — the release workflow builds OpenSSL, then links it

In `release-artifacts.yml`'s `build` job, before `cargo build`:
1. Download the tarball from the pinned URL and `sha256sum -c` it against the
   contract's hash. **Any mismatch fails the job.**
2. Build a static OpenSSL for the matrix target (`linux-x86_64` /
   `linux-aarch64`), using `musl-gcc`, with the flags you settle in §0.
3. `cargo build … --locked` with `OPENSSL_DIR` and `OPENSSL_STATIC=1`.
4. **Assert the binary embeds exactly the pinned version**: `strings` must find
   `OpenSSL <version> ` and no other `OpenSSL N.N.N` string. Print it into the
   summary beside `rustc -Vv`.
5. **Build and run the probe on both architectures.** Commit `openssl_probe` as
   an example, under the name and location you judge right, with `openssl` as
   a **dev-dependency** of `sui-id`. That crate is already in the tree through
   `webauthn-rs-core`, at the same version, so it adds nothing to the supply
   chain. **Say so with the `Cargo.lock` diff.** The probe must fail the job
   on a wrong result, as its `assert!`s do. **Why:** no test in the repository
   signs a real passkey assertion (§F), so without this, OpenSSL's
   signature-verification path in the shipped artifact is exercised nowhere.

**Cache nothing across runs** for OpenSSL. A release builds it from the
verified source, every time.

## C — the manifest

Remove `crates/sui-id/Cargo.toml`'s `[target.'cfg(target_env = "musl")'.dependencies]`
table **and its comment**. The comment explains a mechanism that no longer
exists. **Grep the descriptions too**: `vendored`, `openssl-src` and "vendored
OpenSSL" in the docs, the CHANGELOG `[Unreleased]` entry, the RFCs' current
text, comments and workflows. Read each hit, and correct the ones that now
describe something false. **G23's manifest rule** stays green.

## D — the freshness check compares with OpenSSL, not with a crate

Replace `scripts/check-openssl-src-freshness.py` (and its job in
`audit.yml`, renamed to say what it checks). It reads `contracts/openssl.toml`,
then:
1. **Fails** when upstream has a newer patch release in the pinned series. Source:
   OpenSSL's own tags (`openssl/openssl`, `openssl-3.5.N`).
2. **Fails** when the pinned series is **not in OpenSSL's supported list**
   (openssl-library.org/source/, the *Series / End-of-Life* table). That is the
   rule that would have caught 3.6.
3. **Warns** when the series' end-of-life date is within **180 days**, so a
   series move is planned, not forced.
4. **Exits 2, never 0, when it cannot read either source** (network, or a
   changed page layout). An unreadable source must turn the lane red, never
   pass it. That is the lesson of this whole stage.

**Self-tests** with no network, in the existing style, from saved fixtures of
both sources: behind; current; series dropped from the list; EOL within 180
days; EOL beyond; a page that parses to no table → exit 2; a tag list with no
tag in the series → exit 2. **Mutation:** remove rule 2, and the "dropped from
the list" test goes red. Quote it.

**`check-publish-preconditions.py` already requires `Security audit` green on
the exact commit**, so this check still blocks a release with no new rule.

## E — the documents

- **`release-process.md`**: the commitment paragraph now says what a bump is:
  `contracts/openssl.toml`'s version and hash, by the pinning procedure (A).
  It also says that the check reads OpenSSL itself, and why: the paragraph should
  not repeat the error this stage corrects.
- **RFC 143's text** is the architect's. Do not edit `rfcs/`.
- **`CHANGELOG.md` `[Unreleased]`**: the stage 2 `Added` entry, if it says
  "vendored", is corrected, and the static binaries' OpenSSL is named as 3.5 LTS.
  Operators reading `0.89.0`'s notes should know which OpenSSL they run.

## F — recorded, not built here

**No test signs a real passkey assertion.** `finish_passkey_authentication`
(`crates/sui-id-core/src/authn/webauthn.rs:279`, `federated_mfa.rs:610`) is
reached in no test, because no software authenticator is in the dev-dependencies.
B5's probe covers the shipped library's crypto, not sui-id's use of it. A
real assertion test needs a new dev-dependency, which is a decision. The
architect records it, and this stage does not add one.

## G — constraints and what to return

- **No tag, no dispatch, no Release, no `rfcs/` edits.** The architect commits
  and rehearses.
- `bash scripts/local-matrix.sh --changed` (`Cargo.toml`/`Cargo.lock` change,
  so expect the complete matrix), quoting the scope line and **the names of
  any failing checks**. Also A3.4, the fixtures, and every touched script's
  self-test run directly.
- **Reproduce B locally before returning**, in `ubuntu:24.04` with
  `musl-tools` (CI's environment, not Alpine's), for x86_64. Then say what
  aarch64 will need that you could not run.
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/cycle-k-stage5-openssl-from-upstream-source-2026-11-09.md`,
with:
- the baseline;
- the musl flags and where they came from;
- the PGP measurement and the pinning procedure;
- D's tests and mutation;
- the Ubuntu reproduction transcript;
- **the rehearsal's criteria, set before it runs**. At least: both legs embed
  exactly `OpenSSL 3.5.9`; the probe passes on both; a hash mismatch fails the
  job (say how you proved that locally); and stage 2–4's criteria still hold.
