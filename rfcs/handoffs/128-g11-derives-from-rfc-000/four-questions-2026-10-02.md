# The four questions from the re-review, answered

**Date:** 2026-10-02
**By.** The architect, at `@nabbisen`'s direction: *"Review it carefully by
yourself"* (Q1), *"where will it be applied to?"* (Q3), *"review it carefully by
yourself about whether it is actually reasonable (or there is any concern,
deficit, risk or debt in the future)"* (Q4). Q2 is approved and recorded.
**Standard applied, his:** *"finally clean, safe and secure, and robust and
sophisticated design."*

## Q2 — the 094/095/096 scheduling judgment: approved, recorded

*"Approved."*, 2026-10-02, **before M2a starts**. Recorded in
`rfcs/accepted/096-upstream-oidc-federation-validation.md` above the amendment it
governs, naming that the judgment was the implementation role's, endorsed by the
architect, and had never been before him until now.

That closes the fifth row of the close-out table — the one category where acting
early was still free, and it was acted on early.

## Q1 — RFC 117: which is better?

**Recommendation: re-accept the narrowed text. Do not record that the 06:54
acceptance covered it.** Not a close call, and the reason is structural rather
than procedural.

**What the two options actually are.**

| | |
|---|---|
| **A. Record that the acceptance covered the narrowing** | Costs nothing now. States as fact something neither of us knows |
| **B. Read the narrowed text and accept it** | Costs one reading of an unbuilt RFC. Produces a record that is true |

**Why A is not merely weaker but self-defeating.** The 08:31 rewrite was not a
clarification. The text accepted at 06:54 claimed *"the owner's decisions become
verifiable"* as one property; the narrowing **split it in two and established that
only one survives** — evidence survives an agent that disables the gate,
enforcement does not. Those are different claims about what the RFC delivers.
Recording that an acceptance of the first covered the second would be
reconstructing an owner decision from inference.

**RFC 117 is the RFC about making owner decisions verifiable.** Settling its own
history by inference would leave the project's instrument for verifiable decisions
resting on an unverifiable one. Against the standard — clean, safe, robust,
sophisticated — A fails on all four: it is not clean (it asserts what is not
known), not safe (it sets the precedent that a material rewrite inherits an
earlier approval), not robust (it survives only until someone reads the commit
log, as the implementation role just did), and not sophisticated (it is the
cheapest move available, chosen because it is cheap).

**Why B is cheap *now* and will not stay cheap.** Stages 1–3 are unbuilt. Today,
re-accepting is reading a document. Once the signed ledger exists, re-accepting
means re-accepting shipped work, and the question becomes expensive and awkward.
**This is the last moment at which B costs nothing.**

**What B looks like concretely**, so it is one reading and not a project:

1. You read RFC 117's section *"The claim — evidence and enforcement are not the
   same strength"* — the part the review forced.
2. If it is what you meant, the architect adds `**Amended on.** 2026-09-24` naming
   the narrowing, and an approval sentence for the narrowed text dated 2026-10-02.
3. **The record states the real sequence either way**: accepted 06:54, narrowed
   08:31, re-accepted on reading. The sequence is already in the repository; the
   only thing missing is the second acceptance.

If on reading it you do **not** agree with the narrowing, that is a more valuable
outcome than either option, and the RFC is unbuilt.

## Q3 — the naming convention: where would it apply?

**It applies to the `Approved by.` field, and its home is `rfcs/README.md`'s
template** — the document that defines what each header field should contain. Not
RFC 000: this is a convention about how an approval is *written*, not a rule about
who may approve, and RFC 000 is also the July document whose authority we have
agreed not to lean on.

**Can it be a gate as well?** Measured, rather than guessed. Rule tested: *an
approval must cite a decision by number, or record that the owner ruled.*

| Re-review verdict | RFCs | Rule's result |
|---|---|---|
| He ruled | 102, 103, 115, 116 | **all pass** |
| Approval named the change | 118 | **passes** |
| Visibility gap | 110, 121, 122, 123 | **all fail** — correctly |
| Accepted before review | 117, 126 | **both fail** — correctly |
| Partial gap | **112** | **passes, and should not** |

**Six of seven problem cases caught, five of five clean cases passed, one blind
spot.** RFC 112 cites `D2 and D3` — real decisions, just not the two under
review. The gate cannot close that without deciding *which* decision was the
deciding one, which is precisely the legislating RFC 110 forbids.

**So: document it in the template as a convention; the gate is available and
honest about its one blind spot.** My recommendation is the template first and the
gate only if he wants it — a convention that catches six of seven by being read is
worth more than arguing about the seventh.

## Q4 — the G11 citation-subject check: is it reasonable?

**Recommendation: yes, as an extension to condition 9 — not a new gate or lane —
with three conditions attached.** The concerns are real and I would rather state
them than have them discovered.

**What it would check.** At least one citation in `Independent design review`
resolves under this RFC's own `handoffs/<N>-` directory, unless allowlisted.
Simulated over all 26 RFCs carrying the field: exactly three allowlist entries
(095→094, 096→094, 103→102), RFC 126 passes, and **it would have failed RFC 126 in
its original state** — which is the defect it exists to catch.

### The concerns, deficits, risks and debt

**1. It checks location, not subject — and that is a proxy, not the rule.** A file
filed under `handoffs/126-…/` is *assumed* to be a review of RFC 126. Nothing
verifies it. A wrongly-filed document passes. **There is no mechanical fix**, and
the honest response is to say so in the docstring rather than let a reader believe
the gate proves more than it does.

**2. Is it legislating?** The question RFC 128 taught us to ask. My reading: no.
RFC 000 requires *an independent design review* **of this RFC**; a citation to a
review of a different RFC does not satisfy the field's plain meaning, so the gate
enforces the rule rather than supplying one. **But the *method* — filing location
— is this project's convention, not RFC 000's rule.** The gate would be encoding a
convention as a proxy for a rule. That is acceptable only if stated plainly, and
it is the risk I would watch.

**3. Allowlist debt, and it compounds.** `contracts/rfc-policy.toml` would then
carry a **third** closed allowlist beside `[archive_citations]` and
`[historical_rfc_mi]`. Each is a maintained exception list, and exception lists
are where rules go to quietly stop meaning things. **Condition: every entry
carries a reason**, as `contracts/contract-paths.toml` already requires, with a
blank reason failing.

**4. It is a gate for a defect that happened once and was self-caught.** RFC 126's
miscitation was found and disclosed by the architect without any gate. Twenty-one
gates exist and the complexity caution has been given twice. **The counter, which
I find persuasive:** it was caught by diligence, not by anything repeatable, and
the failure is silent — a wrong citation resolves, so condition 9 reports green.
Cost is a few lines inside an existing checker and no new lane.

**5. Future debt.** If reviews ever live outside `handoffs/`, the rule breaks and
the allowlist absorbs the breakage until it means nothing. **Condition: the rule is
revisited if a review is ever legitimately filed elsewhere**, rather than
allowlisted away.

### The three conditions

1. Extension to **condition 9**, not a new gate or lane.
2. Every allowlist entry carries a **reason**; a blank reason fails.
3. The docstring states that **location is a proxy for subject**, so no future
   reader mistakes it for a stronger guarantee.

**On the standard:** this is the one of the four answers where "sophisticated"
argues for *less* than was proposed. A gate that honestly checks a proxy, says so,
and costs no new lane is sophisticated. The same check presented as proving the
review is about this RFC would not be.
