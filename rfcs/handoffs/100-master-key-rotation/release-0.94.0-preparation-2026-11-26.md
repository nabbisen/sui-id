# `0.94.0` — release preparation

> ## Whose turn it is: **the implementation role's.**
> This is the mechanical half of the release. These stay with the architect and the owner:
> - the tag, the publish, and the `Security audit` readings;
> - the Quick Start's `VERSION=`, at the cut;
> - **the RFC and ROADMAP records**: RFC 100 to `done/`, RFC 151's closure, and RFC 148/107's closure assessment, which the
>   architect writes in parallel with this.

**Dispatched.** 2026-11-26, by the architect.

**What this release is.** Cycle P, the first of its two releases
([`cycle-p-and-0.94.0-2026-11-24.md`](cycle-p-and-0.94.0-2026-11-24.md)). **M2c closed 2026-11-25** (`8d8a81f`). Every build
has landed on `main`:

| Build | What | Commit |
|---|---|---|
| P1 | RFC 100 stage 6, the database half | `c3b8920` |
| P2 | the rotation rewritten once, on the journal | `59ff4c9` |
| P3 | custody of the kept backup, and the operator's guide | `2660276` |
| P7 | a rotation refuses beside a running sui-id, and says what it means | `070e7f7` |
| P5 | RFC 151, the password minimum can be raised | `a3e8dfc` |
| P8 | introspection agrees with userinfo (a held security fix, published with it) | `31106bf` |
| P6 | RFC 097 stage 1, the inventory and G24 | `d78e995` |
| P9 | a restore refuses beside a running sui-id | `43c5c3e` |

**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)

**Baseline.** The tip of `main` when you start. **Record `git log -1` in the return, and say which commit the matrix ran
on**, if that is not the same one.

## A — the version bump

Six sites in the root `Cargo.toml`: `version` (`:21`), and the five internal crates (`:34-38`). Each goes from `0.93.1` to
`0.94.0`. Let cargo refresh `Cargo.lock`, and confirm that only the workspace members moved: the six, plus `xtask`.

**Do not change the Quick Start's `VERSION=`** (`docs/src/getting-started/quick-start.md`). `0.94.0` cannot be downloaded
until the publish, so the architect changes it at the cut.

## B — the two pinned documents

Apply the rule in `contracts/doc-authority.toml`, as checked by `scripts/check-doc-authority.py`, to two pins:
- `docs/development-specification.md`'s banner;
- `docs/threat-model.md`'s.

**Show the arithmetic** for each: quote the line of the script that compares, then give what the pin says, what `0.94.0` is,
and whether it falls inside the tolerance. Change a pin only if the rule requires it, and say so.

## C — the upgrade note

Add `### v0.94.x` to `docs/src/guides/upgrade.md`, above `### v0.93.x` (`:59`). **Lead with what an operator must act on,
in this order.** Check each fact on the binary or the code before writing it:

1. **Three migrations run**: `0053`, `0054` and `0055`. Confirm with
   `git diff --name-only 0.93.1 HEAD -- crates/sui-id-store/src/migrations`. So **a pre-migration snapshot is taken**. Say where,
   measured from the code that takes it.
2. **`admin master-key rotate` now requires an old-key custody choice.**
   - The two forms are `--retain-backup-until <instant> --custody-reference <text>` and
     `--discard-backup-once-verified --verification-reference <text>`.
   - Without one, it is a usage error. **Measure the exit code on the binary.**
   - **A script that rotates must change.** Say it plainly, and quote the usage error's real text.
3. **A rotation, and now a restore, refuse while sui-id is running against the same database**, naming
   `<key_file>.rotation-lock`. Quote both refusals from the binary. Then say three things:
   - **stop the server first**;
   - **never delete the lock file**;
   - **upgrade every sui-id process that uses the database before rotating**, because a version older than `0.94.0` takes no
     lock.
4. **New commands**: `admin master-key backup remove` and `backup renew`, and the kept-backup warning at every start. One
   line each.
5. **No action needed:**
   - **The password minimum** is a setting now. It stays 12 after the upgrade until an administrator raises it.
   - **Introspection** answers inactive for a disabled or deleted user's unexpired token. Name it for anyone running a
     resource server, and point to the CHANGELOG's *Security* entry.
6. **Configuration.** Compare `Config`'s fields at `0.93.1` and at `HEAD`, and say what changed, if anything.
7. **For anyone building against the crates**, one line per breaking change to a published crate's public API since
   `0.93.1`. **Measure it; don't recall it.** Use `cargo semver-checks` if it is available here (say which version), or else a
   diff of `pub` items at the two tags, and say which method you used. Expect at least:
   - `KeyFileOps` gaining `lock_shared`, and `LockGuard`'s signature;
   - `CoreError`'s two new variants;
   - `Strings` fields that changed type (the hints now take the minimum);
   - `StoreError`'s new variant;
   - `ServerSettings`' new field.

Keep it short. The guide already explains rotation and custody; link to it rather than repeat it.

## D — the `CHANGELOG` promotion

`[Unreleased]` becomes `## [0.94.0] — 2026-11-26`, with a fresh empty `[Unreleased]` above it. **The architect resets the
date at the cut** if the tag lands on a later day.

Move the five subsections whole: `Added`, `Changed`, `Fixed`, `Removed` and `Security`. **Do not reword a bullet, with one
exception, which the architect decides here:**
- **Delete the `Changed` bullet that announces `--remove-backup-after-verified` being renamed to
  `--discard-backup-once-verified`.**
- **Why:** measured at the `0.93.1` tag, neither name exists in `cli.rs` or in the CHANGELOG. The rename happened between two
  unreleased stages. In the released notes, that bullet would be the only place an operator ever met the old name, and they
  would go looking for a flag that never shipped.
- The bullet above it, that `admin master-key rotate` now requires a custody choice, is a real change from `0.93.1`, and
  stays.

**The one-line summary names every subsection**, leading with what an operator must act on: the custody choice, and the
refusals beside a running sui-id. It must also name:
- the `Security` entry (introspection);
- the `Removed` one (the old `.bak.<timestamp>` naming);
- the password minimum;
- the threat-model inventory.

Draft it; the architect reviews it.

Render the Release body:
`python3.14 scripts/extract-changelog-section.py --changelog CHANGELOG.md --version 0.94.0 --repo nabbisen/sui-id --ref main
--root .`. Quote the output; it must exit 0. Confirm that `grep -c '^\[0\.' CHANGELOG.md` is still `0`.

## E — packaging, every crate

**After the bump**, run `cargo package --workspace --exclude xtask --allow-dirty`, and include its output verbatim. All six
must package and verify.
- **Clear `target/package` first.** `0.94.0` is a version string this session has not packaged before (`0.94.0-dev` to
  `-dev5` were throwaways). Still, if a verification build reports an API as missing, check `~/.cargo/registry/src/` for a
  stale extraction, as P5 found. Say whether you had to.

## F — what you must not do

- **No tag**, no publish, and no `workflow_dispatch` of anything.
- **Do not touch `rfcs/`, `ROADMAP.md`, or `contracts/owner-attribution-baseline.txt`.**
- **Do not change the Quick Start's `VERSION=`** (A).
- **No Level B claim.**
- **Do not restate a gate's command** in any document (RFC 131 D4).

## G — what to return

`.git-exclude/review-requests/release-0.94.0-preparation-<date>.md`, with:
- the baseline, and the commit the matrix ran on;
- the version sites and the `Cargo.lock` diff;
- the pinned-document arithmetic;
- the upgrade note, as added, with:
  - the measured usage-error text and exit code;
  - both refusals;
  - the `Config` comparison;
  - the API-change method and its output;
- the promoted `CHANGELOG`, with the deleted bullet named, its summary, and **the rendered Release body**;
- `cargo package`'s output, all six crates;
- **the full** `bash scripts/local-matrix.sh`, not `--changed`, with its count and the names of any failing checks;
- per-hunk hashes, beside the package, **against the commit you started from**.
