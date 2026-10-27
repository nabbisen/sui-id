# RFC 141 — the design review record

**What this file is.** The durable record RFC 141's
`Independent design review` field cites. It exists because that field
must say **what the review was and was not independent of**, rather than
leaving the word to imply a third party — the errata in
`rfcs/README.md` §*Errata: the `Independent design review` field*.

## Who reviewed it

**`@nabbisen`, by accepting it on 2026-10-28: *"RFC 141 is Accepted."***

`ROADMAP.md` §S1a places design review with **the accountable
approver, or an external architect engaged for it — never the
implementation role**. None was engaged for this RFC, so the acceptance
quoted above is the review.

**The architect wrote RFC 141 and did not review it.** §S1c prohibits
architect self-review by default and permits it only where it is asked
for, requiring the record to say so. **It was not asked for here**, and
this file does not claim otherwise — unlike
[RFC 140's record](../140-application-identity-the-operator-vouches-for/design-review-2026-10-27.md),
where he did ask in his own words and the record quotes him.

**If an external design review is wanted instead**, this field and this
file change and the RFC returns to the state before acceptance. The
architect has flagged that to him rather than assuming; nothing in RFC
141 is built until that is settled, because §3 below is the part most
worth a second pair of eyes.

## What acceptance covers, and the one thing it does not settle

RFC 141 §7 asks an open question — **whether `0.86.0` itself publishes
through the new workflow, or `0.87.0`**. The RFC's own recommendation is
`0.87.0`: build it in `0.86.0`, exercise the refusal fixtures, and let
the first real use be a release that is not already waiting on it.
**That recommendation stands unless `@nabbisen` says otherwise**; it is
a sequencing choice and nothing in the design depends on it.

## The part that carries the risk

**§3, the Level B gate.** RFC 131 D2 makes a release cut require every
`[gates]` lane green via `workflow_dispatch` on the exact commit being
tagged. Today a person checks that before publishing. If the publish
workflow does not check it, **automating the publish removes the checker
without replacing them** — the automation would weaken D2 rather than
strengthen it.

So the workflow must establish, for the commit the tag points at, that a
`CI` `workflow_dispatch` run on that exact SHA concluded `success` with
**0 skipped jobs**, that a `Security audit` run on the same SHA
concluded `success`, that the tag is annotated and on `main`, and that
the six version fields equal the tag — and must stop before minting a
token if any of those fails, with a negative fixture proving each
refusal.

**§6's ordering carries the other risk**: Trusted-Publishing-only mode
and revocation of the workstation token come **after** the first
successful workflow publish, so a failure of the new path falls back to
the old one instead of locking the project out of its own registry.
