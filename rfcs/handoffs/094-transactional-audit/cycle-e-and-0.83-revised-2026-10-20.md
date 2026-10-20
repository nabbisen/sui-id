# Cycle E and `0.83.0` — a revised recommendation

> ## Whose turn it is: **not the architect's.** One decision, restated.
> This **replaces §D of [`cycle-e-proposal-2026-10-20.md`](cycle-e-proposal-2026-10-20.md)**.
> Everything else in that proposal stands, including the RFC-numbering
> repair in its §B.

**Written.** 2026-10-20 JST, by the architect.
**Prompted by.** *"I wonder if we should scope 0.83, too, now."* Scoping it is
cheap, and doing it showed the earlier recommendation was wrong.

## A — the two rulings being recorded

**`0.82.0` is authorized** — a dependency and security maintenance cut, on the
measured severity in
[`release-0.82.0-scope-2026-10-20.md`](../096-upstream-oidc-federation/release-0.82.0-scope-2026-10-20.md)
§B rather than on nine raw advisory counts.

**The pre-publish audit step is approved**, with a constraint recorded
verbatim and attributed, because it is a standing rule rather than a remark
about this one change — **`@nabbisen`, 2026-10-20**:

> *"be careful not to damage development cycles and productivity. I mean it
> will harm the project if some check for the release gate only is processed
> at each RFC, each implementation or each review."*

Implemented accordingly in `docs/src/contributing/release-process.md`: one
reading of an existing lane, at the cut, explicitly **not** a `[gates]` lane,
**not** part of Level B, and **not** evidence any RFC, dispatch,
implementation round or review must produce. A3.4 passes — the audit lane is
not in `[gates]`, so writing its command there does not restate one.

**Taken more broadly than this one change, which is how it was meant.** The
constraint is a standing limit on what a round is made to carry: a check that
pays off only at a release belongs at the release, and the burden of proof is
on anything that would charge it per round.

## B — scoping `0.83.0` showed §D of the cycle E proposal was wrong

That proposal said *"M2c and M3 are the only remaining milestones that do
not"* sit behind M2b — and then recommended M2b without weighing M3 as a
candidate at all. Naming it and not considering it is worse than omitting it.

**Measured 2026-10-20:**

| | M3 — RFC 095 | M2b — RFC 094 |
|---|---|---|
| RFC status | **Accepted** | **Accepted** |
| Entry gate | **open** — six of six conditions met, discharge approved 2026-10-05 | n/a |
| Landed | stage 1 `78a2dec`, stage 1b `99f83bb` | — |
| Remaining | **stages 2 and 3, undispatched** — the remaining field rules, the authentication/grant matrix, browser presentation, response/error assertions | **77 planned rows** (G17: 102 rows, 25 sealed) |
| Blocked on | nothing | nothing |
| Needs a freeze exception? | no | no |
| Needs an RFC written? | no | no |
| Fits one release? | **yes** | **no** |

**M2b is a programme, not a release.** The earlier proposal admitted this —
*"77 planned rows is not a cycle, it is a programme"* — and then recommended
it as the cycle anyway, promising to decompose it later. For the question
"what is `0.83.0`", that is the wrong shape of answer.

**M3 finishes something.** Two stages against a normative source that already
exists (`metadata-validation.md`), on an entry gate already open, closing a
milestone rather than opening one.

## C — the revised recommendation

**`0.83.0` = RFC 095 M3 stages 2 and 3.** **M2b becomes the programme that
follows**, decomposed into tranches across `0.84.0` and beyond.

This does not weaken the earlier argument that M2b is the critical path —
096-B2, M4-B, RFC 097/M5, M6 and M7 all still sit behind it, and nothing else
unblocks them. It changes only the order and the framing: finish the
milestone that is two stages from done before starting the one that is a
programme, and stop calling a programme a cycle.

**What `0.83.0` would not do: close M3.** Closure needs more than stages 2
and 3, and the extra parts are yours:

- **Every `legacy_unstamped_dynamic_candidate` needs a durably recorded
  disposition** — disablement, explicit legacy acceptance, or a separately
  audited metadata correction — and *"a 'zero candidates' claim is not
  accepted, because the historical best-effort audit cannot prove
  completeness."*
  **The label exists only in the RFC; no code emits it.** So the mechanism
  that produces the candidate list is itself unbuilt, and it is a
  prerequisite of the disposition, not a consequence of it. If M3 closure is
  wanted, that mechanism is a stage of its own and I should plan it in.
- **Acceptance of the evidence**, on the architect's assessment of it.

So: `0.83.0` ships stages 2–3; M3 *closes* once the candidate mechanism is
built and you have dispositioned what it reports.

## D — the sequence, if this is authorized

1. **`0.82.0`** — the lockfile update, authorized. Small, independent.
2. **`0.83.0`** — M3 stages 2 and 3. I bring the stage plan before dispatching.
3. **`0.84.0`+** — M2b, as staged tranches in the shape 096-B1's plan took.

One open dispatch at a time throughout, so these are sequential.

## E — accepted

**`@nabbisen`, 2026-10-20: *"Your recommedation is accepted."*** So `0.83.0`
is M3 stages 2 and 3, and M2b is the programme after it, in tranches from
`0.84.0`. The sequence in §D is the schedule of record until it is revised.

**Order of work from here:** `0.82.0`'s preparation is dispatched first — it
is authorized, independent and small — and the M3 stage plan is written
alongside it, since planning is not a dispatch and does not consume the
one-open-dispatch slot.

## F — what was asked

**One decision: is `0.83.0` M3, or M2b's first tranche?** My recommendation is
M3, revised from the earlier M2b and for the reason in §B — not because the
critical-path argument changed, but because a release should be something
that finishes.

The embeddable and store-boundary decisions in the earlier proposal's §C are
untouched and still open; neither is engaged by this recommendation, because
the freeze holds either way.
