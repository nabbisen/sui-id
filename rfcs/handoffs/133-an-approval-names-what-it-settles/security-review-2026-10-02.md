# RFC 133 — security review, 2026-10-02

**By the architect, who authored this RFC, and therefore not independent.**
Written **before** acceptance rather than after, so it is input to `@nabbisen`'s
decision rather than paperwork following it. Carried under `ROADMAP.md` R1's
residual; S1c permits this only because he has asked for the architect's own
assessment on this arc throughout.

## What is at risk

RFC 133 touches **G11** — the gate that checks every RFC in the repository. A
weak change here does not fail loudly; it makes a green gate mean slightly less
than a reader thinks, across all 130 RFCs at once. That is the lens for all three
findings below.

## Finding 1 — my own simulation silently skipped a category

D2 claims *"simulated across all 26 RFCs carrying the field… exactly three
allowlist entries."* Checking the simulation rather than the claim: it enumerated
citations matching `handoffs/<NNN>-` and **skipped any RFC whose citations matched
none** — so an RFC citing a review filed anywhere else was never tested, and the
rule as written would fail it.

**Measured: exactly one such RFC exists — `archive/018`, which carries the field
with no link at all.** It is archived, and condition 9 applies to Accepted RFCs
with `Security review: Required`, so it is outside scope either way.

**So the claim survives, and the method that produced it did not.** The number
three is right by luck of the corpus, not because the simulation covered the
space. **Required of the implementation: the rule states its scope explicitly** —
the same set condition 9 already governs — rather than inheriting it from whatever
the simulation happened to enumerate.

This is the fifth time this session a measurement of mine has been narrower than
the claim it supported. I would rather find the sixth myself than have it found
for me.

## Finding 2 — the allowlist has the same reach as the rule, and less scrutiny

D2's allowlist lets an RFC cite a review of a different RFC. That is exactly the
condition the rule exists to forbid, made legal by a line in
`contracts/rfc-policy.toml`.

**The gate cannot tell a legitimate shared review from an entry added to make a
red gate green.** Nothing in the mechanism distinguishes 095→094 (a genuine
three-RFC review) from a future entry added during implementation because a
citation was wrong and the deadline was close.

D2a(2) requires a reason per entry, which helps a reader but not the gate — a
reason can be written for a bad entry as easily as a good one.

**Required: an allowlist entry is a reviewable act, not an implementation
detail.** Adding one is declared in the package that adds it, with the shared
document named, and is reviewed as a change to the contract — never landed
silently alongside the change that needed it. The same should be said of
`[archive_citations]` and `[historical_rfc_mi]`, which have carried this exposure
longer without it being written down.

## Finding 3 — D1's convention may not reach the people it is for

D1 puts the convention in `rfcs/README.md`'s template. The people who write
approvals are `@nabbisen` and the architect, who do read it. But RFC 132 D4 has
just made `CONTRIBUTING.md` the document that tells a contributor *"a change to
behaviour, a contract, or a security property goes through an RFC"* — and a
contributor following that paragraph into the RFC process will not necessarily
open the template.

**Recommended, not required:** the RFC-lifecycle paragraph in `CONTRIBUTING.md`
gains a clause pointing at the template for what an approval must contain. One
sentence, and it closes the gap between the document that sends people into the
process and the document that defines it.

## What I checked and found sound

- **D1a's refusal to build a gate is correct, and I tried to break it.** The
  obvious counter is "surely a gate catching six of seven beats a convention
  catching six of seven." It does not: a gate is satisfied by citing *a*
  decision, and RFC 112 shows a well-intentioned author citing the wrong ones. A
  gate would convert that from a visible gap into a green tick. **The absence of
  a gate here is the design, not a shortfall in it.**
- **D2's restriction to an extension of condition 9** keeps the Gate Matrix at
  twenty-one lanes and adds no network call, no nondeterminism and no new
  failure mode to CI.
- **D2 would have caught the one defect it is for.** RFC 126's original field
  cited only RFC 123's review; the rule fails that. This is not a gate built for
  a hypothetical.
- **Neither decision touches runtime, data or credentials.** The security
  surface is the integrity of the governance record, which is the subject rather
  than a side effect.

## Verdict

**Accept, with Findings 1 and 2 as required changes to the implementation** —
scope the rule explicitly, and make an allowlist entry a reviewable act — and
Finding 3 as a recommendation.

Neither required change alters a decision in the RFC. Both close a gap between
what the RFC says and what an implementer would otherwise be free to do, which is
the only thing a security review of a governance change can usefully do.
