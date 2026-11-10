# RFC 143 — closure assessment, 2026-11-11

> ## Whose turn it is: **yours**, to approve or decline the closure.

**Prepared by.** The architect, 2026-11-11, after `0.89.0` was published.
**Asked of you.** Approve RFC 143's closure. On approval, the architect moves it
to `done/` with `Status: Implemented (v0.89.0)`, with this document as its
`Closure evidence`. **RFC 143's closure could only be met by a real release**,
so unlike RFC 142 it closes after the publish, not in the preparation.

**The release.** Tag `0.89.0`, annotated and signed (`git verify-tag`: good
signature), at **`9ff8ddb`**:
- **Level B** on that exact commit: `workflow_dispatch` run `42851297216`,
  **28 jobs, 28 success, 0 failed, 0 skipped**.
- **`Security audit`** on the same commit: run `42851303082`, success.
- **Published** by `publish.yml` on the tag itself (run `42921837076`,
  `ref=0.89.0`, `sha=9ff8ddb`): verify, build-artifacts, publish and
  github-release all succeeded.
- `check-published-versions`: *"0.89.0 is published on all 6 crates"*, exit 0.

## Independence, stated plainly

**The architect is not independent of this RFC.** It wrote the RFC, reviewed
its design at your request, dispatched every stage, and **folded code into
most of them**: the workflow fixes in stages 2, 3 and 5, the
`check-publish-preconditions` design in fix round 2, the extraction script's
fence and fragment handling, the freshness check's contract rule, and the
test-port race fix before the tag. That makes the architect a co-implementer.
*"The implementer cannot be the sole approver of a security-sensitive design
or its closure evidence"* (`docs/development-specification.md:1011`). **This
document is preparation. Your approval is the closure.**

## The closure prerequisites, against the evidence

RFC 143's field, one clause per row. **Measured on 2026-11-11 against the
published release**, in a clean `debian:stable-slim` container: `cargo=none
git=none`, with only `ca-certificates` (the stated runtime requirement) and
`curl` (to download) installed.

| Clause | Evidence |
|---|---|
| *"A person with neither `cargo` nor `git` can install a release of sui-id"* | The Quick Start's commands, as written: `ARCH=$(uname -m)`, `VERSION=0.89.0`, two `curl -fLO` downloads from the Release, then `install -m 0755` |
| *"…verify that what they received is what this project published"* | **The checksum line**, as documented: `sui-id-x86_64-unknown-linux-musl: OK`, exit 0. **The provenance command**, as documented (`gh attestation verify … --repo nabbisen/sui-id --source-ref refs/tags/0.89.0`, on the file the container downloaded): exit 0. It names the signer `release-artifacts.yml@refs/tags/0.89.0`, ref `refs/tags/0.89.0`, commit `9ff8ddb`, the tagged commit |
| *"…and reach the first-run setup screen"* | `sui-id --print-sample-config`, `listen_addr`/`issuer` set as the Quick Start says, first run printed the setup URL, and `GET / -> 303 http://127.0.0.1:8801/setup` |
| *"…following only `quick-start.md`"* | Every command above is the Quick Start's own, in order. Two departures, both stated. **`sudo` was omitted**, since the container runs as root. **The provenance check ran with the host's `gh`**, on the container's downloaded file, because the Quick Start states that check needs `gh` v2.68.0+ and sign-in, which a bare container does not have. The checksum, install and run were all inside the container |
| *"Every artifact is produced by the release workflow, never by a workstation"* | All three assets carry attestations from `release-artifacts.yml` at the tag. The downloaded files' hashes equal the Release API's own `digest` fields: binary `fe52060352c5…`, `SHA256SUMS` `e9ebf160dc8e…`. And `github-release` confirmed the asset list and digests before finishing |
| *"…its integrity is verifiable without trusting the download host"* | The attestation is signed through Sigstore and checked against this repository's workflow identity, not against anything served beside the file. **And it is release-specific**: the same file checked with `--source-ref refs/heads/main` fails (exit 1), so a rehearsal build can never pass as a release |
| *"…a tampered or truncated artifact fails the documented verification step rather than installing"* | **One byte flipped** (offset 1,000,000): checksum exit 1, attestation exit 1. **Truncated** to 20,000,000 bytes: checksum exit 1, attestation exit 1. The intact file: exit 0 on both |

**`aarch64`**: not run here, because this host has no ARM hardware. The
release's own `build-artifacts` ran it natively on `ubuntu-24.04-arm`, for this
exact commit (run `42921837076`). It built against OpenSSL 3.5.9, passed the
signature probe, and reached `303 → /setup` in a bare container. That is the
workflow's evidence, not a reproduction by the architect.

## What the RFC said that did not hold, recorded so closure is honest

1. **D2's premise was false twice.**
   - OpenSSL is reachable: it verifies every passkey sign-in (stage 1).
   - `openssl-src` does not follow OpenSSL's patch releases (2026-11-09). The
     static build now links OpenSSL 3.5 LTS, built from OpenSSL's own source
     and checked against a pinned SHA-256. The freshness check reads OpenSSL
     itself.
   - Both corrections are in D2's own text, with your rulings (*"static"*,
     *"A"*).
2. **The header's *"No crate, and no runtime code"* is not what shipped.**
   Stage 1's smoke test found sui-id **crashing at startup on a host with no CA
   certificates**, on every build. Fixing it changed runtime code
   (`hibp.rs`, `egress.rs`, `state.rs`, `startup.rs`). The fix was inside this
   RFC's purpose, since the static binary's target hosts are where it bites,
   and it is in `0.89.0`'s `Fixed` section. **The header is corrected on the
   move to `done/`.**
3. **`aarch64` ships without position-independence**, stated in the Quick
   Start and the release notes, on your ruling (*"Agreed."*).

## What remains open, and is not this RFC's

- **No test signs a real passkey assertion.** The release's probe proves the
  shipped OpenSSL's ES256, RS256 and EdDSA verification, not sui-id's own call
  into it. A real test needs a software-authenticator dev-dependency, which is
  your decision when it is scheduled.
- **The `release` environment still has no required reviewer**, deferred by
  your 2026-11-08 ruling.
