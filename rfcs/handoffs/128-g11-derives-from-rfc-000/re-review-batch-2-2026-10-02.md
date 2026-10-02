# Re-review of the fifteen — batch 2 of 3

**Date:** 2026-10-02
**By.** The architect. Self-review under `ROADMAP.md` S1c, permitted because
`@nabbisen` asked for this work; where a verdict endorses a change to a design I
wrote, his reading is the only independence present.
**Scope.** Six RFCs in four documents: **117, 094/095/096, 110, 116** — the
governance-mechanism reviews, plus the one shared document whose RFCs are still
open. Ordered by consequence.
**Question.** Was the judgment right, and does the design still stand?

## How this batch differs from batch 1

Batch 1 found six RFCs where every judgment that took effect had reached
`@nabbisen` first, and one classification error of mine. **Batch 2 is not like
that.** Three of these four documents raise something, and two of them are
**live** — they bear on work that is not yet built, so unlike batch 1 these are
not settled matters being audited after the fact.

The running count of **visibility gaps** — a judgment adopted into text whose
approval names no specific choice — reaches **three**: 112 (partial), 122, and now
110.

## RFC 117 — the one that matters most, and it is live

**The judgment.** The review's closing recommendation, `:276`: *"I would recommend
not **Accepting the text as it stands**, because it claims the enforcement it
cannot give."* RFC 117 is itself the RFC about making owner decisions verifiable.

**It was right, and it was acted on in full.** This is the part to say first.
Commit `4c3c9e7` ("the claim narrowed and the work split") rewrote the RFC around
the review's distinction: a new section, *"The claim — evidence and enforcement are
not the same strength"*, states plainly that *"the first version of this RFC said
'the owner's decisions become verifiable' and used one sentence for two
properties. The design review separated them, and only one survives."* Evidence
survives an agent that disables the gate; enforcement does not. The review's
objection was not deflected — it reshaped the RFC.

**What is wrong is the record of it, and the irony is not lost.**

| | |
|---|---|
| The RFC's own approval note | *"the architect then amended the RFC and did not walk the folder"* |
| `Amended on` line | **absent** — there is none, despite a material amendment RFC 000 requires to be recorded |
| The approval sentence | *"RFC-117 is accepted."* — and, unlike 112, 118 and 122, it does **not** say "accepted on the amended text" |
| Order on the day | **Reconstructed 2026-10-02, and it is knowable — see the correction below.** Accepted 06:54, *then* rewritten 08:31. No second approval follows |

So **the one RFC in this project dedicated to making owner decisions verifiable
has an owner decision that cannot be verified from its own record.** I am not
being clever: that is the literal state, and it is the strongest argument for the
prospective convention batch 1 raised.

**Why this is live and not archaeology.** RFC 117's stages 1–3 are unbuilt — the
signed ledger does not exist, which is why it is one of the four RFCs blocked in
`accepted/`. The text he accepted is the text that will be built against.

### Correction — the order **is** reconstructable, and it is worse than I wrote

**Found by the implementation role, 2026-10-02.** I wrote that a reader "cannot
reconstruct whether he accepted before or after the narrowing." That was
under-evidenced: I compared two file timestamps where the commit history answers
the question outright. Reconstructed and verified independently:

| Time | Commit | What it did |
|---|---|---|
| 06:33 | `8741fa6` | *"five rulings of 2026-09-24; RFC 117 opened"* |
| **06:54** | `368d278` | *"RFC 117 accepted; design review **requested**, with an objection against it"* — **the acceptance, with the review not yet performed** |
| **08:31** | `4c3c9e7` | *"design review landed; **the claim narrowed and the work split**"* — the material rewrite |
| 09:44 | `a887813` | stage 0 implemented |
| 20:28 | `3dec640` | moved to `accepted/` — narrates the history; **adds no fresh approval** |

So it is not "same day, unclear order." It is **accepted, then substantively
rewritten, with no second approval recorded for the rewrite.** The text he
approved at 06:54 is not the text that now stands.

**This makes the finding sharper, not softer**, and it removes one of the two asks
I had offered.

**For `@nabbisen`, one thing, and it is not a reopening:**

**Confirm that your 2026-09-24 acceptance was intended to cover the narrowing that
followed it** — the rewrite separating evidence from enforcement. If yes, an
`Amended on` line recording the date and what changed closes the gap permanently.
If you would rather look at the narrowed text first, that is the better answer and
the RFC is unbuilt, so nothing is lost by it.

I had also offered "say you cannot recall and let the record say so." **That option
is now unnecessary for the order** — the repository establishes it. What only you
can supply is intent.

## RFC 094 / 095 / 096 — right, and it governs work not yet built

**The judgment**, `:255-257`: the 2026-08-26 correction re-pointing 096-B1's
prerequisite from M2a's *session-security conversion wave* to M2a's **runner
foundation** is *"the right fix for what this question actually found."*

**It is right.** A prerequisite should name what the dependent work actually
needs, and 096-B1 needs the sealed transaction seam, not the whole conversion
wave. Narrowing a prerequisite to its true dependency is strictly better than
over-requiring: it makes the ordering honest and stops a blocked RFC waiting on
work irrelevant to it. The amendment is recorded in 096's header
(`Amendment summary (2026-08-26)`), which is more than RFC 117 got.

**But it has not taken effect yet.** M2a is cycle C work (v0.80.0, from
2026-10-27). 094, 095 and 096 are three of the four RFCs blocked on unbuilt work.
**This judgment will govern the order in which that work happens**, and it was
made by the implementation role and endorsed by me — the two roles S1a says do not
settle design between them.

**Not an error; a flag.** The call is sound on the merits and I would make it
again. It belongs in front of `@nabbisen` *before* M2a starts rather than after,
because that is when correcting it is still free.

## RFC 110 — right, and the third visibility gap

**The judgment.** M1, `:72`: *"The label rule **should be** an allowlist, not a
denylist."* Also Item 5, `:92`: *"It cannot be checked meaningfully. Do not build
it."*

**Both right, and the first is demonstrably so.** The allowlist is RFC 110's D1
and is how G11 condition 14 works today. A denylist of forbidden phrasings would
have been unbounded and evadable by rewording; the closed six-label allowlist is
why the guard caught *my own draft* on 2026-10-01 when it cited archived RFC 018 in
a header. The "do not build it" call was equally right — the thing proposed could
not have been checked meaningfully, and not building it is why RFC 110 shipped a
narrow guard that works instead of a broad one that would not.

**The gap.** The approval reads in full: *"RFC 115 and 110 are accpepted."* It
names no choice. The allowlist-versus-denylist decision — an architectural fork
that shaped the gate — sits inside text he accepted, and the record does not show
it reaching him distinctly.

Same shape as 122, and like 122 **not worth reopening**: closed, correct, shipped,
and the guard has since proved itself against its own author.

## RFC 116 — right, and he ruled it explicitly

**The judgment.** Item 12, `:238`, self-labelled: *"Which inventory copy survives —
my view (a view, not a ruling). The TOML."* Plus three *"My recommendation"*
columns and *"My view: the parse belongs in Python."*

**Right, and the record is the best in either batch.** RFC 116 D1 reads *"One
source per registry: the TOML survives. **Ruled by `@nabbisen`**"*, and his
approval adds *"He ruled its two open questions on 2026-09-24 (D1, D3a)."* The
review labelled its own view as a view; the owner ruled; the RFC records who
ruled and when.

**This is the shape the other three should have had**, and it shows the
convention batch 1 proposed is not a new burden — it is something this project has
already done correctly, once, without being asked.

## Independent verification

[`re-review-batch-2-verification-2026-10-02.md`](./re-review-batch-2-verification-2026-10-02.md),
committed beside this document.

No discrepancy found in the quotes — every cited line exact, including the owner's
own typo in RFC 110's approval (*"accpepted"*), which they reproduced rather than
silently tidied. The visibility-gap tally of three was cross-checked against batch
1's corrected document for consistency.

Their contribution is the RFC 117 timeline above: they reconstructed the full
commit sequence where I had compared two timestamps, and the result **strengthened
the finding against their own prior work**. They also declined to second-guess the
094/095/096 scheduling judgment, saying they had no basis to — the right call,
and the same restraint they showed on RFC 127's completeness clause.

Corroboration, not approval.

## Batch 2 verdict

**No design defect found in any of the four.** Every judgment re-reviewed here was
right on the merits, and I would make the same call again in each case.

What the batch returns is three record defects of increasing seriousness: RFC 110
has a visibility gap in a closed matter; 094/095/096 has a sound judgment that
will govern unbuilt work and should be seen before it does; and **RFC 117 was accepted at 06:54 and materially rewritten at 08:31 with no
second approval — on a live, unbuilt RFC whose entire subject is making owner
decisions verifiable.**

Batch 3 is the last: 121, 123 and 126 — the three judgments the stage-0 audit
already knew about and that the architect already adopted.
