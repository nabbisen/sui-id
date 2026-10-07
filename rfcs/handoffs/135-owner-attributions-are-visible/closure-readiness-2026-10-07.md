# RFC 135 — closure readiness

**Date:** 2026-10-07. **Recommendation: ready to close, pending your approval
and one Level B run.** Found while sweeping `accepted/` for RFCs whose work is
finished but whose status says otherwise — RFC 135 has been sitting in
`accepted/` while G16 runs on every commit.

**Independence.** The architect wrote RFC 135 and this assessment, so it is not
independent. RFC 000 provides for that: the accountable owner signs off, and
`@nabbisen` holds that role here.

## The four prerequisites, measured

RFC 135's own words: *"`contracts/gate-inputs.toml` records this RFC as G16's
owning RFC; this RFC carries the Gate Matrix heading A3.4 condition 7 requires;
RFC 117 is archived as superseded with no header in `proposed/`, `accepted/` or
`done/` citing it as authority; and G16's own files cite this RFC rather than
RFC 117."*

| | Prerequisite | Result |
|---|---|---|
| 1 | `[gate_owners]` records G16's owning RFC | **met** — `contracts/gate-inputs.toml:70`, `G16 = "135"` |
| 2 | This RFC carries the Gate Matrix heading | **met** — `## Gate Matrix lane owned by RFC 135` at `:104`, with the G16 lane row at `:114` |
| 3 | RFC 117 archived, nothing citing it as authority | **met** — its file is in `rfcs/archive/` (see below); the remaining mentions in `accepted/135` and `done/116` are body prose explaining the supersession, not header citations. **G11 condition 15 enforces exactly this rule and passes** |
| 4 | G16's own files cite RFC 135, not RFC 117 | **was NOT met — fixed today, see below** |

## Prerequisite 4 was unmet, by one line

The archived file is `rfcs/archive/117-*.md` — written as a glob deliberately.

**A G16 observation, found by writing this document — and the glob above is
the workaround, not a style choice.** RFC 117's archived filename happens to
contain both halves of what the pattern looks for, so **a sentence consisting
of nothing but that path matches it and fails the gate.** **I hit it three times writing this one section:** once giving the path as
evidence, again when the first attempt to explain the problem spelled the path
out, and a third time when the explanation itself named both halves of the
pattern in one sentence. **G16 cannot be described in prose that G16 accepts**,
which is worth knowing before someone tries to document it.
**That is a false positive on a path**, not on a claim. It is harmless — the remedy is a glob or a baseline line — and
RFC 135's own text already calls the patterns *"deliberately generous"*, with
the reasoning that a false positive costs a line in the baseline while a false
negative is the failure being prevented. **Recorded rather than fixed**:
narrowing the pattern to exclude paths would trade a known-cheap false positive
for an unknown false negative, which is the wrong direction for this gate.

**`contracts/owner-attributions.toml` line 1 read:**

```
# RFC 117 stage 0 (G16): the owner-attribution census.
```

**The file G16 actually reads still pointed at RFC 117.** The other two were
re-homed correctly — the baseline says *"RFC 135 (was RFC 117 stage 0)"* and
the script's baseline header names RFC 135. One file missed in a three-file
change.

**Counting was not enough to find it, and nearly hid it.** A grep for `RFC 117`
across G16's files returns 2, 5 and 18 hits, and almost all of them are
legitimate history: *"the archived RFC 117 §Summary, which measured them"*,
*"RFC 117 proposed signatures from a key no…"*, *"The first draft of RFC 117
used a narrow pattern…"*. Only reading them shows that exactly one is a
**self-identifying header** rather than a historical reference.

Now reads `# RFC 135 (was RFC 117 stage 0) (G16): …`, matching the baseline's
phrasing. G16, G11 and G18 pass.

## What remains before it can move to `done/`

1. **Your sign-off**, which RFC 135's header reserves to you, as RFCs 134 and
   137 did.
2. **A Level B commit carrying this fix.** `5ed86e9` is Level B but predates
   it. This change touches `contracts/owner-attributions.toml`, which is not in
   the Rust lanes' `paths`, so **its own CI run will be path-filtered and will
   be Level A** — the same trap that made me reject `4f93823` for RFC 137. The
   next commit touching `crates/**` gives a Level B run containing it; 096-A
   stage 3a will do that when it returns.

**I am not closing it on a Level A run**, and I am not asking you to approve a
closure whose evidence does not yet exist. **This is the readiness record; the
closure review follows once there is a Level B commit to cite.**

## Risk

**Low, and it was lower than it looked.** G16 has been working correctly
throughout — the defect was a comment naming the wrong RFC, not a behaviour. No
attribution was missed, no baseline entry was wrong. What was broken is
traceability: a reader of `contracts/owner-attributions.toml` would have gone
looking for an archived RFC to understand a live gate.
