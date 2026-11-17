# `0.92.0` — release preparation

> ## Landed 2026-11-17, with two architect fold-ins —
> `.git-exclude/reviewed/release-0.92.0-preparation-2026-11-17.md`. The threat
> model's banner no longer names the workspace's version, which it had
> stated wrongly since `0.91.0`; and two defects N2b fixed reach the
> CHANGELOG. **Next:** the owner's decision on the review's F3 (a regular
> user's own pages show the console navigation): if it is fixed in `0.92.0`,
> stage 14 runs on the commit that fixes it; if not, on this one. Then the cut.
>
> ## Whose turn it was: **the implementation role's.**
> This is the mechanical half of the release. These stay with the architect
> and the owner: the tag, the publish, the `Security audit` readings, the live
> run (096-B1 stage 14), the Quick Start's `VERSION=` (at the cut), and **the
> RFC and ROADMAP records**.

**Dispatched.** 2026-11-17, by the architect.
**What this release is.** Cycle N, as re-scoped 2026-11-16 —
[`cycle-n-and-0.92.0-2026-11-15.md`](cycle-n-and-0.92.0-2026-11-15.md). All
three of its builds have landed on `main`: RFC 100 stage 0 (`ddcf4b0`), RFC
147 (`54b1be7`), and RFC 144 in two halves (`88e92a8`, `43dcc9d`).
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
**Baseline.** The tip of `main` when you start. **Record `git log -1` in the
return, and say which commit the matrix ran on**, if that is not the same one.
**After this lands**, the owner runs 096-B1's stage 14 live run on **this
stage's commit**
([runbook](../096-upstream-oidc-federation/096-b1-stage14-live-run-runbook-2026-11-12.md)).
**The tag waits for that run's record**, as cycle N decided. So no tag-shaped
work happens here.

## A — the version bump

**Six sites** in the root `Cargo.toml`: `:21` `version`, and `:34-38`, the five
internal crates. Each `0.91.1` → `0.92.0`. Let cargo refresh `Cargo.lock`,
and confirm only the workspace members moved: the six, plus `xtask`.

**Do not change the Quick Start's `VERSION=`**
(`docs/src/getting-started/quick-start.md:27`, `0.91.1`). It tells a reader
which release to download, and `0.92.0` cannot be downloaded until the
publish. The architect changes it in the cut (`0.91.0`'s review, its one
fold-in).

## B — the two pinned documents

The rule is `contracts/doc-authority.toml` (`tolerance_minor = 2`), checked
by `scripts/check-doc-authority.py`. It applies to
`docs/development-specification.md`'s banner (`:8`, *"Stale as of
v0.90.0"*) and `docs/threat-model.md:4` (*"now v0.90.0"*). **Show the
arithmetic** for each, from the checker's own comparison (quote the line of
the script that compares): what the pin says, what `0.92.0` is, and whether
it is inside the tolerance. Change a pin only if the rule requires it, and
say so.

## C — the upgrade note

Add `### v0.92.x` to `docs/src/guides/upgrade.md`, above `### v0.91.x`
(`:59`). Measured on `43dcc9d`. Each fact can be checked, so check each:
- **No migration** runs from `0.91.x`: no file under the store's migrations
  changed since the `0.91.1` tag (`git diff --name-only 0.91.1 HEAD`). So
  **no pre-migration snapshot is taken**. One line.
- **No configuration change.** `sui-id.example.toml` gained comments naming
  keys that already existed; no key was added, renamed or removed. Check
  that against `Config`'s fields at both commits, and say so.
- **The console looks different**: a new palette, larger text that follows
  the browser's font size, the account menu (with sign-out) on every page.
  One line, so an operator is not surprised; link nothing.
- **For anyone building against the crates**, one line each:
  - `sui-id-web`: `Shell` takes `nav: Chrome` in place of `show_nav` and
    `admin_username`;
  - `sui-id-core`: `CreateClientSpec` has a required `consent_policy`;
  - `sui-id`: `CurrentAdmin` and `CurrentAdminOrAuditor` carry the username.

Lead with *"No migration and no configuration change."*, because that is
what an operator needs first. Keep it short.

## D — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.92.0] — 2026-11-17`, with a fresh empty
`[Unreleased]` above. **The architect resets the date at the cut** if the
tag lands on a later day.

The subsections are already in Keep a Changelog's order: `Changed`,
`Fixed`, then the custom `Tests and CI`. **Move each whole, and do not reword
a bullet.**

**The one-line summary names every subsection**, leading with what an
operator gets:
1. the console is easier to read and to find things in: AAA contrast in a
   new palette, text that follows the browser's font size, 44px controls,
   the account menu on every page, and labelled main actions (`Changed`);
2. then `Fixed`: a missing `sui-id.toml` now says how to create one;
   destructive buttons' labels in dark mode, and disabled fields, were below
   contrast;
3. then `Tests and CI`: a crash-injection harness for master-key rotation.

**Do not claim the live run.** It has not happened yet.

Render the Release body:
`python3.14 scripts/extract-changelog-section.py --changelog CHANGELOG.md
--version 0.92.0 --repo nabbisen/sui-id --ref main --root .`. Quote the
output; it must exit 0. Confirm `grep -c '^\[0\.' CHANGELOG.md` is still `0`.

## E — packaging

`cargo package -p sui-id-shared --allow-dirty`, with its output verbatim.

## F — what you must not do

- **No tag**, no publish, no `workflow_dispatch` of anything.
- **Do not touch `rfcs/`, `ROADMAP.md`, or `contracts/owner-attribution-baseline.txt`.**
- **Do not change the Quick Start's `VERSION=`** (A).
- **No Level B claim.**
- **Do not restate a gate's command** in any document (RFC 131 D4).

## G — what to return

`.git-exclude/review-requests/release-0.92.0-preparation-<date>.md`, with:
- the baseline, and the commit the matrix ran on;
- the version sites and the `Cargo.lock` diff;
- the pinned-document arithmetic;
- the upgrade note, as added, and the `Config` comparison behind *"no
  configuration change"*;
- the promoted `CHANGELOG`, its summary, and **the rendered Release body**;
- `cargo package`'s output;
- `bash scripts/local-matrix.sh` (**full**, not `--changed`), with its count,
  and the names of any failing checks;
- per-hunk hashes, beside the package, **against the commit you started
  from**.
