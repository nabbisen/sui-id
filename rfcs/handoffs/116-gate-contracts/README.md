# RFC 116 implementation handoff — gate contracts

**Governing RFC.** [RFC 116](../../proposed/116-gate-contracts.md), **Proposed**.
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
| 1 | D1, D2 — the command inventory: one source, gated | RFC Accepted; **open question 1 ruled** |
| 2 | D3 — the audit matrix's `class` column, and the others where derivable | RFC Accepted |
| 3 | D4 — generate `ci.yml`; collapse A3.4 | stages 1 and 2 landed |
| 4 | D5 — G12 through the dispatcher, or the exception restated as a dated decision | stage 3 |
| 5 | D6 — placement of the survivors, one move, all references in the same commit | stages 1–4 |

## Stage 1 — the command inventory: one source, gated

**Blocked on open question 1.** Do not start until `@nabbisen` has said which
copy survives. Building this against the wrong copy wastes the whole stage.

**If the TOML survives (the architect's recommendation):**
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

## Stage 2 — the audit matrix: check the load-bearing column

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
