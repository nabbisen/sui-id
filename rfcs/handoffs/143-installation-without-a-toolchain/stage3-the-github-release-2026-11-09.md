# RFC 143 stage 3 — the GitHub Release

> ## Whose turn it is: **the implementation role's.**
> Cycle K stage 3 —
> [`cycle-k-and-0.89.0-2026-11-08.md`](cycle-k-and-0.89.0-2026-11-08.md) §2.
> **This document is the dispatch.**

**Dispatched.** 2026-11-09, by the architect.
**Governing RFC.** [RFC 143](../../done/143-installation-without-a-toolchain.md),
**Accepted**, D5.
**Baseline.** The tip of `main` when you start (`git log -1`; record it).
Stage 2 is complete: rehearsal `42541537200` on `bd8be11` met all eight criteria.
**Read first:** `.github/workflows/release-artifacts.yml` and `publish.yml` as they
stand. Stage 2's review record,
`.git-exclude/reviewed/cycle-k-stage2-build-check-and-attest-2026-11-08.md`, says
why each part is shaped as it is.

## 0 — the shape: rehearse everything except the one outward call

Creating a Release cannot be rehearsed without creating one. **So everything
before that call goes in `release-artifacts.yml`, which every rehearsal runs.**
That means the notes, and the operator's own verification of the files to be
attached. `publish.yml` gains one small job that only downloads and creates.
What has not been rehearsed is then a single `gh release create`.

You cannot run workflows. The architect commits and rehearses (§E).

## A — the release notes: `scripts/extract-changelog-section.py`

**D5:** the body is *"the `CHANGELOG` section for that version, verbatim"*.

**Measured 2026-11-09:** that cannot be literally verbatim, because the
`CHANGELOG`'s links are repo-relative (`rfcs/…` ×10, `docs/…` ×5, and only two
absolute links). GitHub's renderer leaves a relative `href` as written:

```
gh api markdown -f mode=gfm -f context=nabbisen/sui-id -f text='[x](docs/src/guides/operators.md)'
→ <a href="docs/src/guides/operators.md">x</a>
```

On a page at `/nabbisen/sui-id/releases/tag/0.89.0` that resolves to
`…/releases/tag/docs/src/…`, a 404. **The architect's reading of D5: the words
are verbatim, and the link targets are made to resolve.** Each repo-relative
target becomes `https://github.com/<repo>/blob/<tag>/<path>`, **pinned to the
tag**. Then a link to `rfcs/accepted/…` still works after that RFC moves to
`done/`, and a reader sees the document as it was at that release. Nothing else
in the text changes.

**The script:** `--changelog <path> --version <X.Y.Z | Unreleased> --repo
<owner/name> --ref <git ref>` writes the body to stdout.
- **The section** starts at the line matching `^## \[<version>\]( — .*)?$`, with
  the version **escaped**, and ends before the next line beginning `## `. **The
  heading line is excluded**, since the Release title carries the version. Trim
  leading and trailing blank lines.
- **Refuse** (exit 1, one line on stderr naming the version and the file) when
  the section is **missing** or **empty**. A Release with an empty body is
  worse than no Release.
- **Rewrite** only inline link targets `](target)` where the target has no
  scheme (`https:`, `mailto:` …), does not start with `#` or `/`, and does not
  contain `..`. **Never inside code spans or fenced blocks**: the
  `CHANGELOG` has fenced `toml` blocks (from `:1574`). If a target contains `..`,
  refuse rather than guess what it points at.

**Tests**, `scripts/tests/test_extract_changelog_section.py`, in the existing
script-test style:
- extracts exactly one section, stops at the next `## `, and drops the heading;
- **the version is a whole match**: `0.8.1` does not match `## [0.8.10]`, and
  neither matches `0.8.1-rc.1`;
- `Unreleased` extracts `## [Unreleased]`, and a version never matches it;
- missing → exit 1; present but empty → exit 1;
- `docs/x.md` and `rfcs/done/y.md` are rewritten to the blob URL at the given ref;
  `https://…`, `#anchor`, `/abs` and `mailto:` are untouched; a target with
  `..` → exit 1;
- a `](docs/x.md)` **inside a fenced block and inside a code span** is
  untouched;
- **against the real `CHANGELOG.md`**: `--version 0.88.0` returns a section
  whose first line is the bold summary (`**Upgrading is now previewable…`),
  and every text line equals the file's, apart from link targets.

**Mutation:** drop the code-fence exclusion and its test goes red; make the
version match a prefix and the `0.8.1`/`0.8.10` test goes red. Quote both.

Register the self-test in `contracts/workflow-template.toml` and regenerate
`ci.yml` (`scripts/generate-ci-workflow.py`), exactly as stage 2's
`test_check_openssl_src_freshness` was registered.

## B — `release-artifacts.yml` prepares and verifies what a Release would attach

In the job that already writes `SHA256SUMS` and attests:
1. **Render the notes.** The version is `github.ref_name` when `github.ref_type
   == tag`, and `Unreleased` otherwise (a rehearsal). `--ref` is
   `github.ref_name` in both cases, and `--repo` is `github.repository`. Write
   them to `release-notes.md`, add them to the uploaded `release-artifacts`
   bundle, and **print them into the step summary**, so each rehearsal shows the
   notes it would publish.
2. **Verify, after attesting, exactly as an operator will**: `sha256sum -c
   SHA256SUMS`, then for each binary and `SHA256SUMS`, `gh attestation verify
   <file> --repo "$GITHUB_REPOSITORY" --source-ref "$GITHUB_REF"`. **This checks
   the attestation the step just made, filtered to this ref**, so a rehearsal
   build of `main` can never satisfy a tag's check. **Measure** which permission
   `gh attestation verify` needs from `GITHUB_TOKEN` (for example
   `attestations: read`), add only that, to this job, and quote the source.

## C — `publish.yml`: one job that downloads and creates

A new job, `github-release`, **after** `publish` (`needs: [verify,
build-artifacts, publish]`), in the `release` environment, with `permissions:
contents: write` and nothing else unless C2 needs more:
1. Download `release-artifacts` with `actions/download-artifact` at **the SHA
   already pinned** in `release-artifacts.yml`. That adds no new action and
   touches no `[actions]` row. Then run `sha256sum -c SHA256SUMS` again: what is
   attached is what was attested.
2. **Refuse if a Release for the tag already exists** (`gh release view
   "$TAG"` succeeds). Never overwrite.
3. `gh release create "$TAG" --verify-tag --title "sui-id $TAG" --notes-file
   release-notes.md sui-id-x86_64-unknown-linux-musl
   sui-id-aarch64-unknown-linux-musl SHA256SUMS`. **Not** a draft, and **not**
   `--generate-notes`. `--verify-tag` aborts if the tag does not exist, so this
   job can never create a tag.
4. **Confirm the result**: the Release lists exactly those three assets.
   Compare each asset's digest with `SHA256SUMS`. **Measure** whether the
   release-asset API's `digest` field is available, and say which method you
   used.
5. The tag input goes through `env:`, as in `verify`.

**Failure reporting** (stage 3's own requirement from the cycle document):
- the existing partial-publish report says, in addition, **"no GitHub Release
  was created"**, since `github-release` cannot run after a failed `publish`;
- a failure in `github-release` itself reports, in operator words: **all six
  crates are published; the GitHub Release was not created**. Then the
  recovery. **Recommended:** "Re-run failed jobs" on the same run, which
  re-runs only this job, with the same run context and the same uploaded
  artifacts. **Cite GitHub's documentation** for both halves of that claim
  (which jobs re-run, and whether a re-run can download an earlier
  attempt's artifacts) before writing it into the message. If either half is
  false, give the manual `gh release create` command instead.

## D — documents and records

- `release-process.md`: the run now ends with a GitHub Release. Add the
  `github-release` job to *What runs, in order*, with what it refuses and how
  to recover. Step 5 (after publish) gains *"the Release page shows three assets
  and the notes"*.
- `publish.yml`'s header comment: the job list.
- **No `CHANGELOG` entry in this stage.** The operator-visible change, binaries
  to download, is written once in stage 4 with the Quick Start that explains
  it.

## E — constraints and what to return

- **No tag, no dispatch, no Release.** The architect commits, then rehearses
  `release-artifacts.yml` on `main`, and judges the run against the criteria
  you set.
- `bash scripts/local-matrix.sh --changed`, quoting its scope line and the
  **names** of any failing checks, not only the count. Also A3.4 and its
  fixtures, and both script self-tests run directly.
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/cycle-k-stage3-the-github-release-2026-11-09.md`,
with:
- the baseline;
- A's tests and both mutations;
- B and C's diffs, with the permissions per job and the two measurements (C2's
  digest field; B2's permission);
- the re-run citation;
- **the criteria the rehearsal must meet**, set before it runs. They include
  at least these: the notes appear in the summary and equal the `[Unreleased]`
  section, with links rewritten to blob URLs at `main`; the operator
  verification step passes; and nothing is published.
