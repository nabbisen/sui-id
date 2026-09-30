# RFC 128 — security review

**Reviewer.** High-capability model, security-reviewer role — **and the author of
this RFC.** Not independent, and not recorded as such.
**Why this document exists at all.** G11 condition 9 refused RFC 128's move to
`accepted/` because its security review is Required and it carried no
`Independent design review` field. **The RFC that removes a retired document's
sentence was blocked by that sentence.** Satisfying it was the only way to land
the fix for it, and that is the clearest evidence available for why D1 is right.
**Carried by the owner under `ROADMAP.md` R1**, whose residual column exists for
design judgements no role but the author can assess.

## What was reviewed

Whether removing RFC 018's additions from G11 condition 9 weakens the security
posture RFC 000 establishes.

## Finding: it does not, and the removals are each narrow

| RFC 018 addition | Removing it costs |
|---|---|
| the `N/A` prohibition | Nothing. RFC 000 requires a reviewer and a durable reference; an RFC that records neither fails on the requirement itself, not on the literal string `N/A`. |
| the mandated field **name** | Nothing security-relevant. A reader needs to find who reviewed and what they checked; the key's spelling is not the property. |
| **the definition of independence** | This is the one that matters, and removing it costs **nothing that was real**. RFC 018 defined independence as "the reviewer did not author". Under that definition every "independent" review in this repository from July and August 2026 was performed by **another agent of the same vendor** — `ROADMAP.md` §S1 records exactly this, and says of it: *"a change authored, implemented and approved by one party has had no review at all."* The definition was satisfied on paper while the property it names was not. Deleting it removes a label, not a safeguard. |

## What is deliberately not decided

**Whether an author may review their own RFC.** D3 leaves it to `@nabbisen`, and
this review does not smuggle an answer in. What it observes is narrower: the
existing R1 residual mechanism — *"design judgments no role but the author can
assess, carried explicitly by the owner"* — already handles this case, has two
entries, and was approved long before this RFC. **Using a mechanism that exists
is not the same as deciding a rule that does not.**

## Residual risk of the change

**One, and it is real:** with RFC 018's definition gone and RFC 000 silent, the
word "independent" in twenty RFC headers has no stated meaning. Until
`@nabbisen` rules, those headers assert a property that is now undefined rather
than one that is defined and unmet. **That is worse in one narrow sense — an
undefined claim cannot be checked — and better in the sense that matters: it no
longer reads as a satisfied requirement when it was not.** D4 exists to close it,
and D4 is explicitly gated on his ruling.

## Verdict

**Accept.** The removals are narrow, the safeguard they appeared to provide was
not operating, and the one residual is named and gated on the owner rather than
left implicit.
