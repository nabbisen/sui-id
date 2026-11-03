# `0.87.1` — release preparation

> ## Whose turn it is: **the implementation role's**, for the mechanical half.
> The tag, the `Security audit` readings and the publish dispatch stay
> with the architect.

**Dispatched.** 2026-11-03, by the architect.
**Authorized.** `@nabbisen`, 2026-11-03: *"0.87.1 is accepted."*
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 0 — what this release is, and it is not only the fix

**It was authorized for the master-key fix, and it carries more than
that.** `0.87.1` tagged at `main`'s tip ships **everything since
`0.87.0`**:

- **the fix it exists for** — the new master key is written to disk
  before the re-seal commits (`1b3bc7d`);
- **L08** — the MFA-required directory sign-in is sealed (`7c42c23`);
- **two documentation corrections** — what a verified audit chain
  proves, and the removal of the guide's false *"no half-rotated
  state"* promise.

**Why not a narrower release.** A cherry-pick of the fix onto `0.87.0`
would not be an ancestor of `main`, and
`scripts/check-publish-preconditions.py` requires the tagged commit to
be one. That is a deliberate control, and routing around it for
tidiness is worse than a slightly wider patch.

**No schema change** — `git diff --name-only 0.87.0..HEAD -- crates/sui-id-store/src/migrations/`
is empty — so no migration note, and `G22`'s corpus is unaffected.

## 1 — the version bump

**Six sites**, all in the root `Cargo.toml`, measured today:

```
:20  version = "0.87.0"                          → 0.87.1
:33  sui-id-core   = { version = "0.87.0", … }   → 0.87.1
:34  sui-id-i18n   = { version = "0.87.0", … }   → 0.87.1
:35  sui-id-shared = { version = "0.87.0", … }   → 0.87.1
:36  sui-id-store  = { version = "0.87.0", … }   → 0.87.1
:37  sui-id-web    = { version = "0.87.0", … }   → 0.87.1
```

All six must equal the tag; the preconditions script checks it. Refresh
`Cargo.lock` by letting cargo do it, and confirm **only** those six
entries moved.

### Must **not** be bumped

- **`ROADMAP.md` — do not touch it.** The release record is the
  architect's to add after the cut.
- `contracts/owner-attribution-baseline.txt` — never hand-edited.
- Everything under `rfcs/handoffs/` — records.

### The two pinned documents — **and the arithmetic does not change**

This is where a patch release differs from a minor one, and where the
`0.87.0` preparation's own instructions would mislead if copied.

1. **`docs/development-specification.md`** — update **only** the
   banner's version and date: `Stale as of v0.87.0 (2026-11-01)` →
   `v0.87.1` and today's. **Leave `ten minor versions` and `eight more
   than … tolerance` exactly as they are.** v0.77.0 → v0.87.1 is still
   **ten** minor versions, and `check-doc-authority.py:326-327` computes
   its lag from the **minor** component alone
   (`lag = current[1] - claim[1]`), so a patch cannot move it.
   **Do not widen the §15 divergence clause** either — nothing in this
   release adds a conversion wave; L08 falls inside cycle I's own
   clause, already there.
2. **`docs/threat-model.md`** — `the workspace is now **v0.87.0**`
   → `v0.87.1`. Its own `Stale as of 2026-09-10` line is that
   document's staleness date; **leave it**.

## 2 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.87.1] — <the cut's date>`, with a fresh empty
`[Unreleased]` above it.

**Two subsections exist, `Changed` and `Fixed`, and both are already
written.** Do not merge, reorder or rewrite them.

**The one-line summary must name both.** A summary that leads with the
data-loss fix and omits that an audit record changed, or vice versa, is
the failure this project has a standing rule about. Lead with the
`Fixed` entry — it is why the release exists and it is the one an
operator must act on — and name the `Changed` work in the same
sentence.

**No link-reference block** — confirm `grep -c '^\[0\.' CHANGELOG.md`
is still `0`.

## 3 — packaging verification

```
cargo package -p sui-id-shared --allow-dirty
```

**Report the output verbatim**, including any warning.

## 4 — what you must not do

- **No tag**, no publish, no `workflow_dispatch` of `publish.yml`.
- **No `gh workflow run` of anything**, `audit.yml` included — both
  `Security audit` readings are the architect's.
- **No Level B claim.**
- **Do not touch `ROADMAP.md`** (§1).
- **Do not restate a gate's command** in any document (RFC 131 D4).

## 5 — what to return

- The usual package, against the baseline you recorded.
- The six `Cargo.toml` sites and the `Cargo.lock` diff, with
  confirmation nothing else moved.
- The promoted `CHANGELOG` with its one-line summary (§2).
- **The two pinned documents, and an explicit statement that the
  arithmetic was left alone and why** — I want to see that reasoning,
  not just the diff.
- `cargo package`'s verbatim output.
- `bash scripts/local-matrix.sh`'s output and count.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.

Return it as a review request in `.git-exclude/review-requests/`. Leave
the working tree uncommitted.
