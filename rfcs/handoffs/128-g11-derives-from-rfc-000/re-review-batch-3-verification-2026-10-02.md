# Independent verification — re-review of the fifteen, batch 3 of 3 and the close-out (126, 121, 123)

**Date:** 2026-10-02
**Reviewing.** [`rfcs/handoffs/128-g11-derives-from-rfc-000/re-review-batch-3-2026-10-02.md`](./re-review-batch-3-2026-10-02.md)
**Not an implementation package.** No tree change accompanies this. `git status --porcelain` is empty.

## The finding: the "all fifteen" close-out table accounts for twelve

The close-out section is titled "Close-out — all fifteen" and its shape table is presented as the tally of the whole exercise. Counted the four shape rows directly against the actual RFC lists from all three batches:

| Shape | RFCs listed | Count |
|---|---|---|
| He ruled the judgment explicitly | 115, 102, 103, 116 | 4 |
| Approval named the deciding change | 118 | 1 |
| Visibility gap | 110, 112*, 121*, 122, 123 | 5 |
| Accepted before the design was reviewed | 117, 126 | 2 |
| **Table total** | | **12** |

The fifteen, from all three batches' own scopes: batch 1 = 115, 102, 103, 112, 118, 122 (6); batch 2 = 117, 094, 095, 096, 110, 116 (6, counting 094/095/096 as three RFCs sharing one document, which is how both this document and batch 2 itself count them); batch 3 = 126, 121, 123 (3). Total 15.

**094, 095 and 096 do not appear anywhere in the close-out table.** They are real, verified RFCs from batch 2 — not dropped because they were resolved elsewhere; batch 2 and this document's own "Still with `@nabbisen`" list (item 2) both say the 094/095/096 scheduling judgment remains outstanding. They don't fit any of the four shapes as written, because the four shapes are about *historical record quality* for decisions already reached, and 094/095/096 is explicitly the opposite case: a judgment that **has not yet reached `@nabbisen` at all**, governing work that has not started. That's a real, fifth category, not an omission that resolves to one of the four — but the table's title claims full coverage and its count silently comes up three short. This is worth a corrected title or an explicit fifth row/footnote before `@nabbisen` uses this table as the complete picture.

## Everything else, checked and confirmed exactly

**RFC 126's disclosure, quoted verbatim and matched** (`rfcs/done/126-....md:10-12`): *"This RFC was accepted before its own design had been reviewed — the field below first cited the review that found the problem, which examined RFC 123's design, not this one's... G11 could not see that, because the citation resolved to a tracked document."*

**The proposed gate rule — simulated against the real tree, not just reasoned about.** Wrote a script applying the stated rule ("at least one `Independent design review` citation must resolve to a path under this RFC's own `handoffs/<N>-` directory, else it needs an allowlist entry") to every RFC in `proposed/`, `accepted/` and `done/` with that field (26 RFCs). **Result: exactly 3 need an allowlist entry — 095, 096, 103 — and no others, matching the claim precisely.** RFC 126 passes (it now cites its own review alongside RFC 123's), also matching. This is the kind of claim worth simulating rather than trusting, since "exactly three, and here they are" is a strong, specific, falsifiable statement — and it held.

**RFC 121's self-correction, traced through its actual commits**, not just taken on the document's word:
- D3's text (`rfcs/done/121-....md:80`) matches: *"A failure to verify is recorded, not only displayed..."*
- The approval sentence (`:8-11`) matches exactly, naming the RFC 125 blocker but not D3.
- Found the historical text being corrected: commit `d0046fd` introduced *"D3 is settled and no longer yours to decide — the review resolved it"*; commit `9c87dd1` (2026-09-30 20:38) removed it, with the message *"a review recommends; the design and the decision are the architect's... one line of mine did not honour it."*
- **Timing confirmed**: that self-correction (20:38 on 2026-09-30) predates RFC 128's own creation (`796b920`, 2026-10-01 07:59) by about 11 hours — the claim "before RFC 128 existed" is exactly right, not approximate.

**RFC 123's D1 and shipped code:**
- D1 (`rfcs/done/123-....md:61`): *"Every endpoint that authenticates a client takes a limit, and it is two"* — exact.
- Shipped as claimed: `enforce_client_endpoint_rate_limits` (`crates/sui-id/src/http/handlers/oauth_token.rs:165`) and `RateLimitKey` (`crates/sui-id/src/http/handlers.rs:580`) both exist.
- Approval (`:8-9`): *"Accepted. On the amended text."* — names no choice, matching the "visibility: gap" classification, and self-discloses the Proposed-status implementation timing error already on record.

## What this does not do

It does not relitigate any of the three judgments on the merits — all were right, and I found nothing to dispute in the engineering. It raises one structural finding (the undercount in the close-out's own headline table) that affects how complete the picture looks to `@nabbisen`, not what any individual verdict says.

**Entry point of this package:** `.git-exclude/review-requests/re-review-batch-3-independent-verification-2026-10-02.md`
