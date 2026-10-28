# RFC 141 — the design review record

**What this file is.** The durable record RFC 141's
`Independent design review` field cites. It exists because that field
must say **what the review was and was not independent of**, rather than
leaving the word to imply a third party — the errata in
`rfcs/README.md` §*Errata: the `Independent design review` field*.

## Who reviewed it

**`@nabbisen`, on 2026-10-28: *"RFC 141 is Accepted."*** — and under
this project's own ruling that **is** the design review, not a
substitute for one. `ROADMAP.md` §S1a's table, ruled 2026-10-01:

> | Design review | **`@nabbisen`, or an external architect he engages. Not the dev team.** |

**The architect wrote RFC 141 and is therefore not independent of it**,
and did not self-review it: §S1c permits architect self-review only
where it is asked for, and it was not asked for here. Independence of
the author is what that field is about; it is not a question about
whether the approver considered the design.

**No external architect was engaged.** §S1a makes engaging one the
approver's option — *"I will ask some external architect to help us as
a temporary act of exception if necessary"* — not a precondition.

### Two earlier revisions of this file were wrong, in opposite directions

Recorded because a governance record that has been corrected twice
should say so rather than present its third attempt as its first.

1. **First**: *"the acceptance quoted above is the review."* True in
   substance but reached by the architect's own inference rather than
   by citing §S1a — stating as settled something the approver had not
   been shown to have said.
2. **Then**, when the approver asked whether he had reviewed it: the
   file was rewritten to say **no review had been recorded and whether
   the acceptance included one was unknown**. That was worse. §S1a
   already answers it, and the wording implied the approver might
   accept without considering — which the ruling contradicts and
   nothing in the record supports.

The error both times was the architect reasoning about the governance
rule instead of reading it. §S1a is four lines and a table.

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
