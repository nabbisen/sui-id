# Cycle J stage 5 — make a half-finished rotation legible

> ## Whose turn it is: **the implementation role's.**
> **This is the second half of the philosophy, not a tidy-up.** Stage 4
> made the record correct. One operator action now writes **two** audit
> records, and two records are better than one *only if an operator can
> tell what seeing one without the other means.* Right now nothing says.

**Dispatched.** 2026-11-04, by the architect.
**Stage.** 5 of nine. §D of
`rfcs/handoffs/094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md`.
**Depends on.** Stage 4, landed `2b1bac5`.
**Scope.** The second record, the pair's documentation, and two
corrections to the coverage matrix. **No new sealed command. No gate
work** (stages 6–7). **No conversions.**
**Baseline.** The tip of `main` when you start — `git log -1`, record it.
**Status check.** RFC 094 is **Accepted**. Building is permitted.

---

## 1 — what the two records are, and why the second cannot be sealed

After stage 4, `sui-id admin rotate-key` writes:

1. **`admin.master_key.resealed`** (K07) — inside the transaction that
   re-seals the six tables. Carries the per-table counts.
2. **`admin.master_key.rotated`** — `cli.rs:538`, after **both**
   filesystem renames: old key → `.bak.<stamp>`, staged `.new` → the
   configured path.

**The second cannot join a transaction and that is not a defect.** A
filesystem rename is not a SQL statement; there is no transaction for
it to be atomic with. It reports a fact that becomes true outside the
database.

**So drop the `(A required)` annotation**, and record why. The matrix
row (`contracts/audit-coverage-matrix.md:210`) still reads
`B *(A required)*`. That annotation was **correct when it was
written** — the event was then the only record of the re-seal, and the
re-seal *was* atomicizable. Stage 4 moved that fact to its own event.
What is left reports the renames, and requiring atomicity of it would
be requiring something incoherent. The row becomes `B`, with a
sentence saying it is deliberately B and what it reports.

**Settle it in the record, not in passing.** This is the question stage
4's review left explicitly open, and it is the one a future reader will
otherwise re-litigate.

## 2 — its failure must be reported, not discarded

`cli.rs:538` is `let _ = …append(…).await;`. The error is dropped.

**An operator who loses this record is told nothing.** That is the
precise case where it matters most: the renames succeeded, the
rotation is complete, and the log will not say so — so a later reader
sees `resealed` with no `rotated` and concludes the key file may be
missing, which is the **opposite** of the truth.

**Report it on stderr**, in the same voice as the rest of the
command's output, and say what it does and does not mean: the rotation
**did** complete, the audit record of its completion did not get
written, and the operator should note it by hand. Do **not** fail the
command — the work is done and committed, and exiting non-zero would
invite a re-run, which the staged-file refusal would then block.

This is the by-product stage 4 deliberately left here. It is smaller
now than it was: the sealed half's failure already propagates.

## 3 — the note it writes should say what it alone reports

Today: `"re-sealed {} rows; old key archived at {}"`.

The row count now belongs to `resealed`, which carries it per table.
This event's own subject is the **key files**. Rewrite the note to
report what only this event knows — the archive path and that the new
key is in place at the configured path.

Then **`contracts/audit-coverage-matrix.md`'s note-fields column for
this row is wrong and must match**: it says `keys_resealed=N`, which
the live append has never written in that form. Stage 4 left the row
byte-for-byte alone on purpose; this stage owns it.

## 4 — the documentation, which is the point of the stage

### `docs/src/guides/operators.md`

`:172-173` currently says: *"An audit row
`admin.master_key.rotated` is appended with a count of re-sealed
rows."* That is now wrong twice — there are two rows, and the count is
on the other one.

**Write the diagnostic table.** An operator reading their audit log
after a failed rotation needs to map what they see to what to do:

| what the log shows | what happened | what to do |
|---|---|---|
| both events | the rotation completed | nothing |
| `resealed`, no `rotated` | the database is re-sealed under the new key; the key files may not have been swapped | check for `<key_file>.new`; the procedure in "Use `--new-key`" applies |
| neither | nothing committed; the database is still under the old key | re-run |
| `rotated`, no `resealed` | **only possible on a database rotated before `0.88.0`** | see the note below |

That last row is not hypothetical and it is the one I would most
expect to be left out. `docs/src/reference/audit-events.md` promises
event names do not change between releases; an audit log spanning the
upgrade will hold old single-`rotated` rows from earlier releases, and
a reader applying this table to them would misdiagnose a **complete**
rotation as a half-finished one. **Say which release the pair starts
at, and that rows older than it mean something different.**

### `docs/src/reference/audit-events.md`

`admin.master_key.rotated`'s description still reads *"All
column-encrypted values were re-sealed under the new key"* — which is
now the **other** event's meaning. Stage 4 left it untouched under
instruction. Correct it to describe the key-file replacement, and
cross-reference the pair.

## 5 — `step_now()`: I already did the sweep, and §D overstated it

§D told you to sweep the remaining `step_now()` call sites in
`crates/sui-id/tests/e2e/`. **Do not. I measured it and the sweep is
not needed** — my own §D entry asked for more work than the facts
support, which is the error this cycle keeps producing and I would
rather correct than let you spend a day on.

What I measured:

- **`totp.rs:60`** iterates `for delta in [-1i64, 0, 1]`, so the
  validator accepts a ±1 step window. A code built inline with
  `code_at(secret, step_now())` therefore still verifies when the clock
  crosses one boundary before the request lands. Those eight sites are
  **not** latent flakes.
- **`totp.rs:62`** refuses `step <= last_used_step`, so two successful
  sign-ins inside one 30-second step would be a replay. The one site
  that challenges repeatedly (`r102_stage3.rs:650`) expects **refusal**
  on every iteration — it runs after lockout. Also safe.
- The only broken site was the assertion comparing a **re-evaluated**
  `step_now()`, fixed in `76a3005`.

**What to do instead, and it is two lines.** Put the invariant at
`step_now()`'s definition in both files — `r102_stage3.rs:167` and
`r102_stage6.rs:88` — so the next person writing one of these tests
reads it: generating a code inline is safe because the validator
accepts ±1; **comparing a step must capture it once**, because the
clock moves and the comparison asserts that it did not.

Cite `76a3005` and CI run `41672489921` so the comment carries its own
evidence.

## 6 — what not to do

- No new sealed command, no new descriptor, no inventory row.
- No gate work. The AST gate and the structural/string gates are
  stages 6 and 7.
- **Do not make `admin.master_key.rotated` Class A**, and do not quietly
  leave `(A required)` in place either — §1 asks you to settle it.
- Do not fail the command when the second append fails (§2).
- Do not restate a gate command in prose; A3.4 forbids it.
- No `audit.yml` run, no Level B claim. Both readings stay with me.

## 7 — `CHANGELOG`

One `Changed` entry. Lead with the operator: a rotation that stops
half-way is now diagnosable from the audit log, and the guide says how.
Name the dropped `(A required)` annotation only if you think an
operator cares — I do not think they do; the contract readers do, and
the matrix records it for them.

Do **not** describe this as a fix to a defect introduced in stage 4.
Stage 4 created the second record deliberately; this stage explains it.

## 8 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage5-a-half-finished-rotation-is-legible-2026-11-04.md`,
with: the baseline commit; the four-row diagnostic table as you wrote
it; what you decided about `(A required)` and why; per-hunk hashes via
`scripts/hunk-hashes.py`; and the gate summary.

**The gate summary rule from last round still applies**, and you met it
first time: the run you cite must be the run **on the real tree, after
copy-back**, or the `Gates.` line must say which tree it ran on.

**One request about prose.** Last round's §1 said an audit append "is
gone" when it was not — the code was right and the sentence was wrong,
and it contradicted your own §5 and §8. When you say something was
removed, name the file and line it left, or write "unchanged at
`<file>:<line>`". A one-word summary of a deletion is the sentence most
likely to be wrong and least likely to be checked.
