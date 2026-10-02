# Re-review of the fifteen — batch 3 of 3, and the close-out

**Date:** 2026-10-02
**By.** The architect. Self-review under `ROADMAP.md` S1c, permitted because
`@nabbisen` asked for this work.
**Scope.** The last three: **126, 121, 123** — the judgments the stage-0 audit
already knew about and the architect had already adopted. Ordered by consequence.
**Question.** Was the judgment right, and does the design still stand?

## RFC 126 — right, already self-corrected, and it exposes a gap in G11

**The judgment.** A caller meeting the concurrency bound **queues** rather than
being refused.

**Right, and the design stands.** The caller waits on
`Semaphore::acquire().await`, which does not hold a worker thread while pending —
so queueing costs latency, not availability. Refusing would have converted a load
condition into an authentication failure, which is the worse behaviour under
exactly the burst this RFC exists to survive.

**The record is the most honest in the fifteen, and it was fixed before I looked.**
RFC 126 discloses, in its own header: *"This RFC was accepted before its own design
had been reviewed — the field below first cited the review that found the problem,
which examined RFC 123's design, not this one's."* The architect held
implementation undispatched until a real review of RFC 126 landed, then amended the
RFC on it. The field now cites **its own** review and separately credits RFC 123's
for finding the problem.

**But the disclosure names something checkable that is not checked**, and it says
so: *"G11 could not see that, because the citation resolved to a tracked
document."* Condition 9 verifies a design-review reference is durable and
resolvable. **It does not verify the review is of this RFC.**

### A gate proposal, measured rather than sketched

The naive rule — "the cited evidence must live under this RFC's own handoff" —
false-positives three ways, and I found that by running it rather than reasoning
about it:

| RFC | Cites | Why it is legitimate |
|---|---|---|
| 095, 096 | `handoffs/094-…/094-095-096-correction-review` | one review covering three RFCs |
| 103 | `handoffs/102-…/102-103-design-review` | one review covering two |
| 126 | also `handoffs/123-…` | a correct credit to where the problem was found |

**The rule that works:** *at least one* citation in the field must resolve to this
RFC's own handoff directory, unless the RFC has an allowlist entry naming the
shared document. Run against the whole tree, that needs **exactly three allowlist
entries** — 095→094, 096→094, 103→102 — and **RFC 126 passes**, because it now
cites its own.

Critically, **it would have failed RFC 126 in its original state**, when the only
citation was RFC 123's. That is the defect it exists to catch, and it is the same
shape as RFC 110's `[archive_citations]`: a closed, diffable allowlist rather than
a lexical guess.

This is a proposal, not a decision. It belongs to `@nabbisen` and, if he wants it,
to its own RFC rather than smuggled into this re-review.

## RFC 121 — right, and the attribution was self-corrected

**The judgment.** D3 settled as `tracing` primary with a best-effort audit row.

**Right.** RFC 121's D3 is *"A failure to verify is recorded, not only
displayed"*; routing the record through `tracing` with a best-effort audit row is
the correct shape, because the audit row is the thing that may itself be
unavailable when the chain is what failed. Making the record depend on the
subsystem under suspicion would have been the error.

**Visibility: partial.** The approval says *"Accepted on the **amended** text — its
independent design review returned 'accept with changes'"* and names the
out-of-scope blocker that became RFC 125. It does not name D3. Same partial shape
as RFC 112.

**Worth recording in the architect's favour, since this review has mostly gone the
other way:** the handoff originally said *"the review resolved it"*, and the
architect corrected that wording to record the decision as his own. The
attribution problem this whole arc is about was being caught and fixed in-flight,
before RFC 128 existed.

## RFC 123 — right, and shipped as judged

**The judgment.** Two rate-limit buckets rather than one.

**Right, and it is D1.** *"Every endpoint that authenticates a client takes a
limit, and it is two"* — shipped as `enforce_client_endpoint_rate_limits`, with a
`RateLimitKey` per endpoint class. One bucket would have let traffic against one
endpoint exhaust the budget protecting another; two keeps them independent. The
review's argument dissolved a tension the architect had posed rather than picking
a side of it, which is the better kind of answer.

**Visibility: gap.** The approval reads *"Accepted. On the **amended** text"* and
names no choice.

The approval also records an error of mine already on the record: the
implementation was built while the RFC was still Proposed because my own
end-of-turn file list named the handoff as a dispatch. The work was held
uncommitted until acceptance, and the implementer's package stated the Proposed
status correctly throughout.

## Independent verification

[`re-review-batch-3-verification-2026-10-02.md`](./re-review-batch-3-verification-2026-10-02.md),
committed beside this document.

**They found the close-out table short by three** — see the note under it. A table
headed "all fifteen" that silently totals twelve is exactly the kind of artefact
this whole arc exists to catch, and it was mine.

They also did the thing worth doing with a strong, falsifiable claim: **they
simulated the proposed gate rule** against all 26 RFCs carrying the field rather
than reasoning about it, and got exactly the three allowlist entries claimed, with
RFC 126 passing. And they traced RFC 121's self-correction through its commits
(`d0046fd` introduced the wording, `9c87dd1` removed it at 20:38 on 2026-09-30),
confirming "before RFC 128 existed" by about eleven hours rather than treating it
as approximate.

Corroboration, not approval. This is the third batch in which they corrected the
architect, having been the subject of the re-review throughout.

## Close-out — all fifteen

| Shape | Count | RFCs |
|---|---|---|
| **He ruled the judgment explicitly** | 4 | 115, 102, 103, 116 |
| **Approval named the deciding change** | 1 | 118 |
| **Visibility gap** — judgment adopted into text, approval names no choice (or names different ones) | 5 | 110, 112*, 121*, 122, 123 (*partial) |
| **Accepted before the design was reviewed** | 2 | 117, 126 |
| **Sound, but not yet before the owner** — governs work that has not started | **3** | 094, 095, 096 |

**The fifth row was missing until 2026-10-02**, found by the implementation role:
the table was headed "all fifteen" and totalled **twelve**. The first four shapes
classify *the record of decisions already reached*; 094/095/096 is the opposite
case and does not collapse into any of them — a judgment that is sound, has
**never been before `@nabbisen`**, and governs work not yet started. Adding it is
the correction; it is also the row that most needs his attention, because it is
the only one where acting early is still free.


**All fifteen accounted for. No design defect was found in any of them.** Every judgment the
implementation role made was right on the merits, and I would make the same call
again in each. That is the headline, and it was not the expected one.

**What the exercise actually found is a record problem, concentrated in two
shapes.** Five approvals do not show the owner engaging with a specific design
choice. Two RFCs were accepted before the review that would have shaped them —
RFC 126 caught and disclosed its own, RFC 117 did not and remains live and
unbuilt.

**Still with `@nabbisen`, from all three batches:**

1. **RFC 117's intent** — did the 06:54 acceptance cover the 08:31 narrowing?
2. **094/095/096's scheduling judgment**, before M2a starts.
3. **The prospective naming convention** — should an approval name the design
   choices a review settled? Five past gaps now argue for it, and RFC 116 shows
   the project has already done it right once.
4. **Whether the G11 citation-subject check above is wanted**, and if so as its own
   RFC.
