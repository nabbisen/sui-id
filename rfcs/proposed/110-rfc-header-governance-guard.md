# RFC 110 — An RFC header may not legislate

**Status.** Proposed
**Security review.** Required
**Independent design review.** [Design review 2026-09-24](../handoffs/110-rfc-header-governance-guard/design-review-2026-09-24.md) by the implementation role, which authored neither this RFC nor its handoff. Two blockers, three high, three medium, three low; all resolved in this text. Its measurement overturned the RFC's central mechanism (§D1).
**Design prerequisites.** RFC 000 is the source of the rule being guarded and is not amended by this RFC.
**Implementation prerequisites.** None.
**Closure prerequisites.** An RFC header in `proposed/`, `accepted/` or `done/` cannot carry a reviewer-rule field or state a rule about who may review, and cannot cite an archived RFC as authority except by a closed, diffable allowlist entry; the guard catches the clauses of **both** past incidents, demonstrated against their commits; and the gate is green on the tree it lands in without editing any RFC to make it so.
**Tracks.** Governance integrity.
**Touches.** `scripts/check-rfc-integrity.py`, `scripts/tests/test_rfc_integrity.py`, `ci/rfc-policy.toml`, `rfcs/README.md` (the template still reproduces the banned clause), and `rfcs/handoffs/111-rfc-template-reconciliation/README.md` (which instructs the opposite).
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/110-rfc-header-governance-guard/README.md`](../handoffs/110-rfc-header-governance-guard/README.md)

## Summary

Twice, a governance rule has been written into RFC headers, attributed upward
to an authority that does not contain it, and left unchecked. In July a
vendor-independence rule went into four headers attributed to an owner ruling
that cannot be evidenced, blocking seven RFCs for four weeks. On 2026-09-22
eleven headers (093–103) were found asserting a reviewer bar RFC 000 does not
contain, two of them citing the disposed RFC 018. The clauses are removed;
nothing prevents a third occurrence.

## What the design review changed

The first version of this RFC proposed a **list of five literal phrases**. The
review measured both incidents and showed the list is overfitted to the second
one:

| Tree | Five phrases | Label rule |
|---|---:|---:|
| `363e75b^` — just before the September removal | 11 files | **11 files** |
| `12f464b^` — just before the July withdrawal | **1 file** | **8 files** |

The four July headers read *"**Vendor independence required** by the owner's
2026-07-28 S1 ruling…"* — and **no phrase in the list occurs in any of them.**
A phrase list catches the wording of the incident it was copied from. The
**field label** is the structural fact: both incidents put the clause in a
reviewer-named header field. So the label rule leads, and the phrases become a
secondary net.

The review also found the RFC's claim that condition 15 "today catches
nothing" is **false** — it is red on three files — and that the exemption
mechanism cannot be checked at all.

## Decisions

**D1 — The primary rule is a closed allowlist of header labels.** Exactly six
header labels mention review, approval or independence today: `Security
review`, `Accountable owner and approver`, `Approved by`, `Independent design
review`, `Closure reviewed on`, `Closure approved by`. Any header label
matching `review|approv|independen|authori` that is not one of the six fails.
Measured: **zero hits on today's tree**, and it catches every clause in both
incidents, including a renamed field (`Reviewer requirements.`,
`Independence.`) that the original exact regex would have let through.

**D2 — Phrase matching is normalised, and scans the whole header.** Collapse
whitespace, strip `*`, `_` and backticks, case-fold. Raw matching is defeated
by a hard line break — the eleven September clauses were caught by a raw scan
only by accident of where their lines broke — and 115 RFC headers carry
unlabelled continuation lines, so a sentence routinely spans lines and even
fields.

**D3 — `cannot be the sole approver` is removed from the phrase list.** It is
RFC 000's own sentence (`rfcs/done/000-rfc-lifecycle-policy.md:40-41`). Banning
it would reject a header that **accurately cites** the governing policy, which
is the opposite of this RFC's principle: a header may not *legislate*; it may
certainly *quote*. Measured: the label rule plus the remaining four phrases
still catch every clause in both incidents.

**D4 — Conditions 14 and 15 apply to `proposed/`, `accepted/` and `done/`, not
to `archive/`.** An archived RFC's header is the record of a disposed document
and cannot be re-litigated by a live gate; this RFC exists to stop a rule
*entering* the live set. Without this scope the normalised match fires on
`rfcs/archive/018`, whose header *records* RFC 000's constraint rather than
asserting one — and closure prerequisite 3 would be unmeetable.

**D5 — There is no header exemption field.** The review is conclusive: a gate
can check that a field holds a date and a link to a tracked file, and any
author can write both, so the field would be satisfied whether or not anyone
approved anything. An exemption anyone can write is a hole. If a real exception
ever exists it becomes a closed entry in `ci/rfc-policy.toml`, whose diff is
reviewed — which is weaker than proof and stronger than free text in the very
header the guard polices.

**D6 — Condition 15 gets a closed allowlist, because it is red today.** Three
header citations of an archived RFC exist: `rfcs/proposed/025`'s legitimate
`Supersedes [RFC 007]`, and the **title lines** of `archive/007` and
`archive/018` themselves. D4 removes the last two with no special case. For 025,
a lexical carve-out on the word "Supersedes" would be writable by anyone, so
instead `ci/rfc-policy.toml` gains `[archive_citations]` with the single entry
`"025" = ["007"]` — the same mechanism as `[historical_rfc_mi]`. The check also
skips an RFC's own title line and self-references, and matches the singular,
plural and list forms plus any Markdown link whose target contains `/archive/`.
Bare numbers are not matched: 68 header matches of `007|018` exist and all are
noise.

**D7 — The template and RFC 111 are corrected in the same package.**
`rfcs/README.md:354` — the project's normative RFC template — still instructs
authors to write the banned label and a banned phrase, and `rfcs/README.md` is
not an RFC, so the gate would never see it. The next author who follows the
documented template writes a header that fails G11. Worse,
`rfcs/handoffs/111-rfc-template-reconciliation/README.md` instructs the
opposite: *add* that label to the template. Both are fixed here, or the
documentation and the gate disagree again — which is RFC 098's thesis and RFC
111's own opening complaint.

**D8 — The gate is hosted by G11, and RFC 093 is not amended.** G11's docstring
already hosts invariants 12 and 13 with the disclaimer that RFC 000 remains
their source; 14 and 15 follow that precedent. `ci/gate-inputs.toml` pins only
the command, which does not change, so A3.4's lane-agreement check is not
engaged.

**D9 — Twenty self-tests, not nine.** RFC 093 requires one invalid and one
boundary-valid fixture per invariant. The branches that matter and were missing:
a phrase **wrapped across a line break**; a phrase inside an emphasis span;
case variants; a **renamed label**; the archive-folder scope; condition 15's
plural, list and link forms, and the title/self-reference; the policy allowlist
with and without its entry; and a boundary-valid header containing `sole` and
`approver` separately.

## What this guard does not do

Stated because the review measured it and the RFC should not be read as
claiming more.

The mechanic behind both incidents is an **unverifiable attribution** — the
July clause was dangerous because it said "the owner's 2026-07-28 S1 ruling",
and nothing could check that. This guard is a denylist of *places and wordings*;
it says nothing about attribution. A lexical guard over attributions is not
viable either: 12 RFC headers carry an owner attribution legitimately
(`Approved by`, `Accepted on`), so a gate that bans the phrase bans required
fields, and one that allows it checks nothing.

The review also established why a ledger of owner decisions, on its own, would
not close it: the repository's 410 commits carry two identities and are signed,
but **an agent commits under the owner's own configuration**, so an agent-made
commit carries the same identity and the same signature as an owner-made one. A
ledger file in the tree is therefore also something an agent can write, and a
gate requiring "cite a ledger entry" would be satisfied by an entry the agent
added. The construction that would make "the owner said this" checkable is a
signing key the agents cannot use — held on a hardware token requiring the
owner's touch, its fingerprint pinned in `ci/`, with the gate verifying the
signature on the commit that added each entry. That is a real, recurring cost
to `@nabbisen` and is **his decision, in its own RFC**. This one does not
foreclose it.

## Risks

- **D1 is a denylist of label *shapes*, so a clause written under a label that
  mentions none of the four stems escapes it.** The phrase net exists for that
  case, and it is why D2's normalisation matters. Neither is proof; both
  together caught 100% of what actually happened twice.
- **D7 edits `rfcs/README.md` and another RFC's handoff.** Small, but it means
  this package touches RFC 111's scope; whichever lands second must not
  reinstate the label.
