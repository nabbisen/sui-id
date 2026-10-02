# Independent verification — re-review of the fifteen, batch 1 (115, 102/103, 112, 118, 122)

**Date:** 2026-10-02
**Reviewing.** [`rfcs/handoffs/128-g11-derives-from-rfc-000/re-review-batch-1-2026-10-02.md`](./re-review-batch-1-2026-10-02.md)
**Not an implementation package.** No tree change accompanies this. `git status --porcelain` is empty.
**Why this exists.** Same reasoning as the closure-review batches, with one difference worth naming: this document is a self-review of the architect's own prior design-review judgments, and its central claim is specifically about whether an owner-decision reached `@nabbisen` — which is exactly the property `check-owner-attributions.py` (G16) exists to keep honest elsewhere in this repo. Checking it independently matters more here than in a routine closure review.

## The finding: RFC 112's visibility claim doesn't match its own evidence

The document's central argument is a two-shape table: 115/102/103 show `@nabbisen` ruling explicitly; 112/118 show him "accepting amended text, with the deciding change named in the approval." RFC 122 is then singled out as the one exception — its approval sentence names no specific choice, so "the record does not show the placement choice reaching him."

**Checking RFC 112 against its own evidence shows it belongs in the same bucket as 122, not the one the document puts it in.**

The document's row for 112 says: *"'I recommend no override flag'; one shared reader rather than a guard inside `run` only"* → **"Right, and the deciding changes are named in his approval. There is no override flag — `upgrade.md:105` states it and D1 carries it... The shared-reader point is the stronger of the two."** It identifies the two judgments under review as **D1** (no override flag) and **D4** (one shared reader — confirmed by grep: `**D4 — One reader, one rule, for every caller.**`, line 87).

But the approval sentence it cites as evidence of visibility reads, verbatim (`rfcs/done/112-schema-version-fail-closed.md:10`): *"the changes that matter are **D2 and D3**"* — and D2 (`"Fresh" means no application tables, not no row`, line 68) and D3 (`The check runs first, on a read-only connection`, line 76) are two **different, unrelated design decisions** — about what counts as a fresh database and about connection ordering, not about the override flag or the reader architecture. Read in full, they share no subject matter with D1 or D4.

**So the approval sentence the document quotes does not name either of the two judgments the document is vouching for.** By the standard the document applies to RFC 122 — "a reader cannot tell from that sentence that [this] choice was settled" — RFC 112's D1 and D4 fail the same test. The shape table's classification of 112 as "deciding change named in the approval" is supported by a quote about different decisions entirely.

**RFC 118 does not have this problem — checked the same way, for comparison.** Its approval sentence (`rfcs/done/118-lockout-clears-on-credential-change.md:10`) says *"the change that matters is D1's second-factor carve-out"*, and 118's own D1 (line 53) **is** the second-factor carve-out, confirmed by reading the full paragraph: *"The carve-out is the whole point... the second-factor lockout (L07) writes the same field from `mfa_failure_count`."* The label and the content match. 118 genuinely belongs in the "named in the approval" bucket; 112 does not, on the same test.

**This doesn't change any verdict on the merits** — I independently confirmed D1's no-override-flag reasoning is sound (`docs/src/guides/upgrade.md:105`: *"There is no override flag"*, matching exactly) and D4's shared-reader design is real (`migrations::read_stored_version`/`check_supported` are the landed functions). The *design* holding up is not in question. What's wrong is narrower and more specific: **the claim that this particular judgment's visibility to `@nabbisen` is established by the approval sentence is not supported by that sentence.** On the document's own terms, RFC 112 should have been grouped with RFC 122 as a case where the prospective naming convention would also have mattered — the two-shape table's count should likely read "three RFCs with a visibility gap" (112, plus 122, which the document already flags), not one.

## Everything else, checked and confirmed

**The four quoted approval sentences — all verbatim matches**, not paraphrases:
- 115: `rfcs/done/115-user-creation-without-a-password.md:8` — *"He ruled all three open questions the same day, each as the independent design review recommended (D10–D12)"* — exact.
- 102, 103: `rfcs/done/102-authentication-fails-closed-without-audit.md:26` and `103...:20` — *"who also ruled every open question as recommended"* — exact, both files.
- 118: confirmed above, exact and internally consistent.

**RFC 122 — the router-layer design, checked structurally, not just read about:**
- The design review's exact recommendation (`rfcs/handoffs/122-.../design-review-2026-09-30.md:52`): *"Recommend: a router-layer, on named routes — a pattern this codebase already runs, not a new one."* — exact.
- It shipped: `paths_carrying_no_store_layer()` exists in `r122_routes.rs:74`, and `crates/sui-id/src/http/router.rs` carries 8 separate `SetResponseHeaderLayer::overriding(..., "no-store")` attachments — the router-layer pattern, not a response builder or typed wrapper.
- RFC 122's actual approval sentence (`rfcs/done/122-....md:8-10`): *"its design review corrected three things this RFC claimed and found a sixth surface it had missed"* — confirmed, names no specific architectural choice. The document's claim about 122's visibility gap is accurate.

**102/103's specific citation:**
- `103:116` reads *"expiry 30 minutes, the same as `DEFAULT_TOKEN_TTL`"* — exact. `DEFAULT_TOKEN_TTL` is `Duration::minutes(30)` (`forgot_password.rs:71`), and `recovery_link.rs` reuses it at both mint sites rather than hardcoding a new value — confirmed by reading, not assumed.

**RFC 115's `must_change` claim** — already independently verified in batch 3's own closure review (same facts, same conclusion); not re-run here since nothing changed.

**RFC 118's test** — `r118_the_sign_in_page_ignores_any_query` exists at `r118.rs:467` and passes (`cargo test -p sui-id --test e2e r118_the_sign_in_page_ignores_any_query`: 1/1).

## What this does not do

It does not relitigate whether any of the five designs are sound — I confirmed the ones I checked structurally and found nothing wrong with the engineering. It raises one specific, checkable claim about evidentiary classification (RFC 112's bucket placement) that affects the scope of the prospective question the document poses to `@nabbisen` — if accepted, the question becomes "should the naming convention apply going forward" against a backdrop of two past gaps, not one.

**Entry point of this package:** `.git-exclude/review-requests/re-review-batch-1-independent-verification-2026-10-02.md`
