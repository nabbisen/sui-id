# Independent verification — re-review of the fifteen, batch 2 (117, 094/095/096, 110, 116)

**Date:** 2026-10-02
**Reviewing.** [`rfcs/handoffs/128-g11-derives-from-rfc-000/re-review-batch-2-2026-10-02.md`](./re-review-batch-2-2026-10-02.md)
**Not an implementation package.** No tree change accompanies this. `git status --porcelain` is empty.
**Why this exists.** Same reasoning as batch 1 — this batch's most serious claim (RFC 117's unrecorded amendment) is specifically a verifiability-of-owner-decision claim, which is the property this role is best positioned to check against the actual commit history rather than the prose describing it.

## Headline: no discrepancy found — and the RFC 117 timeline is confirmed, with one detail sharper than stated

Every quoted sentence and cited line in this batch checks out exactly. RFC 117's finding — the one the document calls "live, not archaeology" — holds up under direct commit-history reconstruction, and the history makes the ambiguity the document describes slightly more concrete than it states.

## RFC 117 — reconstructed the actual commit order, not just read the dates

The document says the review and the acceptance text were "both 2026-09-24" with the order left genuinely unclear. I pulled every commit touching the RFC 117 file on that date, in order:

| Time | Commit | What it did |
|---|---|---|
| 06:33 | `8741fa6` | "five rulings of 2026-09-24; RFC 117 opened" |
| 06:54 | `368d278` | **"@nabbisen accepted RFC 117 on 2026-09-24. It stays in proposed/ until its [design review]..."** — the acceptance note, in `proposed/`, before any review |
| 08:31 | `4c3c9e7` | **"design review landed; the claim narrowed and the work split"** — the review's objection is adopted and the RFC text is rewritten here |
| 09:44 | `a887813` | stage 0 implementation |
| 20:28 | `3dec640` | "move to accepted/ — the folder lagged the acceptance by a day" |

**This sharpens the finding rather than just confirming it.** The acceptance note at 06:54 unambiguously predates the review-driven narrowing at 08:31 — it isn't merely "same day, unclear order," it's "accepted, then substantively rewritten, with no second approval recorded for the rewrite." The 20:28 commit only moves the folder; it does not add a fresh approval sentence for the narrowed text (confirmed: `rfcs/archive/117-verifiable-owner-decisions.md` has no `Amended on` line anywhere — `grep -n Amended` returns nothing — and its one approval sentence, *"RFC-117 is accepted"*, is the same sentence structure as the pre-narrowing note, not a new one referencing the rewrite).

This is the strongest form of the finding available from the repository alone, and it supports the document's two asks of `@nabbisen` (confirm the narrowed text was what he meant, or record that the order can't be reconstructed) without needing to soften either option.

**The quoted review sentence is exact**: `rfcs/handoffs/117-verifiable-owner-decisions/design-review-2026-09-24.md:276` — *"I would recommend not **Accepting the text as it stands**, because it claims the enforcement it cannot give."*

## RFC 094/095/096 — quotes confirmed

- `rfcs/handoffs/094-transactional-audit/094-095-096-correction-review-2026-08-26.md:255-257` — *"the 2026-08-26 correction (re-pointing 096-B1's prerequisite to 'M2a's runner foundation' instead) is the right fix for what this question actually found"* — confirmed, reading the full surrounding paragraph for context rather than the fragment alone.
- `rfcs/accepted/096-upstream-oidc-federation-validation.md:14` carries exactly **"Amendment summary (2026-08-26)"**, matching the claim that this amendment (unlike RFC 117's) is recorded.

## RFC 110 — quotes and the approval's exact wording confirmed, typo included

- `rfcs/handoffs/110-rfc-header-governance-guard/design-review-2026-09-24.md:72` — *"The label rule should be an allowlist, not a denylist"* (M1 heading) — exact.
- Same file, line 92 — *"It cannot be checked meaningfully. Do not build it."* (Item 5) — exact.
- `rfcs/done/110-rfc-header-governance-guard.md:8` — *"RFC 115 and 110 are accpepted"* — exact, including the owner's own typo. The approval names no specific choice (no mention of allowlist vs. denylist), consistent with the claimed gap.

## RFC 116 — quotes and the clean record confirmed

- `rfcs/handoffs/116-gate-contracts/design-review-2026-09-24.md:238` — *"Which inventory copy survives — my view (a view, not a ruling). The TOML."* — exact, including the self-labelling.
- `rfcs/done/116-gate-contracts.md:79` — D1 reads *"One source per registry: the TOML survives. Ruled by `@nabbisen`"* — exact.
- Approval line (`:8`) — *"He ruled its two open questions on 2026-09-24 (D1, D3a)"* — exact.

## The running tally, checked for consistency

The document claims the visibility-gap count reaches three: 112 (partial, from batch 1's correction), 122 (batch 1), and now 110. Cross-checked against batch 1's corrected document — both prior entries are as stated, and 110's gap (approval naming no specific choice) is a clean addition to the count, not a restatement of either prior one. The tally is internally consistent.

## What this does not do

No design defect to report — none was claimed, and I found none either. It does not relitigate the 094/095/096 prerequisite amendment's correctness (the review's own reasoning reads soundly and I have no basis to second-guess a scheduling judgment about unbuilt work). It adds one piece of sharper evidence (the exact commit sequence for RFC 117) in support of a finding the document already made correctly.

**Entry point of this package:** `.git-exclude/review-requests/re-review-batch-2-independent-verification-2026-10-02.md`
