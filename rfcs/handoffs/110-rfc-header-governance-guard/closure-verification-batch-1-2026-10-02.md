# Independent verification — closure review batch 1 (RFCs 110, 128, 129, 130, 132)

**Date:** 2026-10-02
**Reviewing.** [`rfcs/handoffs/110-rfc-header-governance-guard/closure-review-batch-1-2026-10-02.md`](./closure-review-batch-1-2026-10-02.md)
**Not an implementation package.** No tree change accompanies this. `git status --porcelain` is empty before and after; the plant/remove test below (A3.4 condition 9) left no residue, confirmed.
**Why this exists.** The dispatch pointed at a closure-review document with no "what to build" section — every action it describes (adding Closure metadata, moving RFCs to `done/`) is explicitly the architect's, gated on `@nabbisen`'s approval, not mine to perform. Rather than do nothing, I independently re-measured the checkable claims in it, the way the architect has re-measured mine. Everything below was run against the tree at `fc056df`, which is still `HEAD`.

## What I re-verified, independently, and how

**CI runs, by `gh run view`, not by trusting the cited numbers:**
- `36878881472` on `fc056df`: `headSha` matches, `conclusion: success`, **24/24 jobs**, all eleven Rust lanes (G01–G09b) present and `success`. Matches the claim.
- `36934836767` on `2262861`: `headSha` matches, `conclusion: success`, 24 jobs — **exactly the eleven Rust lanes `skipped`**, the other 13 `success`. `git show --stat 2262861`: touches `ROADMAP.md`, `contracts/owner-attribution-baseline.txt`, and the closure-review handoff file itself — none under the declared Rust-scope paths. Matches the claim exactly, including the job-level detail the closure review didn't spell out.
- Run durations: `36934836767` ran 2m15s (`createdAt` to `updatedAt`); close enough to the claimed "2.2 minutes" to confirm the figure wasn't invented.

**RFC 110 — G11 conditions 14/15.** `scripts/check-rfc-integrity.py` carries both (lines 758/767, 836); `contracts/rfc-policy.toml:43-44` carries `[archive_citations]` with exactly `"025" = ["007"]`. Ran `scripts/ci-gate.sh G11` live on `HEAD`: pass.

**RFC 128 — the archived-RFC-independence claim, checked three ways myself:**
- `git log -S'"N/A"' -- scripts/check-rfc-integrity.py` → exactly one commit, `9381347`, which documents the absence rather than introducing a check. Matches.
- Diffed `9381347`'s change to `check_accepted_metadata`: the only lines added inside the function are a comment block; the sole executable line (`check_evidence_field(root, rfc, "Independent design review", failures)`) is byte-identical before and after. Matches "condition 9's logic is byte-identical to its introduction."
- **One discrepancy, minor:** the claim "all twelve `author` occurrences in the file are prose" — today's count is 13 by substring (`grep -o author`) or 4 by whole-word boundary (`\bauthor\b`), not 12 either way. Traced the gap: commits `738c233` (RFC 110, conditions 14/15) and `40425d3` landed after whatever count produced "twelve", each adding more `author`/`authorised`/`authority` prose. Read every match by hand (reproduced above) — all are comments, docstrings, or unrelated words (`authorised`, `authority`); **none is code-level author-identity-comparison logic**, so the property the row is actually asserting still holds. The number is stale, not the conclusion. Worth correcting the figure before this closes, since this project's own convention treats a wrong declared count as a finding even when the substance is right.

**RFC 129 — reviewer-existence claim.** Grepped `rfcs/accepted/*.md` and `ROADMAP.md` for forward-looking review/reviewer requirements. The one match that looks like a standing requirement (`094-transactional-audit-registry.md:40`, "required fresh independent design review") reads in context as past tense — "That review is complete and recorded... the return is closed." `ROADMAP.md:78-79, 599` name the RFC 129 D2 external engagement as the sole scheduled outside review and say why (M6 is the first public-readiness claim). No other match names a reviewer that doesn't exist. Matches.

**RFC 132 — no email contact route.** `grep -rE` for an email pattern plus a `mailto:` scan across `.github/` and `CONTRIBUTING.md`: zero matches, in the current tree. (The closure review cites a diff-level count — 1,676 inserted lines, one fixture address — which I did not re-derive exactly; the end-state property is what matters and it holds.)

**RFC 130 — the two mechanical claims I could test directly, not just read about:**
- `paths` present on all 21 `[lane_profiles]` entries: counted both the `paths = ` occurrences and the entry count — 21 and 21.
- **D8's plant test, run myself rather than trusted:** created `.cargo/config.toml`, ran `scripts/check-gate-inputs.sh --all`: failed, naming the exact path (`"exists but matches no declared Rust-scope pattern"`). Removed it, same command: pass. Confirmed no residue afterward (`git status --porcelain` empty).
- **D7's fail-open, all five conditions, via the CLI directly:** `workflow_dispatch` → `true`, `schedule` → `true`, empty `before` → `true`, all-zero `before` → `true`, an unresolvable 40-hex `before` → `true`. 5/5, matching the claim.
- `python3.14 -m unittest scripts.tests.test_compute_changed_scope`: 13/13 pass.

**Cross-check: nothing closed early.** None of `rfcs/accepted/{110,128,129,130,132}.md` carries a `Closure reviewed on`/`Closure approved by` field; all five RFCs remain in `accepted/`, none in `done/`. Matches the stated state — approval has not happened and nothing was moved.

## What this does not do

It does not decide RFC 131's three-way fork — that is `@nabbisen`'s, as the closure review already says. It does not perform any closure action; that is the architect's, gated on approval. It adds one finding (the stale "twelve" count) and otherwise corroborates the batch's claims from independent measurement rather than trust.

## Durability note

Like RFC 128's D2 trace, this file sits in `.git-exclude/`, not tracked. Flagging rather than deciding: if independent corroboration should be durable evidence for a closure this significant (the project's first), committing it — or folding its one finding into the tracked closure-review document — is a call for the architect, not for me to make by moving it myself.

**Entry point of this package:** `.git-exclude/review-requests/closure-review-batch-1-independent-verification-2026-10-02.md`
