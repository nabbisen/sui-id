# RFC 131 — owner review, 2026-10-01

**By `@nabbisen`, who did not author this RFC.** His words: *"Reviewed.
Accepted."*

**This is a real independent design review**, in the ordinary sense of the word,
and the distinction from RFC 130 is deliberate and worth recording. On RFC 130 he
said only *"Accepted."* — an approval, not a review — and that RFC's header says
so and carries the architect's own judgement instead. Here he stated that he
reviewed it. The two records differ because the two acts differed.

## What the review acted on

The review followed two interventions by `@nabbisen` that changed the design, both
recorded in the RFC itself:

1. **He rejected the question, not the answer.** RFC 130's D5 asked him whether a
   release requires the complete matrix. His reply: *"What do I have to decide ?
   It's a kind of design for stable release cycles. First, define stages such as
   time to cut release, implementation and test completed etc. Then design which
   job(s) should be passed at the time."* The architect had handed the owner a
   policy question that a design was supposed to answer. RFC 131 exists because of
   that correction.

2. **He identified a missing stage.** *"In my mental model, one of the stages is
   RFC completion. Your model has S1 or S3 instead ?"* It was neither — measured,
   108 RFCs in `done/` close on individual dates, so RFC completion is distinct
   from both a single implementation handover and a milestone closing. The
   architect's five-stage draft was missing it.

3. **He then capped the complexity.** *"Be careful for the workflows not to be too
   complicated. I mean too many stages can bring confusion around management for
   release stability."* Acting on this produced the design that shipped: writing
   the five stages out showed three of them require the same gate set and differ
   only in evidence that already exists, so five gate lists collapsed to two
   levels and one sentence.

Each of the three improved the design. The third improved it most, and the
architect would not have found it — the five-stage model was already written.

## Standing of this record

Under `ROADMAP.md` S1c, ruled the same day, self-review is generally prohibited
with an owner-invoked exception. This RFC needs no exception: it was reviewed by
the owner, who is the role S1a assigns design review to.

RFC 131 is accepted on this review. Its D6 supersedes RFC 130's D5, which unblocks
RFC 130's implementation.
