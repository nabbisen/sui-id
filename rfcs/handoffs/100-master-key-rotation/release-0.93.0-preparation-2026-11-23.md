# `0.93.0` — release preparation

> ## Whose turn it is: **the implementation role's.**
> This is the mechanical half of the release. These stay with the
> architect and the owner:
> - the tag, the publish, and the `Security audit` readings;
> - the Quick Start's `VERSION=`, at the cut;
> - **the RFC and ROADMAP records**: RFC 148 and RFC 107 to `done/`, and
>   RFC 096, now accepted in every phase.

**Dispatched.** 2026-11-23, by the architect.

**What this release is.** Cycle O
([`cycle-o-and-0.93.0-2026-11-18.md`](cycle-o-and-0.93.0-2026-11-18.md)).
Every build has landed on `main`:

| Build | What | Commit(s) |
|---|---|---|
| O1 | RFC 100 stage 1, the startup key check | `92b3d01` |
| O2 | RFC 148, the command system | `ff1a9f5`, `e8d5430` and `0b0f331` |
| O3 | RFC 100 stage 2, preconditions and `--check` | `79ed9c5` |
| O4 | RFC 100 stage 3, the journal | `445a97e` |
| O5 | RFC 100 stage 4, startup recovery | `643aa47` |
| O6 | what the Google run found | `67f1140` |
| O7 | an honest unlink page, and RFC 103 T10 restored | `f9b81ad` |

M4-B closed 2026-11-23 (`d03438b`). There is no live run to wait for: it
ran.

**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)

**Baseline.** The tip of `main` when you start. **Record `git log -1` in the
return, and say which commit the matrix ran on**, if that is not the same
one.

## A — the version bump

**Six sites** in the root `Cargo.toml`: `version` (`:21`), and the five
internal crates (`:34-38`). Each goes from `0.92.0` to `0.93.0`. Let cargo
refresh `Cargo.lock`, and confirm only the workspace members moved: the
six, plus `xtask`.

**Do not change the Quick Start's `VERSION=`**
(`docs/src/getting-started/quick-start.md`). `0.93.0` cannot be
downloaded until the publish, so the architect changes it at the cut.

## B — the two pinned documents

Apply the rule in `contracts/doc-authority.toml`, as checked by
`scripts/check-doc-authority.py`, to two pins:
- `docs/development-specification.md`'s banner;
- `docs/threat-model.md`'s.

**Show the arithmetic** for each: quote the line of the script that
compares, then give what the pin says, what `0.93.0` is, and whether it
falls inside the tolerance. Change a pin only if the rule requires it,
and say so.

## C — the upgrade note

Add `### v0.93.x` to `docs/src/guides/upgrade.md`, above `### v0.92.x`
(`:59`). Check each fact before writing it:

- **One migration runs, `0052_master_key_sentinel`**: `git diff --name-only
  0.92.0 HEAD` under the store's migrations shows it. So **a pre-migration
  snapshot is taken**. Say where, measured from the code that takes it,
  not from memory.
- **The master key is now checked at every start.** A key that does not
  open the existing database is refused, and so is a missing key file over
  existing data, instead of a new key being generated. The first start
  after upgrading writes the sentinel. An operator who has kept the right
  key file notices nothing. One who hasn't is refused at start, which is
  the point. Say what the refusal says, quoting the real text.
- **The command line.**
  - Commands have new, grouped names (RFC 148), and every old form still
    works and prints a notice. Name two or three common old → new pairs
    from the generated `docs/src/reference/cli.md`.
  - **A usage error now exits 2, not 1.** A script that tells "usage
    error" from "failure" by the exit code must change. Measure it on the
    binary, both codes, and say it plainly.
  - `sui-id init` exists.
- **No configuration change.** Check this against `Config`'s fields at
  `0.92.0` and at `HEAD`, and say so. One case to word carefully: a
  federation provider whose DNS answer carried more than 8 addresses
  (Google's does) **now passes** *Check and enable*.
- **Federation.**
  - **A recovery link is now refused for a user linked to a federation
    provider, and password sign-in refuses such a user.** One line. Then
    the CHANGELOG *Security* entry's action, said once: an earlier
    `user.recovery_link.issued` for a federated user may have left a local
    password.
- **For anyone building against the crates**, one line per breaking change
  to a published crate's public API since `0.92.0`. **Measure it; don't
  recall it.** Use `cargo semver-checks` if it is available here (say
  which version), or else a diff of `pub` items at the two tags, and say
  which method you used. Expect at least:
  - the `KeyFileOps` trait's new methods;
  - `ConfirmScreenData`'s new fields;
  - `PreflightError` and `CheckAndEnableError` no longer being `Copy`;
  - the new `Strings` fields.

Lead with what an operator needs first: *one migration, a pre-migration
snapshot, and the key is checked at start*. Keep it short.

## D — the `CHANGELOG` promotion

`[Unreleased]` becomes `## [0.93.0] — 2026-11-23`, with a fresh empty
`[Unreleased]` above it. **The architect resets the date at the cut** if
the tag lands on a later day.

The four subsections, `Added`, `Changed`, `Fixed` and `Security`, are
already in Keep a Changelog's order. **Move each whole, and do not reword a
bullet.**

**The one-line summary names every subsection**, leading with what an
operator gets. It must include the `Security` entry (the federated
recovery-link fix), not only the headline scope. Draft it; the architect
reviews it.

Render the Release body:
`python3.14 scripts/extract-changelog-section.py --changelog CHANGELOG.md
--version 0.93.0 --repo nabbisen/sui-id --ref main --root .`. Quote the
output; it must exit 0. Confirm that `grep -c '^\[0\.' CHANGELOG.md` is
still `0`.

## E — packaging

`cargo package -p sui-id-shared --allow-dirty`, with its output verbatim.

## F — what you must not do

- **No tag**, no publish, and no `workflow_dispatch` of anything.
- **Do not touch `rfcs/`, `ROADMAP.md`, or
  `contracts/owner-attribution-baseline.txt`.**
- **Do not change the Quick Start's `VERSION=`** (A).
- **No Level B claim.**
- **Do not restate a gate's command** in any document (RFC 131 D4).

## G — what to return

`.git-exclude/review-requests/release-0.93.0-preparation-<date>.md`, with:
- the baseline, and the commit the matrix ran on;
- the version sites and the `Cargo.lock` diff;
- the pinned-document arithmetic;
- the upgrade note, as added, with:
  - the measured refusal text;
  - both exit codes;
  - the `Config` comparison;
  - the API-change method and its output;
- the promoted `CHANGELOG`, its summary, and **the rendered Release body**;
- `cargo package`'s output;
- **the full** `bash scripts/local-matrix.sh`, not `--changed`, with its
  count and the names of any failing checks;
- per-hunk hashes, beside the package, **against the commit you started
  from**.
