# `0.91.0` — release preparation

> ## Whose turn it is: **the implementation role's.**
> This is the mechanical half of the release. These stay with the architect
> and the owner: the tag, the publish, the `Security audit` readings, the live
> run (096-B1 stage 14), and **the RFC and ROADMAP records**.

**Dispatched.** 2026-11-15, by the architect.
**What this release is.** Cycle M, authorized 2026-11-14 —
[`cycle-m-and-0.91.0-2026-11-14.md`](cycle-m-and-0.91.0-2026-11-14.md). S1
(with A2), S2, S3 and S4 have all landed on `main`.
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
**Baseline.** The tip of `main` when you start. **Record `git log -1` in the
return, and say which commit the matrix ran on**, if that is not the same one.
**After this lands**, the owner runs 096-B1's stage 14 live run on **this
stage's commit**
([runbook](../096-upstream-oidc-federation/096-b1-stage14-live-run-runbook-2026-11-12.md)).
Then any fixes, then the tag. So no tag-shaped work happens here.

## A — the version bump

**Six sites** in the root `Cargo.toml`: `:21` `version`, and `:34-38`, the five
internal crates. Each `0.90.0` → `0.91.0`. Let cargo refresh `Cargo.lock`,
and confirm only the workspace members moved: the six, plus `xtask`.

The Quick Start's `VERSION=` (`docs/src/getting-started/quick-start.md:27`)
→ `0.91.0`.

## B — the two pinned documents

The rule is `contracts/doc-authority.toml` (`tolerance_minor = 2`), checked
by `scripts/check-doc-authority.py`. It applies to
`docs/development-specification.md`'s banner (`:8`) and
`docs/threat-model.md:4`. **Show the arithmetic** for each: what the pin says,
what `0.91.0` is, and whether it is inside the tolerance. Change a pin only if
the rule requires it, and say so.

## C — the upgrade note

Add `### v0.91.x` to `docs/src/guides/upgrade.md`, above `### v0.90.x`
(`:59`). Measured on `fb60c31`, these facts are true. Each can be checked,
so check each:
- **No migration** runs from `0.90.x`. No file under the store's migrations
  changed since the `0.90.0` tag
  (`git diff --name-only 0.90.0 HEAD`). So **no pre-migration snapshot is
  taken**. Say it in one line.
- **No configuration change.**
- **`sui-id admin rotate-key` now refuses** when the current key comes from
  `SUI_ID_MASTER_KEY`. One line, linking the operator guide's rotation
  section. Restate nothing from it.
- **Rotation is now durable against a power loss, except between its two
  renames.** Link the operator guide's *Recovering a rotation that stopped
  partway*, which says what to do and what not to do. Restate nothing.
- **`sui-id-store`'s public API loses `users::set_last_login`, and
  `signing_keys::insert_with_plaintext` needs `test-support`.** One line,
  for anyone building against the crate.

Keep it short. Lead with *"No migration and no configuration change."*,
because that is what an operator needs first.

## D — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.91.0] — 2026-11-15`, with a fresh empty
`[Unreleased]` above. **The architect resets the date at the cut** if the
tag lands on a later day, as for `0.90.0`.

**Order the subsections as Keep a Changelog does**, which is how every
earlier release here is ordered (`Added`, `Changed`, `Fixed`, `Security`).
The custom `Tests and CI` goes last. So the order is `Changed`, `Fixed`,
`Tests and CI`. Move each section whole, and do not reword a bullet.

**The one-line summary names every subsection**, leading with what an operator
gets:
1. master-key rotation is durable against a power loss (except between its two
   renames), and refuses a key from `SUI_ID_MASTER_KEY` (`Fixed`);
2. then the `Changed` half: the Kani harnesses removed, and `sui-id-store`'s
   two API changes;
3. then `Tests and CI`: a flaky backup test fixed.

**Do not claim the live run.** It has not happened yet, and nothing in this
release's code depends on it.

Render the Release body:
`python3.14 scripts/extract-changelog-section.py --changelog CHANGELOG.md
--version 0.91.0 --repo nabbisen/sui-id --ref main --root .`. Quote the
output; it must exit 0. Confirm `grep -c '^\[0\.' CHANGELOG.md` is still `0`.

## E — packaging

`cargo package -p sui-id-shared --allow-dirty`, with its output verbatim.

## F — what you must not do

- **No tag**, no publish, no `workflow_dispatch` of anything.
- **Do not touch `rfcs/`, `ROADMAP.md`, or `contracts/owner-attribution-baseline.txt`.**
- **No Level B claim.**
- **Do not restate a gate's command** in any document (RFC 131 D4).

## G — what to return

`.git-exclude/review-requests/release-0.91.0-preparation-2026-11-15.md`, with:
- the baseline, and the commit the matrix ran on;
- the version sites and the `Cargo.lock` diff;
- the pinned-document arithmetic;
- the upgrade note, as added;
- the promoted `CHANGELOG`, its summary, and **the rendered Release body**;
- `cargo package`'s output;
- `bash scripts/local-matrix.sh` (**full**, not `--changed`), with its count,
  and the names of any failing checks;
- per-hunk hashes, beside the package, **against the commit you started
  from**.
