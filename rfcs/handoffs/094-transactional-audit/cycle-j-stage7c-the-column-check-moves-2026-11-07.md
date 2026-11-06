# Cycle J stage 7c — the column check moves onto the AST

> ## Whose turn it is: **the implementation role's.**
> You proposed this in stage 7 §5(a) and the review accepted it. This is
> the dispatch, written **before** the work rather than announced after
> it — stage 7b was built on a review sentence that said "dispatched"
> when nothing had been, which was my error and is not repeated here.

**Dispatched.** 2026-11-07, by the architect.
**Stage.** 7c. §D of
`rfcs/handoffs/094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md`.
**Why it exists.** `scripts/rust_registry.py`'s own docstring: it reads
the sealed-command registry *"without an AST… nothing here can see a
type."* The column check is built on it, so it carries the exact defect
RFC 094 exists to end.
**Scope.** Move the column check into `cargo xtask audit-structure`.
**Not** the actor-cell contract's wording (§4). **Not**
`rust_registry.py`'s retirement (§5).
**Baseline.** The tip of `main` when you start — `git log -1`, record it.
**Status check.** RFC 094 is **Accepted**. Building is permitted.

---

## 1 — what moves, measured

`scripts/check-audit-matrix-columns.py`, 317 lines, three properties
plus a schema rule, all from its own docstring:

- **class** — a row claiming A names an event carried by a sealed
  descriptor of class `Atomic`; a row claiming B names none; **and
  every sealed `Atomic` descriptor has a row claiming A.** Both
  directions. The reverse is what makes *"every Class-A command has a
  row"* true rather than hoped, so do not drop it.
- **actor** — for a sealed row: `ActorRequirement::None` is a cell
  starting with an em dash; `Optional` names an actor **and** says
  `none`; `Required` names an actor and no `none`.
- **step_up** — the note cell says `step_up` exactly when the descriptor
  carries a required `step_up` attribute.
- **schema** — a table line whose first cell names an event **under any
  other header** is a violation, so a new table shape cannot hide rows.

**No hardcoded counts.** The docstring's *"exact over the five rows that
carry it"* is prose; there is no literal `5` in the code. Do not
introduce one.

## 2 — the descriptor side is the point: read it with `syn`

`xtask` already parses `EventDescriptor` statics — `coverage.rs` takes
the `name` field. This stage needs the **same statics'** `class`,
`actor`, and the `step_up` entry inside `attributes: &[…]`.

**That is the whole justification for the move.** A port that shells out
to `rust_registry.py`, or re-implements its text scan in Rust, delivers
nothing: the defect is that a text read cannot see a type, and both of
those are text reads.

## 3 — the matrix side stays text, and that is correct

A markdown cell *is* text; reading it as text is not the defect. Stage
7b already built header-keyed table parsing in `coverage.rs` and takes
the first cell under an `Event name` header. This stage needs the
**full header→cell map** the Python keeps (`cells: dict[str, str]`,
lower-cased header → cell text).

**One interaction to get right, because a port is where it gets lost.**
After 7b, coverage takes names **only** from `Event name` columns, so a
name-shaped cell under a different header is *ignored* by coverage. The
column check's `schema` rule says that same case is a **violation**.
Both now live in one tool. **Do not let the coverage rule silence the
schema rule**: a name under the wrong header must still fail, and you
should have a fixture proving it fails rather than passing quietly as
"not a coverage entry".

## 4 — port the actor heuristics verbatim; do not improve them here

The actor check reads prose shapes — *"starts with an em dash"*, *"names
an actor and says `none`"*, as in `"admin user id; none for the CLI"`.
That is fragile, and I am recording it as an observation rather than
work: a cell contract expressed as prose heuristics will mis-read a
reworded cell.

**This round ports it as-is.** Changing the contract is a different
decision with a different blast radius, and mixing it into a port
destroys the equivalence evidence that makes the port reviewable.
**Say in your return whether you think the contract should change**, and
it becomes its own item.

## 5 — what to delete, and what to leave

**Delete `scripts/check-audit-matrix-columns.py`** in the same change,
and stop `scripts/check-audit-matrix.sh` invoking it (`:122`), updating
that script's header comment (`:14`). Same principle RFC 094 applied to
G17: one file must not hold a second definition of true.

**Leave `scripts/rust_registry.py`.** Its retirement is a separate
question with its own consumers —
`scripts/tests/check-gate-matrix-fixtures.sh` lists it in the
`audit-desync` and `audit-in-sync` fixtures, and
`scripts/tests/test_audit_matrix_columns.py` tests the script you are
deleting. **Say what you did with that 19-case suite** (§6), and do not
quietly leave a test file testing a deleted script.

**G13 stays registered.** Its remaining step is the string comparison,
the vocabulary-drift diagnostic. Its retirement is still
`@nabbisen`'s amendment, unchanged by this round.

## 6 — the equivalence bar, and the fixture trap you already hit once

**Port all 19 cases** from `scripts/tests/test_audit_matrix_columns.py`
(373 lines) with the same assertions, as you did for G17's 26. State per
property — class forward, class reverse, actor, step_up, schema — one
case where the new implementation fails.

**And the A3.2 fixtures are the trap.**
`scripts/tests/check-gate-matrix-fixtures.sh:33-34` lists all three
scripts for the `audit-desync` and `audit-in-sync` fixtures. Those
fixtures exist to prove a desynchronised matrix is **caught**. After
this move, a column desync is caught by G23, not by G13's chain.

**So the fixtures must be updated to run whichever gate now catches it,
and must still fail on the desync.** At stage 6b you found a negative
fixture that silently stopped being negative once the real scope
changed; this is the same shape, visible in advance. Prove
`audit-desync` still fails and say which gate fails it.

## 7 — constraints

- **G23's command does not change.** It already receives `--matrix` and
  `--root`; the column check needs no new input. So no `[gates]` edit,
  no RFC 094 table row edit, no A3.4 byte-match churn. **If you find you
  need a new flag, stop and say so** — that would mean two files change
  in lockstep.
- `contracts/workflow-template.toml:78` and the generated comment at
  `.github/workflows/ci.yml:414` both describe G13 as ending in a Python
  step. Update the template; **regenerate `ci.yml`, never hand-edit it.**
- `contracts/README.md`'s row for `audit-coverage-matrix.md` already
  names G23 in "Read by" — check whether anything else needs it.
- No gate command restated in prose (A3.4).
- No `audit.yml` run, no Level B claim. Both readings stay with me.

## 8 — `CHANGELOG`

**None, and say so.** A check moves between gates; nothing an operator
or a library consumer observes changes.

## 9 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage7c-the-column-check-moves-2026-11-07.md`:
the baseline; how `class`, `actor` and `step_up` are read from the
descriptor statics with `syn`; the 19 ported cases and the per-property
failure demonstrations; what happened to the Python test file; the
`audit-desync` fixture's proof and which gate fails it; the schema/
coverage interaction fixture from §3; per-hunk hashes; and the gate
summary **run on the real tree after copy-back**.

**On sizing**: ~690 lines of Python stand behind this (317 + 373), plus
fixture work. It looks like one round to me and I have been wrong about
that twice this cycle while your measurement rounds have been right.
**If it is two, say so before building**, and keep any split on the side
that leaves no gate with two definitions of true.
