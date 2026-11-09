# RFC 143 stage 4 — the documents, and two carried items

> ## Whose turn it is: **the implementation role's.**
> Cycle K stage 4 —
> [`cycle-k-and-0.89.0-2026-11-08.md`](cycle-k-and-0.89.0-2026-11-08.md) §2.
> **This document is the dispatch.**

**Dispatched.** 2026-11-09, by the architect.
**Governing RFC.** [RFC 143](../../accepted/143-installation-without-a-toolchain.md),
**Accepted**, D3 and D4.
**Baseline.** The tip of `main` when you start (`git log -1`; record it).
Stage 3 is complete: rehearsal `42548940886` on `c57d6fa`.
**Rehearsal artifacts to test against:**
`.git-exclude/tmp/rehearsal-42548940886/`. It holds the two binaries,
`SHA256SUMS` and `release-notes.md`, attested from `refs/heads/main`. These
are real attested files, not mock-ups, so every command you document can be
run for real.

## A — `quick-start.md`: download, verify, run

**D4's order:** the downloaded binary first, then `cargo install sui-id` for
people who have the toolchain, then `git clone` for people building from
source. **The glibc/`libssl` prerequisites and the MSRV move under the two
`cargo` paths**, where they belong. Today they stand above everything and read
as requirements for everyone.

**The download path, which every command must have been run to write:**
1. **Choose the architecture**: `uname -m` gives `x86_64` or `aarch64`; name
   the file for each.
2. **Download** the binary and `SHA256SUMS` from
   `https://github.com/nabbisen/sui-id/releases/download/<version>/…`, with
   `curl -fLO`. **The version:** a `VERSION=…` line set to the current
   release, plus a link to the Releases page. **Propose how it stays current**:
   adding it to the release-preparation checklist is the obvious answer, and
   is fine if you find nothing better.
3. **Verify, before running**:
   - **The checksum.** An operator downloads **one** binary, and `sha256sum -c
     SHA256SUMS` then **fails** on the other file's missing line. **Measure**,
     in `debian:stable-slim` **and** `alpine` (BusyBox), whether
     `--ignore-missing` works in both. Otherwise use `grep ' <file>$'
     SHA256SUMS | sha256sum -c -`, and document the form that works on both.
     **Say what it proves:** that the download matches the file published
     beside it, and nothing about who built it.
   - **The provenance**: `gh attestation verify <file> --repo nabbisen/sui-id
     --source-ref refs/tags/<version>`. **Say what it proves:** that the file was
     built by this repository's workflow from that release's tag. **And say why
     `--source-ref` is not optional.** One binary can carry several true
     attestations: the architect measured two on a single file, from two
     rehearsals of an unchanged build. Without the flag, a passing check does
     not say *which* release the file is. It needs `gh`, so say so.
   - **What a failure means** (D3): *do not run it*, and report it through
     `.github/SECURITY.md`'s private advisory link.

   **The operator chooses with that understood.** Present the two checks as
   different strengths, never as one check written two ways.
4. **Run**: `chmod +x`, then into `Configure` and `First run`, which stay as they
   are. They already work for any binary.
5. **The one runtime requirement:** the system's CA bundle (`ca-certificates`
   on Debian/Ubuntu, and the equivalent elsewhere). Without it, sui-id starts
   with one warning, and HIBP checks, SMTP, LDAPS and federation cannot verify
   TLS. Link to where that warning is described, if anywhere.

**The `aarch64` sentence waits on an open question**, §3 of
[stage 2's result](stage2-result-and-the-aarch64-pie-question-2026-11-09.md).
Write the architect's recommended form as an HTML comment, `<!-- OPEN:
stage 2 result §3 is unanswered. … -->`, so it is visibly undecided. The
recommended form: *"The ARM build does not use address randomisation for its
own code (it is not position-independent); the x86_64 build does."* The
architect replaces the comment once §3 is answered. **Do not word it as
settled.**

## B — the other documents

- **`deployment.md` §3, *Install the binary***: add the downloaded binary as
  the first way, with `install -m 0755 -o root -g root`, and point back to
  the Quick Start's verification rather than restating it. Keep
  `cargo`/build as the second way.
- **`overview.md` and `faq.md`**: if either says sui-id must be built or
  installed with `cargo`, or lists `libssl` as a universal requirement, correct
  it. Grep, then **read** each hit.
- **`CHANGELOG.md` `[Unreleased]`, `### Added`**, in operator words:
  downloadable static binaries for x86_64 and aarch64 Linux on each GitHub
  Release, with `SHA256SUMS` and build provenance; and the Quick Start now leads
  with downloading. **This is the section `0.89.0`'s Release body will be**, so
  read it as a release note.

## C — the two items carried from stage 3's review

1. **`extract-changelog-section.py` refuses a link target that does not exist.**
   Add `--root` (the checkout). A rewritten target with no file or directory
   under it → exit 1, naming the target. **Why:** the link gates (`G10b`, `G14`)
   do not cover `CHANGELOG.md`, so a section linking a moved file would ship
   a 404 in a Release. On a release run the checkout is the tag, so this check
   runs before anything is published. Pass `--root .` in
   `release-artifacts.yml`.
   **Tests:** an existing target is rewritten; a missing one → exit 1; an
   `https://` target is never checked against the filesystem.
   **Mutation:** remove the check, and the missing-target test goes red.
2. **The verify step says what it verified.** In `release-artifacts.yml`'s
   *"Verify, exactly as an operator will"*, add `echo "verified: $f (source
   ref $GITHUB_REF)"` after each `gh attestation verify`. That command prints
   nothing on success when not on a terminal, so today the log does not show
   what passed.

## D — prove the Quick Start, the way closure will

**In a clean `debian:stable-slim` container with neither `cargo` nor `git`**,
follow the new Quick Start from the top, using the rehearsal artifacts in place
of the download (mount them, or `podman cp`), and `--source-ref
refs/heads/main` in place of the tag:
- the checksum passes;
- the attestation check passes **if `gh` is available to you**; if not, say
  so, and the architect runs that line;
- the binary reaches the setup screen;
- **then flip one byte and show that the checksum fails.**

Also run the checksum line in `alpine`. That is RFC 143's closure check,
rehearsed against the real attested files. The closure itself repeats it
against the published `0.89.0`.

## E — constraints and what to return

- **No tag, no dispatch, no Release.** The architect commits, and rehearses
  `release-artifacts.yml` for C.
- `bash scripts/local-matrix.sh --changed`, quoting its scope line **and the
  names of any failing checks**. Also the link gates on the touched documents,
  and the script self-test run directly.
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/cycle-k-stage4-the-documents-2026-11-09.md`,
with:
- the baseline;
- the Quick Start's install section as it now reads;
- D's transcript (commands and output), including the flipped byte;
- the `sha256sum` measurement in both images;
- C's tests and mutation;
- your proposal for keeping `VERSION=` current.
