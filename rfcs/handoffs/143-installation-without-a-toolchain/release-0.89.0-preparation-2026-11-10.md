# `0.89.0` — release preparation

> ## Whose turn it is: **the implementation role's**, for the mechanical half.
> The tag, the `Security audit` readings, the publish dispatch, the Quick
> Start's one open sentence, and **the RFC and ROADMAP records** stay with the
> architect.

**Dispatched.** 2026-11-10, by the architect.
**What this release is.** Cycle K: RFC 143, installation without a toolchain,
plus security maintenance. Authorized 2026-11-08 —
[`cycle-k-and-0.89.0-2026-11-08.md`](cycle-k-and-0.89.0-2026-11-08.md).
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
**Baseline.** The tip of `main` when you start (`git log -1`; record it).
**Step 0.** `Security audit` dispatched on `main` at `dab768d`: **success**, run
`42811952945`. `cargo audit` is clean apart from the allowed `paste` warning,
and `openssl freshness` reports `3.5.9 is current for series 3.5`. Nothing to
carry in. (RUSTSEC-2026-0459, found earlier the same day, is already fixed in
`aeae382`.)

## 0 — what it contains

`[Unreleased]` has three subsections, all already written:
- **`Added`**: downloadable static binaries for x86_64 and aarch64 Linux, with
  `SHA256SUMS` and build provenance, linked against OpenSSL 3.5 LTS; and the
  Quick Start now leads with downloading.
- **`Fixed`**: sui-id no longer crashes at startup on a host with no CA
  certificates.
- **`Security`**: `rusqlite` 0.41, bundling SQLite 3.54.1, closing
  RUSTSEC-2026-0459.

**No schema change**: `git diff --name-only 0.88.0..HEAD --
crates/sui-id-store/src/migrations/` is empty.

**This is the first release that runs `build-artifacts` and `github-release`**
inside `publish.yml`. Read `release-process.md`'s *What runs, in order* end
to end before writing anything, because §3's upgrade note describes what an
operator now gets.

## 1 — the version bump

**Six sites**, all in the root `Cargo.toml`, measured today: `:21` `version`,
and `:34-38`, the five internal crates. Each `0.88.0` → `0.89.0`. Refresh
`Cargo.lock` by letting cargo do it, and confirm only the workspace members
moved (the six, plus `xtask`).

**The Quick Start's `VERSION=` line** (`quick-start.md:27`) already reads
`0.89.0`, set in stage 4. Confirm it, as the checklist's version-bump item
now requires, and change nothing.

## 2 — the two pinned documents

A minor release, so the arithmetic moves. `check-doc-authority.py` takes the
lag from the minor component, with `tolerance_minor = 2`:
1. **`docs/development-specification.md`**, the banner (`:8-10`): `Stale as of
   v0.88.0 (2026-11-08)` → `v0.89.0` and the cut's date; `eleven minor versions`
   → **twelve**; `nine more than` → **ten**. Read the rest of the banner's
   sentences too, and say whether any other number in it is derived from the
   version.
2. **`docs/threat-model.md:4`**: `**v0.88.0**` → `**v0.89.0**`. Leave its own
   `Stale as of` line alone.

## 3 — one operator note

**`docs/src/guides/upgrade.md`**, a new `### v0.89.x` section at the top of
*Version-specific notes*, in the existing sections' style. Keep it short:
- **No migration runs** from `0.88.x`, so no pre-migration snapshot is taken.
  Say so, so nobody looks for one.
- **A new way to install**: the static binary. An operator who moves from
  `cargo install` to it **verifies it first**: link to the Quick Start's
  download section. Restate nothing.
- **The CA bundle**: a host without one now starts, with a warning, instead of
  crashing. Say what that warning means, in one sentence, and link to where it
  is described.

## 4 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.89.0] — <the cut's date>`, with a fresh empty
`[Unreleased]` above it. **Three subsections exist, `Added`, `Fixed` and
`Security`.** Do not merge, reorder or rewrite them.

**The one-line summary must name every subsection** (the release-summary
rule):
1. **Lead** with what makes this release what it is: sui-id can now be
   **downloaded as a verified static binary**, with no toolchain.
2. **Then** the startup fix for hosts with no CA certificates.
3. **Then** the bundled SQLite security update.

Use one sentence or two. **Do not** name RFC numbers or stages. **This section
is the GitHub Release body**, verbatim, so render it before returning:
`python3.14 scripts/extract-changelog-section.py --changelog CHANGELOG.md
--version 0.89.0 --repo nabbisen/sui-id --ref main --root .`. Quote the
output. It must exit 0, and every link in it must have resolved.

**No link-reference block**: confirm `grep -c '^\[0\.' CHANGELOG.md` is
still `0`.

## 5 — packaging verification

```
cargo package -p sui-id-shared --allow-dirty
```

**Report the output verbatim**, including any warning. `sui-id` now carries
an `examples/openssl_probe.rs` and an `openssl` dev-dependency. Say whether
`cargo package -p sui-id --allow-dirty --list` includes the example, as a
fact for the record. It cannot be verified end to end before the others are
on the index.

## 6 — what you must not do

- **No tag**, no publish, no `workflow_dispatch` of anything.
- **Do not touch `rfcs/`, `ROADMAP.md`, or `contracts/owner-attribution-baseline.txt`.**
- **Do not touch the `<!-- OPEN: … -->` comment** in `quick-start.md` (`:74`).
  It waits on an open question, and the architect replaces it in the tagged
  commit.
- **No Level B claim.**
- **Do not restate a gate's command** in any document (RFC 131 D4).

## 7 — what to return

`.git-exclude/review-requests/release-0.89.0-preparation-2026-11-10.md`, with:
- the baseline;
- the six sites and the `Cargo.lock` diff;
- the two pinned documents, with the arithmetic shown;
- the upgrade note's text;
- the promoted `CHANGELOG`, its summary, and **the rendered Release body**;
- `cargo package`'s verbatim output, and the `--list` finding;
- `bash scripts/local-matrix.sh` (**full**, not `--changed`), with output and
  count from the real tree, and **the names of any failing checks**;
- per-hunk hashes, beside the package.

Leave the working tree uncommitted.
