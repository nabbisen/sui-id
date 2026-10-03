# RFC 094 M2a — closure review

**Date:** 2026-10-03
**Reviewed by:** the architect, which did **not** author RFC 094 (the previous
architect did) but **did** write the dispatches whose output this reviews, and
the consolidated criteria list it is measured against. **Not independent**, and
carried under `ROADMAP.md` R1's residual.
**Recommends closing M2a. Does not close it.** M2a closure is `@nabbisen`'s, and
the `ROADMAP.md` row must stay open until he says so.

## Level B

**`9e387e3f`, run `37106874150`, 26 jobs, 0 skipped, all green.**

Verified as Level B rather than assumed: all **23** entries in
`contracts/gate-inputs.toml`'s `[gates]` have a green job on that commit; the
three extra green jobs are A3.2, A3.4 and the changed-scope job. RFC 131 defines
Level B as "every gate in `[gates]`, green on one exact named commit. **Nothing
else**" — it names `workflow_dispatch` as the way to *obtain* it reliably, not as
a requirement of its form, so this push run qualifies on the definition's own
terms. A milestone closing is one of D2's three claims requiring it.

## The seven criteria

Consolidated 2026-10-03 from RFC 094's clause list and `ROADMAP.md`'s exit cell,
which had diverged in both directions.

| # | Criterion | Evidence |
|---|---|---|
| 1 | every converted Class-A path uses the approved seam | **G20**, which derives `ClassATx<'_, ID>` under `sui-id-store/src` and `comm`s it against the declared set |
| 2 | injected append failures roll back for every converted row | **24 of 24**, registry-backed; eleven added 2026-10-03 |
| 3 | every converted row has exactly-once evidence | **24 of 24**, registry-backed; twenty-three added 2026-10-03 |
| 4 | C15 is atomic | validate-first; rollback proven by injected failure |
| 5 | the structural gate passes over converted commands | **G20**, both registries satisfied |
| 6 | coverage matrix states conversion status, no unconverted command atomic | `check-audit-matrix-columns.py` fails a row claiming `A` that is not sealed-atomic and a row claiming `B` that is |
| 7 | raw DB access confined to `sui-id-store`, gate-asserted | `rusqlite` depended on by no other crate; e2e assertion passes |

**Criteria 1, 2, 3 and 5 are gate-asserted**, not asserted by a human reading
`grep` output. That matters more than the count: the four that could silently
regress are the four a gate now refuses.

## What I verified myself rather than accepting

- **The doubled-append check.** Backed up `registry.rs`, doubled
  `append_within_tx`, ran the suite: **25 failed, 297 passed**, message "exactly
  one event". Restored; 322 green; that file clean. Earlier reviews in this
  programme recorded that I was taking revert-and-rerun on trust; the final piece
  of M2a's evidence is not.
- **Both registries at 24**, G20's own output, A3.2, the 13 gate self-tests, and
  the `rusqlite` e2e assertion — each run by me.
- **Level B's coverage**, by comparing `[gates]`' keys against the run's green
  jobs rather than trusting the job count.

## What this review does not establish

- **It is not independent.** I wrote the dispatches that produced criteria 2 and
  3, and I wrote the consolidated list this measures against. The implementation
  role verified measurements separately on the packages, which is corroboration
  and neither approval nor independence. R1 keeps carrying it.
- **It says nothing about M2b**, which is untouched: 43 Class-A commands remain
  unconverted, no AST boundary gate, no authority switch.
- **The seam and structural criteria rest on a textual proxy.** G20 greps for
  `ClassATx<'_, ID>`; it proves the construct is present as written, not that no
  path evades it through an alias or macro. The script's docstring says so. The
  proxy's limit is disclosed, not closed.

## Recommendation

Close M2a, with `ROADMAP.md`'s M2a row marked **CLOSED 2026-10-03** and its
Evidence cell naming run `37106874150` on `9e387e3f` — the M1a/M1b pattern.

**RFC 094 stays in `accepted/`.** Its closure prerequisites are per-stage, and
M2b is not started, so no RFC moves to `done/` here. What closes is the
milestone, which unblocks RFC 095 (M3), RFC 100 (M2c), RFC 096-B and RFC 094 M2b.
