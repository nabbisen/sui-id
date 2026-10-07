# RFC 135 — closure review

**Date:** 2026-10-07. **Recommendation: close to `done/`, Status Implemented.
Awaiting the owner's sign-off, which RFC 135's header reserves.**

**Independence.** Written by the architect, which wrote RFC 135 and its one
fix. Not independent. RFC 000 provides for that case: the accountable owner
signs off. The implementation role verified the four prerequisites separately
(`.git-exclude/reviewed/rfc-135-closure-readiness-verification-2026-10-07.md`);
that is corroboration, not independence.

## The four prerequisites

RFC 135's own words: *"`contracts/gate-inputs.toml` records this RFC as G16's
owning RFC; this RFC carries the Gate Matrix heading A3.4 condition 7 requires;
RFC 117 is archived as superseded with no header in `proposed/`, `accepted/` or
`done/` citing it as authority; and G16's own files cite this RFC rather than
RFC 117."*

| # | Result | Evidence |
|---|---|---|
| 1 | **met** | `contracts/gate-inputs.toml:70` — `G16 = "135"` |
| 2 | **met** | `## Gate Matrix lane owned by RFC 135` at `:104`; the G16 lane row at `:114` |
| 3 | **met** | RFC 117's file is in `rfcs/archive/`; remaining mentions are body prose, not header citations. **G11 condition 15 enforces this rule and passes** |
| 4 | **met, after a fix** | `contracts/owner-attributions.toml` line 1 now names RFC 135 |

**Prerequisite 4 was not met when I started.** Line 1 read *"RFC 117 stage 0
(G16): the owner-attribution census"* — the policy file G16 reads still pointed
at the archived RFC. The baseline and the script had been re-homed; one file in
three was missed.

**Counting would not have found it.** `RFC 117` appears 2, 5 and 18 times
across G16's files, and nearly every instance is legitimate history — *"the
archived RFC 117 §Summary, which measured them"*, *"the first draft used a
narrow pattern"*. Only reading them shows exactly one was a self-identifying
header rather than a reference.

## Level B

**`2526313`, run `37582343136` — 27 jobs, 0 skipped, all green.** Verified to
cover all **24** entries in `[gates]`, not inferred from the count. The
prerequisite-4 fix is in that commit: `git show 2526313:contracts/owner-attributions.toml`
line 1 reads `# RFC 135 (was RFC 117 stage 0) (G16): …`.

**Two earlier green runs were rejected for this purpose.** `4f93823` and the
commit carrying the fix itself would both have been **Level A** — the fix
touches `contracts/owner-attributions.toml`, which is outside the Rust lanes'
`paths`, so its own run is path-filtered. RFC 096-A stage 3a touches
`crates/**` and carried it into a full-matrix run. **That the evidence had to
wait for an unrelated commit is a property of path filtering worth knowing**:
a change to a governance contract cannot produce its own Level B.

## Two findings recorded rather than fixed

**1. G16 cannot be described in prose that G16 accepts.** RFC 117's archived
filename contains both halves of what the attribution pattern looks for, so a
sentence consisting of nothing but that path fails the gate. Writing the
readiness assessment tripped it three times: the path as evidence, the
explanation that spelled the path out, and the explanation that named both
halves of the pattern in one sentence.

**Left alone deliberately.** RFC 135 already calls the patterns *"deliberately
generous"* and reasons that *"a false positive costs a line in the baseline; a
false negative is the failure being prevented."* Narrowing the pattern to
exclude paths would trade a cheap known false positive for an unknown false
negative — the wrong direction for this gate, by its own stated design.

**2. A local G16 pass does not prove a baseline edit was the intended one.**
The implementation role noticed this: a local run reports it is *"not a GitHub
event run"* with no base revision, so it cannot show what a baseline edit
changed. **The diff is the evidence.** Both baseline edits made today were
verified by diffing against a copy taken first — once confirming exactly two
added lines, once confirming three path moves and one addition with two hashes
unchanged across the move. That practice was being followed but was not written
down anywhere.

## What this RFC actually delivered

**No code.** RFC 135's own implementation prerequisite says *"No new code — G16
already exists and passes; the work is re-homing."* What it delivered is that
a live gate now names a live RFC as its owner, in all three of its files, and
that RFC 117's unbuilt signing-key half is disposed of honestly rather than
left open.

**The one defect found at closure is exactly the kind this RFC exists to
prevent** — a governance artefact pointing at the wrong authority. It was found
by checking the prerequisites against the files rather than assuming a
three-file change had been complete.

## Recommendation

**Close to `done/`, Status Implemented**, on the owner's sign-off.
