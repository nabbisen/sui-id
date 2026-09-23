# RFC 110 — independent design review

**Date:** 2026-09-24
**RFC:** [RFC 110 — An RFC header may not legislate](../../proposed/110-rfc-header-governance-guard.md)
**Request:** [`design-review-request.md`](design-review-request.md)
**Reviewer:** Mid-capability model, implementation role. Authored neither the RFC nor its handoff.
**Baseline read:** `f33c7e5`, working tree clean. Read-only: no code and no RFC text changed. Every count below was produced by running the real `parse_header` from `scripts/check-rfc-integrity.py` over the tree (scratch scripts only; nothing committed).
**Outcome:** The guard is **worth building and buildable, but two of the RFC's own factual premises are false, and as specified it cannot meet its own third closure prerequisite** ("green on the tree it lands in without editing any RFC"). Two blockers, three high, both blockers with a small fix. The label rule is the strong part; the phrase list is the weak part. My view on the wider question (item 11) is in §5.

---

## 1. The measured answers (items 2 and 3)

All matches below are case-insensitive unless stated. Tree: 132 RFC files across `proposed/ accepted/ done/ archive/`. Header = the title line up to but excluding the first `## ` line, exactly as G11's `parse_header` computes it.

**Header hits for the five literal phrases (item 2).** The request says the answer *should be zero*. It is, **but only if the match is a raw per-line search, and a raw search is the wrong design** (see B1):

| Match mode | Header hits | Where |
|---|---:|---|
| Raw text, as a plain substring | **0** | — |
| Whitespace-normalised (`\s+` → one space) | **1** | `rfcs/archive/018-rfc-lifecycle-policy.md`, header lines 12-13: "*the implementer cannot / be the sole approver* of a security-sensitive design", inside an emphasis span that wraps across a line break. It is a **record** ("RFC 000 … carries … the constraint that…"), not a rule. |
| Label rule (`Independent security and <x> reviewer`) | **0** | — |

**Body hits (item 3).**

| Match mode | Body hits | Where |
|---|---:|---|
| Raw | **1** | `rfcs/archive/018` body, line 220 ("Independence here means **role independence**"; matched case-insensitively — with the RFC's case-sensitive `Role independence` it is 0) |
| Whitespace- and emphasis-normalised | **3, in 2 files** | `rfcs/done/000` body (its real sentence, wrapped: "The implementer cannot be / the sole approver of a security-sensitive design or its closure evidence", lines 40-41); `rfcs/archive/018` body (`Role independence`, `cannot be the sole approver`) |

None of the body hits could be reached by a *boundary* bug: I checked the boundary on all 132 files. The fence-unaware `^## ` cut and the fence-aware cut (`iter_unfenced_lines`) **agree on every file** (0 mismatches); every RFC has a level-2 heading; every file's line 1 is a `# ` title with no second H1 in the header; the longest headers are 67 lines (`done/102`), 51 (`done/084`), 44 (`accepted/094`), median 13. `done/000` and `archive/018` embed their example metadata blocks in the *body* (after the first `## `), so the header is clean for both. Sixteen historical `RFC-MI-*` files carry a fenced ` ```toml ` front-matter block *inside* the header; none contains a phrase.

**Two further measurements the RFC needs**, because it claims the guard would have caught both incidents:

| Tree | Five phrases, raw | Five phrases, normalised | Label rule | July-style "Vendor independence" headers present |
|---|---:|---:|---:|---:|
| `363e75b^` (just before the September removal) | 11 files | +1 (`018`) | **11 files** | 1 |
| `12f464b^` (just before the July withdrawal) | **1 file** (`095`) | 1 | **8 files** (`093`–`100`) | **4** (`094`, `096`, `097`, `099`) |

So the label rule catches every clause in **both** incidents, including all four July headers; the five phrases catch all eleven September files only because the list was copied from that incident's text, and catch **none of the four July headers**. The July header read "**Vendor independence required** by the owner's 2026-07-28 S1 ruling — the reviewer of record must be outside the vendor…" (`094-transactional-audit-registry.md:19` at that commit). No phrase in the list occurs in it.

---

## 2. Findings

### BLOCKER

**B1 — Condition 14 cannot be both effective and green on the landing tree.**
- **Raw matching is trivially evaded.** Hard-wrapped prose splits a phrase across a line break; the archive/018 hit above is exactly that case (the phrase is contiguous only after whitespace collapses). The eleven September clauses were caught by a raw scan *by accident of where their lines broke*. An author who writes "must not\nhave authored" defeats the guard.
- **Normalised matching fires on `archive/018` today.** That breaks closure prerequisite 3 ("green … without editing any RFC to make it so") and the handoff's own last test row.
- **Fix:** normalise (collapse whitespace; strip `*`, `_` and backticks; case-fold; scan the whole header text, **not per field** — 115 RFC headers carry unlabelled continuation lines, so a sentence routinely spans lines and even fields), **and scope conditions 14 and 15 to `proposed/`, `accepted/` and `done/`.** An archived RFC's header is the record of a disposed document; it cannot be re-litigated by a live gate, and RFC 110's own reason to exist (stop a rule *entering* the live set) does not apply to it. With both changes the tree is green: 0 hits in `proposed/accepted/done` under every match mode I ran.

**B2 — Condition 15 is red on the landing tree, and the RFC's statement "Today this catches nothing" is false.**
Running "an `RFC NNN` reference in a header that resolves under `rfcs/archive/`" finds **three files**:
1. `rfcs/proposed/025-multi-tenant-expansion.md` — a **legitimate supersession**: `Supersedes [RFC 007 (Multi-tenancy)](../archive/007-multi-tenancy.md), which had the topic but not the detail.` (twice: the text and the link).
2. `rfcs/archive/007-multi-tenancy.md` and 3. `rfcs/archive/018-rfc-lifecycle-policy.md` — **each file's own title line** (`# RFC 007 — …`, `# RFC 018 — …`) is inside the header block and resolves to the archive.
This answers item 8: **yes, a carve-out is needed, and it is exactly one file**. The archive-folder scope fix (B1) removes 2 and 3 with no special case. For 025, do **not** write a lexical "preceded by `Supersedes`" exemption (any author can write the word); add a **closed allowlist to `ci/rfc-policy.toml`** — `[archive_citations]` mapping `"025" = ["007"]` — the same mechanism as `[historical_rfc_mi]`, so the one legitimate exception is a visible, diffable, reviewable line. Also skip the title line and self-references.

### HIGH

**H1 — The template that reproduces the clause is outside the RFC's scope, and a second proposal contradicts this one.**
- `rfcs/README.md:354` — the project's normative RFC template — still tells authors to write `**Independent security and closure reviewer.** Role independence per RFC 000 — the reviewing role and what it reviews…`. That is the banned **label** and a banned **phrase** (`Role independence`). `rfcs/README.md` is not an RFC, so G11's header scan never reads it, and RFC 110's `Touches` names only the script and its tests. The next author who follows the documented template writes a header that fails G11.
- `rfcs/handoffs/111-rfc-template-reconciliation/README.md` ("What to do", item 2) instructs the opposite: *add* `Independent security and closure reviewer` to the template because "seven or eight uses … is convention".
- **Fix:** the same package edits the template (delete that line; the reviewer facts are carried by `Independent design review` and `Accountable owner and approver`, which stay), and the RFC 111 handoff is amended to drop that label from its list. Otherwise the documentation and the gate disagree again, which is RFC 098's thesis and RFC 111's own opening complaint.

**H2 — Phrase 5, `cannot be the sole approver`, is RFC 000's own sentence.** `rfcs/done/000-rfc-lifecycle-policy.md:40-41`: "The implementer cannot be the sole approver of a security-sensitive design or its closure evidence." The other four phrases are the invented bar; this one is the real rule. As specified the guard would reject a header that **accurately cites RFC 000**, which is the opposite of the principle ("may not *legislate*", not "may not *quote*"). The stage 5 review of RFC 103 quoted this very sentence correctly. **Fix:** drop phrase 5. Measured: labels + phrases 1-4 still catch every clause in both incidents.

**H3 — The exemption cannot be checked; do not build it.** See item 5.

### MEDIUM

**M1 — The label rule should be an allowlist, not a denylist.** Today exactly six header labels mention review, approval or independence: `Security review` (24), `Accountable owner and approver` (24), `Approved by` (7), `Independent design review` (7), `Closure reviewed on` (3), `Closure approved by` (3). A rule "any header label matching `review|approv|independen|authori` that is not one of these six fails" is **0 hits on the tree today**, and it also catches a renamed field (`Reviewer requirements.`, `Independent security reviewer.`, `Independence.`) that the exact regex `Independent security and <x> reviewer` lets through. This is the structural core of the guard; the phrase list becomes a secondary net.

**M2 — The nine self-tests are the right core but miss the branches that matter.** RFC 093 requires each G11 invariant to have "one invalid fixture and one boundary-valid fixture" (`rfcs/done/093-build-toolchain-release-gates.md:152`). Missing: a phrase **wrapped across a line break** (B1); a phrase inside an **emphasis span**; **case variants**; a **renamed label** that the exact regex misses (M1); the **archive-folder scope** (a header hit in `archive/` passes); condition 15 for the **plural/list form** `RFCs 018 and 099` (5 headers use `RFCs …` lists today; none names an archived RFC, but a singular-only `RFC NNN` regex misses them), the **link form** `../archive/…`, and the **title/self-reference**; the **policy allowlist** (pass with the entry, fail without); **exemption isolation** if any exemption survives (it must not silence condition 15 or another RFC); and a **boundary-valid** fixture per condition (innocent header text containing `sole` and `approver` separately). Twenty tests, not nine; the existing suite has 28 and runs in CI (`.github/workflows/ci.yml:616`).

**M3 — The July mechanic is attribution to a dated owner ruling, and neither condition inspects attribution.** See item 11.

### LOW

**L1** The failure message should name file, **line**, the matched phrase, and say "a header may record who reviewed something; it may not rule on who is allowed to". Normalisation loses line numbers; map matches back to the original line.
**L2** Bare-number references (`018`) are safe to skip: 68 header matches of `007|018` exist, all noise (`i18n`, commit hashes, dates).
**L3** RFC 110's own header wording ("cannot state a rule about who may review") is not caught by any phrase and does not need to be.

---

## 3. Answers to items 1, 4–10

**1. Is the header block well-defined?** Yes; see §1. One caveat that changes the design: the boundary is *fence-unaware* (`re.match(r"^## ", line)`, `check-rfc-integrity.py` `parse_header`), which is harmless today (0 mismatches) but means a fenced block containing `## ` early in a future header would cut it short and hide a later clause. Cheap hardening: use `iter_unfenced_lines` for the boundary.

**4. Is the field-label rule right?** The exact regex has **no legitimate use** on today's tree (0 hits) and RFC 000's template does not require it (`rfcs/done/000` names `independent design-review identity/reference`, not this label). But it is too narrow; use the allowlist in M1.

**5. The exemption.** **It cannot be checked meaningfully. Do not build it.** What the gate *can* check is syntax: an ISO date and a Markdown link that resolves to a tracked, non-ignored file (it already does exactly that for `Independent design review`, `check_evidence_field`). Any author can write a date and link a tracked file — the field satisfies the check whether or not any owner approved anything, so it would be the hole the request describes. It also has nothing to resolve against: there is no record of owner approvals (item 11), and commit identity does not help (§5). If a real exception ever exists, make it a **policy-file entry** (`ci/rfc-policy.toml`, closed list, diffable) rather than free text in the very header the guard polices. That moves the exception to a place whose history is reviewed, though I would not claim it is stronger than the author's own good faith.

**6. Can the number-to-folder resolution be reused?** Yes. `discover_rfcs` already yields `Rfc(folder, number, namespace)` for every file, and `check_handoff_correspondence` shows the per-folder `NNN-*.md` glob. Build `where[number] → folders` once (measured: no number appears in two folders; archived numbers are `007` and `018`). `RFC-MI-*` needs its own arm; none is archived (the historical list is closed), so it is a one-line no-op today.

**7. Forms to catch.** All 261 `RFC NNN` references in headers use exactly one form (`RFC ` + digits): no `RFC-018`, no zero-padded variants. Catch: `RFCs?[ -]?NNN` (singular, plural and list forms: `RFCs 018 and 099`, `RFCs 093–103`) and any Markdown link whose target path contains `/archive/`. **Skip bare `NNN`**: it is unmatchable without huge noise (L2). The two skipped forms are safe because a header cannot rest on a document it does not name.

**8. False positives.** Yes: `proposed/025`'s `Supersedes [RFC 007]` (B2) and every archived file's own title. Carve-outs, precisely: scope to non-archive folders; skip the title line; one allowlist entry `"025" = ["007"]`.

**9. Does this belong in G11?** Yes. G11's docstring already hosts items 12 and 13 with the same disclaimer; `ci/gate-inputs.toml:84` pins only the **command** (`--policy ci/rfc-policy.toml`), which does not change, and `scripts/check-gate-inputs.sh` compares commands with RFC 093's table (`rfcs/done/093…md:116`), so nothing pulls in A3.4. Nothing hashes or pins the script's content. The one addition outside the script is the `[archive_citations]` table in `ci/rfc-policy.toml`, which G11 already loads in `main()`. RFC 093's closed contract is not amended: 12 and 13 set the precedent. Update the docstring's list to 14 and 15 and say, as it does for 12 and 13, that RFC 000 remains their source.

**10. Are the nine tests the right nine?** See M2: right core, incomplete. The two most important omissions are the **wrapped phrase** and the **archive scope**, because those are the two ways the guard as specified is either evaded or red.

---

## 4. What I would change in the RFC (summary)

1. Match on **normalised** header text; scan the whole header, not per field.
2. Scope conditions 14 and 15 to `proposed/`, `accepted/`, `done/`.
3. Drop phrase 5 (RFC 000's own words).
4. Replace the exact label regex with the **six-label allowlist**.
5. Delete the header exemption; use a `ci/rfc-policy.toml` allowlist if one is ever needed.
6. Condition 15: singular/plural/link forms; skip the title line; `[archive_citations]` allowlist with the one entry for `025`.
7. `Touches` gains `rfcs/README.md` (remove the template line) and `ci/rfc-policy.toml`; amend the RFC 111 handoff item 2.
8. Twenty self-tests, one invalid and one boundary-valid per branch, per RFC 093.

---

## 5. Item 11 — my view on the wider question (a view; `@nabbisen` rules)

**View: build the narrow guard, and treat it as a partial fix that will need *supplementing*, not replacing. It is worth building on its own.** Reasons:

- **What it is good at, measured.** The label rule and the allowlist catch every clause in both incidents, including the July headers the phrases miss. That is a structural fact about *where* these clauses were written (a reviewer-labelled header field), and the allowlist form of the rule does not depend on anyone guessing the next wording.
- **What it cannot do, measured.** It is a denylist of *places and wordings*. It says nothing about the mechanic the request names: an unverifiable *attribution*. The July clause was dangerous not because of its words but because it said "the owner's 2026-07-28 S1 ruling", and nothing could check that.
- **A lexical wider guard is not viable.** I measured owner-attribution phrases in the 132 RFC headers: **12 headers** carry one legitimately (`Approved by`, `Accepted on`, "Owner decision, 2026-09-10" in `archive/018`). Across all tracked files a loose pattern finds 10 distinct dates in 24 files, against the request's 7 in 22: the count depends on the regex, which is itself the problem. A gate that bans the phrase bans required fields; one that allows it checks nothing.
- **Why a ledger alone does not fix it.** I checked what git can and cannot prove: `git log` shows **410 commits under two identities** (216 as `nabbisen@scqr.net`, 194 as the GitHub noreply address), `commit.gpgsign=true`, and signature status `U` (good signature, unknown validity) on 405, `N` on 4, `E` on 1. Because the agent commits under the owner's own configuration, an agent-made commit carries the same identity and the same signature as an owner-made one. So a ledger file in the tree is *also* something an agent can write, and a gate that requires "cite a ledger entry" would be satisfied by an entry the agent added. **The ledger only becomes evidence if its entries are signed with a key the agents cannot use** — for example an SSH or GPG signing key held on a hardware token that requires the owner's touch, whose fingerprint is pinned in `ci/`, with the gate verifying `git verify-commit` (or `ssh-keygen -Y verify`) on the commit that added each entry. That is the only construction I can see in which "the owner said this" becomes checkable rather than asserted. It is a real cost to the owner (a second key, a step per ruling), and it is his decision whether the tree needs that level of evidence.
- **Sequencing.** Build RFC 110 now: it is small, it closes the observed failure at its source, and it does not foreclose the wider design. The wider ledger, if he wants it, is a separate RFC whose first question is the signing key, not the gate.

## 6. What I would want in the acceptance package

The twenty tests (M2), the `ci/rfc-policy.toml` change with the single `025` entry, the `rfcs/README.md` template edit and the RFC 111 handoff amendment, the docstring update, and the real-tree run showing 0 violations **without editing any RFC**, with the measurements in §1 reproduced in the review request as the before/after.

