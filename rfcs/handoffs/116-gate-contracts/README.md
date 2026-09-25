# RFC 116 implementation handoff — gate contracts

**Governing RFC.** [RFC 116](../../accepted/116-gate-contracts.md), **Proposed**.
Nothing here is authorized until it is Accepted.
**Implementer.** Mid-capability model.
**Baseline.** `0c15eca` or later.
**Measurements.** Taken 2026-09-22 against `0c15eca` by the architect, by
reading each file in full and grepping the tree for its readers. Every number
below is reproducible by the command given beside it.

## The measurements this RFC rests on

Re-run these before starting; if one disagrees, say so before building.

| Claim | How it was measured |
|---|---|
| `write-commands.toml` has no reader | `grep -rn "write-commands" scripts/` → no output |
| 99 entries, 89 implemented | `grep -c '^\[\[command\]\]'`, `grep -c 'status = "implemented"'` |
| header claims 82 implemented | the file's own header comment |
| `event`/`descriptor` empty in all 99 | `grep -c 'event = ""'`, `grep -c 'descriptor = ""'` |
| the markdown twin has 92 rows | `grep -cE '^\| [A-Z][0-9]{2} \|' rfcs/handoffs/094-transactional-audit/command-inventory.md` |
| drift: `L01`–`L07`, `U37` in TOML only; `U06` in markdown only | `comm` of the two id lists |
| G13 checks names only | `scripts/check-audit-matrix.sh` header, lines 4–7 |
| runner label appears 20× in `ci.yml` | `grep -c "runs-on: ubuntu-24.04" .github/workflows/ci.yml` |
| packages 10×, stable 8×, MSRV 6×, Python 6×, mdbook 2×, components 4× | the equivalent greps |
| G12 is the only `[gate_matrix_exceptions]` entry | `ci/gate-inputs.toml` |

## Order

Stages 1 and 2 are correctness and are independent of each other. Stage 3
changes how every lane is defined and waits for both. Stage 5 is the
rearrangement `@nabbisen` originally asked about, and is last because it
depends on what survives.

| Stage | Content | Prerequisite |
|---|---|---|
| 1 | D1, D2 — the command inventory: one source, gated | **unblocked 2026-09-24** |
| 2 | D3 — the audit matrix's `class` column, and the others where derivable | **unblocked 2026-09-24**; the matrix is already corrected |
| 3 | D4 — generate `ci.yml`; collapse A3.4 | stages 1 and 2 landed |
| 4 | D5 — G12 through the dispatcher, or the exception restated as a dated decision | stage 3 |
| 5 | D6 — placement of the survivors, one move, all references in the same commit | stages 1–4 |

## Stage 1 — the command inventory: one source, gated

**Unblocked 2026-09-24: the TOML survives.** `@nabbisen` ruled open question 1
on the design review's measurement — `ci/write-commands.toml` is the maintained
copy, carries `status`, `files` and `test_id` on 99 of 99 rows, and parses in
one call, while the markdown twin has **no `files` and no `status` column at
all**, so a path-existence gate is not expressible against it.

**Read RFC 116's D2a before building the gate.** The design review showed the
gate as first worded here **cannot be green**: 76 of the 99 rows name no command
in `commands.rs`, because the inventory records *planned* conversions too, and
those rows are true. The schema needs a state distinguishing sealed from
planned, and the direction rules follow from it. The two dead `files` paths
(`O04`, `X02`) are reported in the package, not silently fixed.

**What the ruling means concretely:**
- `rfcs/handoffs/094-transactional-audit/command-inventory.md` loses its table
  and keeps its prose, pointing at the TOML. Do not delete the file: it carries
  the reasoning, which the TOML cannot.
- A new gate checks `ci/write-commands.toml` against the code: every `id` has a
  command in `crates/sui-id-store/src/commands.rs`, every command has an `id`,
  and each `files` path exists. Bidirectional, like G13.
- The header's counts are **derived and asserted by the gate**, not written by
  hand, or they are deleted. A hand-written count is how this file went stale.
- `event` and `descriptor`: either fill them from the registry — RFC 094
  Stage 1 has landed, so the typed enums they were waiting for now exist — or
  remove both columns. Two columns empty in 99 of 99 rows for a month are not a
  schema, they are a plan.

**If the markdown survives:** the reverse, and the gate parses the markdown
table. Say so in the review package, because a markdown table is a harder
parse and the gate must still be bidirectional.

**Evidence.** The gate fails when a command is added without a row, when a row
names a command that does not exist, and when a `files` path is wrong — one
mutation each, each restored. Plus: the gate is green on the tree it lands in
without editing the inventory to make it so, or, if it is not, every row it
disproves is listed in the review package rather than fixed silently.

## Stage 1b — landed `a214472`, 2026-09-25

Both paths corrected, G17 live, `[gate_matrix_exceptions]` back to G12 alone.
The lane was verified through `ci-gate.sh` as well as directly, in a throwaway
worktree, because the dispatcher demands a clean tree and a matching HEAD.

**The `test_id` measurement, which is the part to carry forward.** Of the
inventory's 99 rows, **one** `test_id` resolves to a function and **98** do not
— re-measured independently by the architect with a different extraction (106
distinct names, one resolving: `compile_fail_session_insert_is_private`). All 23
sealed rows are among the unresolved, and yet those commands *are* tested, under
other names. So the column records tests RFC 094 Stage 0 planned and nobody
wrote under those names.

It is either a **plan**, in which case it should say so, or a **pointer**, in
which case it points nowhere 98 times out of 99. G17 does not check it; nothing
has changed it. **RFC 094's `audit-structure` is the natural owner** — its own
condition "a Class-A command lacks its failure test" is exactly this question —
and the pointer RFC 116 D6 added to RFC 094 is where whoever builds it will
look. Not scheduled here.

## Stage 1b — as dispatched 2026-09-24

**J1 is ruled: correct them, then promote the lane.** Holding G17 in
`[gate_matrix_exceptions]` was the right call for stage 1 — a red lane on main
is not a gate — but the exception is not where this ends. The two rows are not a
judgment: `O04` and `X02` name `repos/backup.rs` and `repos/runtime.rs`, and
`git log --all` shows **neither file has ever existed**. That is a factual error
in a manifest, not a claim anyone decided. It is unlike RFC 116 D3a's twelve
rows, which asserted a security property and went to the owner.

**Required.**

1. **Correct the two `files` paths** to the homes the stage 1 package
   identified and this review confirmed exist:
   `O04` → `crates/sui-id-store/src/backup/ops.rs`,
   `X02` → `crates/sui-id/src/runtime/dev_mode.rs`.
   Change nothing else in either row.
2. **Promote G17 to a live lane**, exactly as J1 sets out: move the line from
   `[gate_matrix_exceptions]` to `[gates]`, byte-matching RFC 116's table row;
   add `G17 = "116"` to `[gate_owners]`; add a `G17` job to `ci.yml` shaped like
   G15's. `[gate_matrix_exceptions]` returns to holding G12 alone, which is what
   its own comment says it is for.

**Evidence.** G17 exits 0 on the tree it lands in, as a live lane, with no row
edited beyond the two paths above. A3.4 passes including condition 7's
byte-match. A mutation that reverts either path is caught by G17 itself.

**While you are there, and only if it is one line:** the stage 1 package
observed that neither row's `test_id` (`o_o04_backup_snapshot`,
`x_x02_dev_seed`) exists anywhere under `crates/`, and that it did not check the
other 97. **Do not fix that here.** Report whether the other 97 resolve, as a
measurement, so the scale is known before anyone decides whose gate it is —
RFC 094's `audit-structure` most likely owns it.

## Stage 2 — the audit matrix: check the load-bearing column

**Unblocked 2026-09-24, and the ground has moved.** D3a's twelve disproved rows
were ruled and **already corrected** in `ci/audit-coverage-matrix.md`: they read
`B *(A required)*`, pointing at RFC 094 M2b. So stage 2 no longer has to find
them — it has to make the file *stay* true. **Re-run the by-hand check at your
own baseline before building**: RFC 115 and RFC 105 have both touched
`commands.rs` since the count was taken, and the twelve were a measurement of
2026-09-24, not a constant.

`ci/audit-coverage-matrix.md`'s `class` column asserts that a command is
Class-A. That assertion is cited by `docs/threat-model.md` and by RFCs 094, 102
and 103, and nothing checks it.

- **`class`**: every row claiming Class-A names a command that is on the
  Class-A seam — `declare_write_command!` with `Database::class_a`. Every
  Class-A command has a row claiming it. Bidirectional.
- **actor, target, attributes**: check each against the command's descriptor
  where the registry makes it derivable. Where it is not derivable, say so in
  the review package and leave the column unchecked rather than checking it
  approximately — a gate that is right most of the time teaches people to
  ignore it.
- Extend `scripts/check-audit-matrix.sh` rather than adding a lane: G13 already
  owns this file, and a second lane over one file is the duplication this RFC
  exists to remove.

**Evidence.** A mutation per checked column: flip a row's class, change an
actor, drop a required attribute — each caught, each restored. **Report any
row the new checks disprove; do not correct it in the same commit.** A false
Class-A row is a security-claim correction and goes to the owner.

## Stage 3 — generate the workflow

`ci/gate-inputs.toml` holds the runner, the packages, four tool versions, the
components, the action pins and the lane list. `.github/workflows/ci.yml` holds
all of them again. `scripts/check-gate-inputs.sh` reconciles the two in eight
conditions.

- A generator writes `ci.yml` from the table. The committed workflow is checked
  against a fresh generation, like `cargo fmt --check`. That check replaces
  conditions 1, 6 and 8 outright; say in the review package what happens to
  each of the other five — kept, moved into the generator, or dropped with a
  reason.
- The `[gates]` table stays the source of truth and `scripts/ci-gate.sh` keeps
  executing it, so a lane stays runnable locally exactly as CI runs it. **This
  property is the reason the table is not deleted instead; do not lose it.**
- `audit.yml` and `fuzz.yml` are out of scope unless they read the same table.
  Say which, with evidence, before touching either.

**Evidence.** The generated `ci.yml` is byte-identical to the committed one at
the start, or every difference is listed and justified. The generator's own
test asserts the **lane set round-trips**: a lane added to the table appears as
a job; a job with no table entry is a failure. A missing lane is the failure
mode that fails invisibly, which is why it gets its own test.

## Stage 3b — the two things stage 3 raised — dispatched 2026-09-25

Both were raised rather than decided, which was right in each case. Both are
ruled here.

### 1. The census must not count a generated file and its source

**Ruled: exclude generated files from G16's scan.** Since stage 3, every
census-tripping sentence in a lane comment lives twice — in
`ci/workflow-template.toml`, where it is written, and in
`.github/workflows/ci.yml`, where it is generated — so it needs two baseline
lines. The implementer cleared it the sanctioned way and flagged the edit; the
recurrence is the problem, not that edit.

**A census should read sources, not artefacts.** The reason this is safe rather
than a hole: an attribution smuggled directly into `ci.yml` would make it differ
from a fresh generation, and `generate-ci-workflow.py --check` fails on that. The
generated file is covered by a different gate, so removing it from this one
loses nothing. **Say so in the policy file**, beside the exclusion, or the next
reader will read it as a gap.

- Add the exclusion to `ci/owner-attributions.toml`, as data, not to the script.
- **Remove the now-redundant baseline line** for `.github/workflows/ci.yml`'s
  lane comment, and leave the template's. A test should show that a new
  attribution in the template still fails, and that the same sentence appearing
  in the generated file does not double it.

### 2. `TZ` — close it, do not soften the claim

**Ruled: close it.** RFC 116 D7 said the local-run property was already weaker
than claimed, because `TZ: UTC` is set in the workflow for G02, G04, G05 and
G06 and `ci-gate.sh` does not export it. Stage 3 put `tz` into
`[lane_profiles]`, so **the data now exists in the one place the dispatcher
already reads.** Softening the claim was the alternative when closing it meant
inventing a new fact; it no longer does.

- `ci-gate.sh` reads a lane's `tz` from `[lane_profiles]` and exports it.
- **The single-line-value rule still binds** (D7): the awk readers must keep
  working, and a test should pin that they do.
- The workflow's per-job `env: TZ` becomes redundant. **Remove it and let the
  generator stop emitting it** — two facts where one will do is what this RFC
  exists to remove — or state why it stays.
- Evidence: a lane whose profile names a `tz` runs locally with it set, shown
  rather than asserted; and `--check` still passes, so the generated workflow
  and the dispatcher agree.

### Not in scope

The `exit "$status"` written twice in the `gate-inputs` job's first step. Stage
3 reproduced it faithfully rather than fixing it, which was correct — it is
unrelated drift and belongs to whoever touches that job next.

## Stage 4 — the last exception

G12 is the only entry in `[gate_matrix_exceptions]`, and its own text says the
exception is "a reason not to touch it right now, not a claim that this
exception can never be revisited". Either route it through the dispatcher, or
restate the exception as a dated decision with a reason that is still true.
Measured cost of leaving it: G12 cannot be invoked by `ci-gate.sh`, so it is
the one lane that has to be run by hand — as it was during the RFC 103 stage 5
review on 2026-09-22.

## Stage 5 — placement

Only now is this answerable, because only now is the file set known.

State, in the review package: what each surviving file is, what reads it, and
where it should live given that — policy fed to a checker, or a registry that
`docs/` and RFCs cite as a source of truth. Then move them in **one** commit
with every reference updated in it, and the link gates green in the same
commit. There were 47 files referencing something in `ci/` at `0c15eca`;
re-measure before moving.

**Do not propose a directory name in the review package as though it were
settled.** Name the options and what each one costs. The naming is
`@nabbisen`'s.

## What to return, each stage

A review-request package under `.git-exclude/review-requests/`, in the form the
RFC 102 and 103 stages used: what was built, the evidence table, the mutations
with their results, the gates run on the final tree, and per-hunk SHA-256
hashes against the stated baseline.
