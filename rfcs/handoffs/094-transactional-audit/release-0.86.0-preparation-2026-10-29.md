# `0.86.0` — release preparation

> ## Whose turn it is: **the implementation role's, for the mechanical half.**
> Cycle H's four items are landed. The cut and the publish stay with
> `@nabbisen` and the architect; everything below is the preparation
> that does not.

**Dispatched.** 2026-10-29 JST, by the architect.
**Checklist.** [`docs/src/contributing/release-process.md`](../../../docs/src/contributing/release-process.md)
§*Pre-publish checklist* — items 1, 2 and 3 are yours. **Step 0 is done
and is not**: the `Security audit` lane was read on `main` before this
was written — `workflow_dispatch` `40793702457` on `b16e2f8`, green.
Items 4 and 5, Level B, the tag and the publish are not yours either;
§4.
**Contents.** RFC 094 M2b wave 3, RFC 140 stage 1, the local-matrix
script, and RFC 141's publish workflow.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 0 — what this release is

`0.86.0` is **cycle H**: the work deferred out of `0.85.0`, plus the
machinery that keeps this programme's own evidence honest.

**Two of the four reach an operator** and have `CHANGELOG` entries
already written, by the rounds that did the work. The other two — the
local-matrix script and the publish workflow — are developer and
release tooling and deliberately have none.

**`0.86.0` does not publish through the new workflow.** RFC 141 §7
settled `0.87.0` for that. This cut uses the existing path.

**It does not close M2b.** Only the authority switch remains, and that
is `0.87.0`.

## 1 — the version bump

**Six sites**, all in the root `Cargo.toml`, measured today:

```
:20  version = "0.85.0"                          → 0.86.0
:33  sui-id-core   = { version = "0.85.0", … }   → 0.86.0
:34  sui-id-i18n   = { version = "0.85.0", … }   → 0.86.0
:35  sui-id-shared = { version = "0.85.0", … }   → 0.86.0
:36  sui-id-store  = { version = "0.85.0", … }   → 0.86.0
:37  sui-id-web    = { version = "0.85.0", … }   → 0.86.0
```

Refresh `Cargo.lock` **by letting cargo do it**. Confirm the six
workspace entries moved and **nothing else in the lock changed**.

### Must **not** be bumped

- `ROADMAP.md:926` — the cycle G release record. **Do not touch
  `ROADMAP.md` at all**; the cycle H row above it is already written.
- `contracts/owner-attribution-baseline.txt` — never hand-edited.
- Everything under `rfcs/handoffs/` — records.

### Must be bumped — read each sentence, including the arithmetic

1. `docs/development-specification.md:8-24`.
   - `Stale as of v0.85.0 (2026-10-27)` → `v0.86.0` and the cut's date.
   - `eight minor versions past it` → **nine** (v0.77.0 → v0.86.0).
   - `six more than … tolerance allows` → **seven**
     (`tolerance_minor = 2`).
   - `unverified against v0.85.0` → `v0.86.0`.
   - **Widen the existing §15 divergence clause**, do not add a fourth:
     it already names M2b stages 1–3 and wave 4; wave 3 (federation
     provider configuration) now joins them.
2. `docs/threat-model.md:4` — `the workspace is now **v0.85.0**` →
   `v0.86.0`. The `Stale as of 2026-09-10` line above it is the
   document's own staleness date — leave it.

## 2 — the `CHANGELOG` promotion

`[Unreleased]` → `## [0.86.0] — <the cut's date>`, with a fresh empty
`[Unreleased]` above it.

**Two subsections exist and are already written** — `Added` (RFC 140
stage 1) and `Changed` (wave 3). **Do not rewrite, reorder or merge
them.**

**Write the one-line summary under the version heading, and it must
name both subsections.** Not just the milestone. `0.84.0`'s summary
named only its headline scope while its `Security` section held two
things the headline did not imply, and that is a mistake this project
has already paid for once.

**There is no `Security`, `Removed` or `Fixed` section, and that is
correct — do not invent one.** Measured: nothing in this release is a
breaking change *relative to `0.85.0`*. The signatures wave 3 added
were changed again by RFC 140 stage 1 within the same unreleased
cycle, so no published surface moved.

**No link-reference block** — zero lines matching `^\[0\.`.

## 3 — packaging verification

```
cargo package -p sui-id-shared --allow-dirty
```

**Report the output verbatim**, including any warning.

## 4 — what you must not do

- **No tag. No publish. No `workflow_dispatch`. No Level B claim.**
- **Do not run the new publish workflow**, and do not configure
  anything on crates.io. `0.86.0` publishes by the existing path and
  the six Trusted Publishing configurations are `@nabbisen`'s.
- **No `Security audit` judgement.** Running `cargo audit` locally is
  welcome — **with the lane's own flags, read from
  `.github/workflows/audit.yml`'s `Run cargo audit` step**, not from
  here and not from memory.

## 5 — your local matrix

**Run `bash scripts/local-matrix.sh`.** That is now the whole
instruction — it derives its own set and currently reports **47**
checks. Do not enumerate anything by hand; that habit is what the
script exists to end.

## 6 — what to return

- The diff of the six version sites, and confirmation the lock changed
  only there.
- What you changed in each staleness marker, including both numbers
  and the widened divergence clause, and what you left alone.
- The summary line you wrote, and confirmation it names both
  subsections (§2).
- The `cargo package` output, verbatim.
- `scripts/local-matrix.sh`'s output and the count it printed.
- `python3.14 scripts/hunk-hashes.py --baseline <the hash you recorded>`.

**No `CHANGELOG` entry for this round itself.**

Leave the working tree uncommitted. One open dispatch at a time.
