# `0.88.0` — release preparation

> ## Whose turn it is: **the implementation role's**, for the mechanical half.
> The tag, the `Security audit` readings, the publish dispatch, **and the
> RFC and ROADMAP records** stay with the architect.

**Dispatched.** 2026-11-08, by the architect.
**What this release is.** *"M2b closes, and RFC 142 closes"*, which was
settled for `0.88.0` on 2026-10-29. RFC 142's closure was approved by
`@nabbisen` on 2026-11-08, and M2b's evidence was accepted the same day.
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
**Baseline.** The tip of `main` when you start — `git log -1`, record it.
**Step 0.** `Security audit` dispatched on `main` at `8621d08` before this was
written: **success**, run `42467163262`. Nothing to carry into the release.

## 0 — what it contains

Everything since `0.87.2`, five `CHANGELOG` entries, all already written:

- **`Added`**:
  - the automatic pre-migration snapshot (RFC 142 stage 5);
  - `sui-id preview-upgrade` (RFC 142 stage 4).
- **`Changed`**: a failed best-effort audit write now leaves an error line
  (cycle J stage 10, R1).
- **`Fixed`**:
  - expired password-reset tokens are now purged;
  - the subcommand reference table matches the commands;
  - `--help`'s `rotate-key` flags.

Plus developer tooling that has no entry, and needs none: `--changed`, the
write-site gate, and the manifest check.

**No schema change.** `git diff --name-only 0.87.2..HEAD --
crates/sui-id-store/src/migrations/` is empty, so there is no migration note
for the corpus. **But read §3's upgrade note**: this is the first release
whose start can *take a snapshot* or *refuse* on an older database.

## 1 — the version bump

**Six sites**, all in the root `Cargo.toml`, measured today:

```
:21  version = "0.87.2"                          → 0.88.0
:34  sui-id-core   = { version = "0.87.2", … }   → 0.88.0
:35  sui-id-i18n   = { version = "0.87.2", … }   → 0.88.0
:36  sui-id-shared = { version = "0.87.2", … }   → 0.88.0
:37  sui-id-store  = { version = "0.87.2", … }   → 0.88.0
:38  sui-id-web    = { version = "0.87.2", … }   → 0.88.0
```

Refresh `Cargo.lock` by letting cargo do it, and confirm only the workspace
members moved: the six, plus `xtask`, which inherits the workspace version,
as in `0.87.2`.

## 2 — the two pinned documents: **the arithmetic changes this time**

This is a **minor** release, unlike `0.87.1` and `0.87.2`.
`check-doc-authority.py` computes the lag from the minor component
(`lag = current[1] - claim[1]`), and `contracts/doc-authority.toml` sets
`tolerance_minor = 2`.

1. **`docs/development-specification.md`**, the banner:
   - `Stale as of v0.87.2 (2026-11-07)` → `v0.88.0` and the cut's date;
   - `ten minor versions past it` → **eleven** (v0.77.0 → v0.88.0);
   - `eight more than` → **nine** (11 − 2).

   Read the rest of the banner's sentences too. If any other number in it
   is derived from the version, it moves as well. Say which.
2. **`docs/threat-model.md:4`**: `the workspace is now **v0.87.2**` →
   `**v0.88.0**`. Leave its own `Stale as of` line alone.

## 3 — two operator notes this release owes

1. **`docs/src/guides/upgrade.md`**, a new `### v0.88.x` section at the top
   of *Version-specific notes*, in the existing sections' style:
   - Upgrading **to** `0.88.0` from a release with pending migrations takes
     the automatic snapshot first. Say where it lands, how big, and that
     **a start can now refuse with exit 1** when free space is short, with
     nothing changed. `--no-backup` declines it.
   - `sui-id preview-upgrade` exists, and is worth running first.
   - From `0.87.2` itself **no migration runs**, so no snapshot is taken.
     Say so, so nobody looks for one.

   Link to the guide's own sections. Do not restate them.
2. **`docs/src/guides/deployment.md`, *Exit codes*:** the table still says
   *"`1`: Everything else"*, which stays true. Add one sentence after it: a
   refused pre-migration backup is exit `1`, so `Restart=on-failure` retries
   it until systemd's start limit. That is harmless, because the check writes
   nothing, but the cause is the first line on stderr and in the log. **Do
   not add `1` to `RestartPreventExitStatus`**: it covers every other
   failure too.

## 4 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.88.0] — <the cut's date>`, with a fresh empty
`[Unreleased]` above it. **Three subsections exist, `Added`, `Changed` and
`Fixed`.** Do not merge, reorder or rewrite them.

**The one-line summary must name every subsection.** Lead with what makes
this release what it is: upgrading is now **previewable and backed up
automatically**. Then the audit-write change, then the fixes, in one sentence
or two. **Do not** name the internal milestones (M2b, RFC numbers) in the
summary. Operators read it; they are in the records.

**No link-reference block**: confirm `grep -c '^\[0\.' CHANGELOG.md` is
still `0`.

## 5 — packaging verification

```
cargo package -p sui-id-shared --allow-dirty
```

**Report the output verbatim**, including any warning.

## 6 — what you must not do

- **No tag**, no publish, no `workflow_dispatch` of anything, `audit.yml`
  included.
- **Do not touch `rfcs/`, `ROADMAP.md` or `contracts/owner-attribution-baseline.txt`.**
  Moving RFC 094 and RFC 142 to `done/`, their closure fields, the inbound
  links, and the ROADMAP release record are the architect's, in the commit
  that is tagged.
- **No Level B claim.**
- **Do not restate a gate's command** in any document (RFC 131 D4).

## 7 — what to return

A review request at
`.git-exclude/review-requests/release-0.88.0-preparation-2026-11-08.md`:
- the baseline;
- the six sites and the `Cargo.lock` diff;
- the two pinned documents, with the arithmetic shown;
- the two operator notes' text;
- the promoted `CHANGELOG` and its summary;
- `cargo package`'s verbatim output;
- `bash scripts/local-matrix.sh` (**full**, not `--changed`, since this is a
  release preparation), output and count from the real tree;
- per-hunk hashes.

Leave the working tree uncommitted.
