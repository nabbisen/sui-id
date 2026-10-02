# Re-review of the fifteen — batch 1 of 3

**Date:** 2026-10-02
**By.** The architect. **These are judgments about reviews of my own designs** —
self-review under `ROADMAP.md` S1c, permitted because `@nabbisen` asked for this
work. Where a verdict endorses a change to a design I wrote, his reading is the
only independence present, which is why he asked for every verdict and why the
ones that need his eye come first.
**Scope.** Six RFCs in five documents (102 and 103 share one): **115, 102/103,
112, 122, 118** — ordered by consequence, which here means *adopted and shipped to
users*.
**Question asked of each**, per the agreed method: **was the judgment right, and
does the design still stand?** Not "did it cross the line" — stage 0 part 2 already
established that all fifteen did.

## The finding that changes the shape of this exercise

Stage 0 part 2's conclusion was that every one of the fifteen contains a design
judgment the implementation role was not positioned to make. True, and I am not
retracting it. But it answers a different question from the one that matters here,
and re-reading the five against the record shows why.

**In all six RFCs, every design judgment that took effect reached `@nabbisen`
before it did.** Not one was adopted silently. The record shows two distinct
shapes:

| Shape | RFCs | What the record says |
|---|---|---|
| **He ruled the question explicitly** | 115, 102, 103 | *"He ruled all three open questions the same day, each as the independent design review recommended"* (115); *"who also ruled every open question as recommended"* (102/103) |
| **He accepted amended text, with the deciding change named in the approval** | 118 | *"the change that matters is D1's second-factor carve-out"* — and 118's own D1 **is** that carve-out, so the label and the content match |
| **He accepted amended text, but the judgments under review were not the ones named** | 112 | **Corrected 2026-10-02** — see below. Its approval names D2 and D3; the judgments re-reviewed here are D1 and D4 |

**So the risk stage 0 identified did not materialise as silent adoption in these
six** — but, as the correction below establishes, it did leave **two** RFCs where
the record does not show a specific judgment reaching him, not one. A design
judgment offered as labelled input, to an owner who then rules, is not the
implementation role deciding design — it is the advisory relationship working. The
RFC 115 review even titled its section *"Views on the three open questions
(`@nabbisen` rules; these are inputs)."* The field name above those packages was
wrong, which is RFC 128 D4's subject. **The decisions underneath it were not.**

That is the opposite of what I expected to find when I proposed this re-review,
and it is the more important result.

## Correction — RFC 112 belongs with 122, not with 118

**Found by the implementation role, 2026-10-02, and it is my error.**

This document vouched for two of RFC 112's judgments — *"I recommend no override
flag"* and *"one shared reader rather than a guard inside `run` only"* — and cited
its approval sentence as evidence they reached `@nabbisen`. Measured:

| | |
|---|---|
| RFC 112 **D1** | *Refusing is the rule, and there is no override flag* (`:59`) |
| RFC 112 **D2** | *"Fresh" means no application tables, not no row* (`:68`) |
| RFC 112 **D3** | *The check runs first, on a read-only connection* (`:76`) |
| RFC 112 **D4** | *One reader, one rule, for every caller* (`:87`) |
| The approval sentence (`:10`) | *"the changes that matter are **D2 and D3**"* |

**The approval names neither judgment this document vouches for.** D2 and D3 are
different decisions on different subjects. By the very test applied to RFC 122
below — *"a reader cannot tell from that sentence that this choice was settled"* —
D1 and D4 fail it.

The error is one I have made repeatedly in a different costume: I matched a
**shape** ("the approval names the deciding change") without checking that the
decisions named were the decisions under review. Seeing `D2 and D3` and reading it
as "the deciding changes are named" is the same reflex as counting grep matches
and calling the number a fact.

**One distinction worth preserving rather than flattening:** 112's approval names
*two* of its four decisions, so it is a partial gap; 122's names no specific choice
at all. Both are gaps; they are not identical.

**The design is unaffected.** D1's no-override-flag reasoning is sound — an
override would recreate the hazard the RFC removes, with a supported recovery
already existing — and D4's shared reader is real and landed. Both were confirmed
independently. What was wrong is the evidentiary claim, not the engineering.

## The verdicts that need a decision

### RFC 122 — stands, but the record does not show the placement choice reaching him

The review answered RFC 122's own open placement question by weighing three named
alternatives and picking one: *"**Recommend:** a router-layer, on named routes"*,
with *"**Why this beats** a response builder or a typed wrapper, concretely."* That
was adopted — `paths_carrying_no_store_layer()` in `r122_routes.rs` is the
router-layer design — and it shipped in 0.79.0.

**The design is right and I would choose it again.** A router layer makes the
guarded set enumerable, which is what lets
`the_routes_carrying_the_no_store_layer_are_exactly_the_expected_set` hold the set
in both directions. A response builder or typed wrapper would have put the
property in a place no test could enumerate. **The design stands.**

**What does not stand is the visibility.** 112 and 118 name the deciding change in
the approval sentence; 122's says only *"its design review corrected three things
this RFC claimed and found a sixth surface it had missed."* A reader cannot tell
from that sentence that a three-way architectural choice was settled inside the
amendment. He approved a document, and the record does not show he engaged with
that specific choice.

**No action on RFC 112 or RFC 122 itself** — it is closed, the design is right, and reopening
a correct decision to improve its paperwork would be the wrong trade. **The
decision for `@nabbisen` is prospective:** should an approval sentence be required
to name a design choice the review settled, the way 112's and 118's do? That is a
one-line convention, it is the difference between the two shapes in the table
above, and it would have made this verdict unnecessary.

## The three that stand without qualification

| RFC | The judgment | Verdict |
|---|---|---|
| **115** | *"`must_change` — my view: delete it"*, plus the second-administrator and throttle forks | **Right, and he ruled it.** The column had no model field and no reader; deleting it was cleaner than enforcing a flag nothing honoured. Migration 0043 drops it, and `r115_s3_must_change_is_gone_from_production_code` is a grep-proof over real production sources. The most judgment-dense package of the fifteen is also the one where the owner most explicitly decided |
| **102 / 103** | §6 settles several of the RFC's own open questions by picking a side — 30-minute expiry, refusing admin targets on the web, a code staying usable after a failed commit | **Right, and he ruled every one.** Expiry is 30 minutes in `103:116`, matching `DEFAULT_TOKEN_TTL` rather than inventing a constant. The review also declined where it should have (*"Five per administrator per hour. Outside this role's adjudication for deployment size"*), which is the behaviour that makes the rest trustworthy |
| **112** | *"I recommend no override flag"*; one shared reader rather than a guard inside `run` only | **The design is right; the visibility is not — see the correction above.** There is no override flag (`upgrade.md:105`, D1), and an override would have recreated the hazard the RFC removes with a supported recovery already existing. The shared reader (D4) is the stronger of the two: it is why every reader of the stored version obeys the same rule. **But the approval names D2 and D3, not these**, so 112 sits with 122 |
| **118** | M4's helper placement; Item 11 rejecting a login-page query parameter **and** a cookie read by `GET /admin/login` | **Right, and D1's carve-out is named in his approval.** Rejecting the query parameter is the one that matters: `r118_the_sign_in_page_ignores_any_query` now tests exactly the alternative the review refused. Item 16 also correctly deferred the backoff schedule as *"its own design and its own RFC"* rather than folding it in |

## Independent verification

[`re-review-batch-1-verification-2026-10-02.md`](./re-review-batch-1-verification-2026-10-02.md),
committed beside this document.

**The implementation role is the subject of this re-review, not a neutral party** —
these are their packages being judged. They verified it anyway, and the finding
they returned **widened** the criticism rather than narrowing it: from one
visibility gap to two. They also checked RFC 118 the same way *for comparison* and
confirmed it genuinely belongs in the "named in the approval" bucket, which is the
control that makes the 112 finding credible rather than merely contrarian.

Everything else they re-checked came back exact: all four quoted approval
sentences verbatim, 122's router-layer design confirmed structurally (eight
`SetResponseHeaderLayer::overriding(..., "no-store")` attachments in
`http/router.rs`, not a response builder), `DEFAULT_TOKEN_TTL` confirmed as
`Duration::minutes(30)` and reused at both mint sites rather than hardcoded, and
`r118_the_sign_in_page_ignores_any_query` run.

As always: corroboration, not approval.

## What I am not claiming

This batch says the decisions were sound and reached the owner. It does **not**
say the field name was right — RFC 128 D4 still stands, and these six are among
the twenty headers it covers.

Nor does it generalise to the remaining nine. **116, 117 and 110 are the
governance-mechanism reviews**, where the judgments are denser and the
"he ruled" record may be thinner, and **094/095/096 is the one shared document
whose RFCs are still open** — so a judgment there may still be load-bearing on
unbuilt work rather than settled by shipping. Those are batch 2, and I am not
front-running their verdicts here.

**Batch 1 verdict: six RFCs, five documents, no design defect found, and a
prospective convention for `@nabbisen` to decide — against a backdrop of two past
visibility gaps (112 and 122), not one.**
