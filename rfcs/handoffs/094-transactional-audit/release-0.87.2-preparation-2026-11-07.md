# `0.87.2` — release preparation

> ## Whose turn it is: **the implementation role's**, for the mechanical half.
> The tag, the `Security audit` readings and the publish dispatch stay
> with the architect.

**Dispatched.** 2026-11-07, by the architect.
**Authorized.** `@nabbisen`, 2026-11-07: *"Authorized."*
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
**Baseline.** The tip of `main` when you start — `git log -1`, record it.
**Step 0.** `Security audit` dispatched on `main` at `8df3093` before this
was written: **success**, run `42356328349`. Nothing new to carry into the
release.

**This one should not wait.** It carries a `Security` entry, and its fix is
already public on `main`. Do it before cycle J stage 7d resumes.

## 0 — what this release is

`0.87.2` tagged at `main`'s tip ships **everything since `0.87.1`**. Three
`CHANGELOG` entries, all already written:

- **`Security`**: sign-in follows only a same-site return path (`8df3093`).
  This is why the release exists.
- **`Changed`**: master-key rotation records its re-seal in the same
  transaction, as a new event `admin.master_key.resealed` (cycle J stage 4).
- **`Changed`**: a rotation that stops partway is diagnosable from the
  audit log (cycle J stage 5).

**Why not a narrower release.** The same reason as `0.87.1`.
`scripts/check-publish-preconditions.py` requires the tagged commit to be an
ancestor of `main`, so a cherry-pick onto `0.87.1` would fail a deliberate
control.

**No schema change.** `git diff --name-only 0.87.1..HEAD --
crates/sui-id-store/src/migrations/` is empty. There is no migration note,
and G22's corpus is unaffected.

## 1 — the version bump

**Six sites**, all in the root `Cargo.toml`, measured today:

```
:21  version = "0.87.1"                          → 0.87.2
:34  sui-id-core   = { version = "0.87.1", … }   → 0.87.2
:35  sui-id-i18n   = { version = "0.87.1", … }   → 0.87.2
:36  sui-id-shared = { version = "0.87.1", … }   → 0.87.2
:37  sui-id-store  = { version = "0.87.1", … }   → 0.87.2
:38  sui-id-web    = { version = "0.87.1", … }   → 0.87.2
```

Refresh `Cargo.lock` by letting cargo do it, and confirm **only** those six
entries moved.

### The two pinned documents, and the arithmetic does not change

1. **`docs/development-specification.md:8`**: change **only** the banner's
   version and date, `Stale as of v0.87.1 (2026-11-03)` → `v0.87.2` and the
   cut's date. Leave the minor-version arithmetic alone, for the reason the
   `0.87.1` preparation gave: `check-doc-authority.py` computes its lag from
   the **minor** component alone, so a patch cannot move it.
2. **`docs/threat-model.md:4`**: `the workspace is now **v0.87.1**` →
   `v0.87.2`. Leave its own `Stale as of` line alone.

### Seven sentences that name the wrong release

Stages 4 and 5 were written expecting to ship in `0.88.0`. They ship in
`0.87.2`. **Change every one of these to `v0.87.2`**, and nothing else in
those sentences:

| file | line | text today |
|---|---|---|
| `docs/src/guides/operators.md` | 173 | `since v0.88.0` |
| `docs/src/guides/operators.md` | 202 | `rotated before v0.88.0` |
| `docs/src/guides/operators.md` | 207 | `the upgrade to v0.88.0` |
| `docs/src/guides/operators.md` | 210 | `a pre-v0.88.0` |
| `docs/src/reference/audit-events.md` | 87 | `Since v0.88.0` |
| `docs/src/reference/audit-events.md` | 88 | `Before v0.88.0` and `from before v0.88.0` (two) |

**Then prove it:** `git grep -n "0\.88\.0" -- . ':!rfcs' ':!ROADMAP.md'
':!CHANGELOG.md' ':!contracts/owner-attribution-baseline.txt'` must return
nothing. Quote its (empty) output. The exclusions are records and plans,
which correctly still say `0.88.0` about the next minor release.

### Must **not** be touched

- `ROADMAP.md`: the release record is the architect's to add after the cut.
- `contracts/owner-attribution-baseline.txt`: never hand-edited.
- Everything under `rfcs/handoffs/`: these are records.
- **The `Security` entry's wording.** It is deliberately neutral. Do not
  expand it.

## 2 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.87.2] — <the cut's date>`, with a fresh empty
`[Unreleased]` above it.

**Two subsections exist, `Changed` and `Security`.** Do not merge, reorder or
rewrite them.

**The one-line summary must name all three entries.** Lead with the
`Security` entry, because it is why the release exists. Name it the way the
entry does: *"sign-in follows only a same-site return path"*. Then name the
two `Changed` items in the same sentence. A summary that names only the
security fix hides that an audit event was added. One that leads with the
audit work buries the reason to upgrade.

**No link-reference block**: confirm `grep -c '^\[0\.' CHANGELOG.md` is
still `0`.

## 3 — packaging verification

```
cargo package -p sui-id-shared --allow-dirty
```

**Report the output verbatim**, including any warning.

## 4 — what you must not do

- **No tag**, no publish, no `workflow_dispatch` of `publish.yml`.
- **No `gh workflow run` of anything**, `audit.yml` included. Both
  `Security audit` readings are the architect's.
- **No Level B claim.**
- **Do not restate a gate's command** in any document (RFC 131 D4).
- **Run the full local matrix**, `bash scripts/local-matrix.sh`, not
  `--changed`, which does not exist yet. A release preparation runs the
  full matrix regardless.

## 5 — what to return

- The usual package, against the baseline you recorded.
- The six `Cargo.toml` sites and the `Cargo.lock` diff, with confirmation
  that nothing else moved.
- The seven sentences, and the empty `git grep` output.
- The promoted `CHANGELOG` with its one-line summary (§2).
- The two pinned documents, and a statement that the arithmetic was left
  alone.
- `cargo package`'s verbatim output.
- `bash scripts/local-matrix.sh`'s output and count, **from the real tree**.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.

Return it as a review request in `.git-exclude/review-requests/`. Leave
the working tree uncommitted.
