# R10 — multi-source Gate Matrix lane registry

**Governing RFC.** [RFC 094](../../accepted/094-transactional-audit-registry.md)
§Structural coverage gate, from *"Registering it requires a mechanism that does
not exist yet"* through §Migration (lines 738–866 at `c97027c`). The design is
complete there; this handoff adds nothing to it and must not.
**Review of record.** [Lane-ownership design review, 2026-08-27](094-lane-ownership-design-review-2026-08-27.md)
— found two checks the first draft silently dropped; both restored as checks 5
and 6. Read it before the RFC section: it explains why six, not four.
**Authorized.** `@nabbisen`, 2026-09-12. Tracked as **R10** in `ROADMAP.md`.
**Implementer.** Mid-capability model.
**Baseline.** `aa93c9a` or later on `main`.
**Landed.** `153db49`, 2026-09-12 — reviewed at
`.git-exclude/reviewed/r10-lane-registry-2026-09-12.md`. R10-b below is open.

## Why now

Three things are queued behind this and none can start without it:

1. RFC 094's own structural-gate lane (`audit-structure`, M2b) — the RFC
   requires the registry "before this gate is relied on as M2a exit evidence".
2. An interim lane for `scripts/check-audit-matrix.sh`, which has **never run
   in CI** (finding: `.git-exclude/reviewed/audit-matrix-gate-never-wired-2026-09-12.md`).
3. RFC 098's three documentation checks, which need a lane of their own.

RFC 093's lane set is closed by its own text ("G01–G12 and that set is
closed"), and RFC 093 is in `done/`, so none of the three can be added by
amending it. RFC 094's registry is the sanctioned path, and its migration is
designed to be behaviour-preserving so it can land and be proven green before
any new lane exists. **This dispatch is that migration and nothing more.**

## Scope — exactly three files

| File | Change |
|---|---|
| `ci/gate-inputs.toml` | Add `[gate_lane_sources]` with one entry, `"093" = "Gate Matrix v1"`, and `[gate_owners]` mapping every current `[gates]` key (G01–G12, incl. G07b/G09a/G09b/G10a/G10b) to `"093"`. Nothing else in the manifest changes. |
| `scripts/check-gate-inputs.sh` | Condition 7 becomes the six checks below. `--rfc` is **removed** — sources are manifest data now, and a flag that overrides them would be a second registry. `ci.yml` never passed it (line 464); only the fixture script did. |
| `scripts/tests/check-gate-inputs-fixtures.sh` | Drop `--rfc` from `run_checker`; extend `make_valid_fixture` to copy the four lifecycle folders under `rfcs/` (number-to-file resolution needs them); add the fixtures in §Fixtures. |

**Not in scope:** any real new lane (no G13 in the real manifest); any edit to
RFC 093 (its heading is recorded as-is — that is the point); any edit to RFC
094 (its single-row lane table arrives with the *next* dispatch, alongside the
audit-matrix lane); `ci.yml` (A3.4's command is unchanged); the
`[gate_matrix_exceptions]` reason rule and conditions 1–6 and 8, which stay
exactly as they are.

## The six checks — from RFC 094, restated only so the fixture list can cite them

Keep the `condition 7:` message prefix so A3.4's numbering in `ci.yml`'s
comments stays true; distinguish the checks as `condition 7 (check N):`.

1. Every `[gates]` key has exactly one `[gate_owners]` entry.
2. Every `[gate_owners]` value appears in `[gate_lane_sources]` and resolves
   to **exactly one** RFC file across `proposed/`, `accepted/`, `done/`,
   `archive/` — zero or several is a failure, never a first-match guess.
3. Every lane in **every** source RFC's table is present in `[gates]` or
   `[gate_matrix_exceptions]`.
4. Every `[gates]` command byte-matches the row for that lane in **its owning
   RFC's** table, under the one normalisation permitted today (`` ` and ` `` →
   `` ` && ` `` for G05/G06). No other.
5. No lane appears in both `[gates]` and `[gate_matrix_exceptions]`.
6. Every `[gate_matrix_exceptions]` key names a lane in some source RFC's table.

Exceptions need no `[gate_owners]` entry (RFC 094 states why; do not add one).

### Heading matching — three rules, none optional

- **Plain equality, never a pattern.** Strip leading `#`s and surrounding
  whitespace from the candidate line; compare to the recorded string with `==`.
  Today's `awk '/^## Gate Matrix v1/'` is a regex on a literal; it must go.
  A heading like `Gate Matrix (v2)` fed to a regex reads its parentheses as a
  group.
- **Level-agnostic.** `#` through `######` may carry a lane table.
- **Exactly once in the owning RFC.** Zero or several fails.

The section body runs from the matched heading to the next heading line of any
level — `/^#/` — which is today's behaviour. That rule also ends a section at a
`#` line inside a fenced code block; RFC 094 records this as "today's
behaviour", so it is **not** yours to fix here. Note it in the request if a
fixture trips on it; do not widen the parser.

Ownership conflicts are TOML parse errors (duplicate table keys), not
conditions — do not write a detector for them.

## Fixtures — the evidence that the mechanism can say no

The existing harness builds a valid fixture from the real files and applies
exactly one violation per case (`expect_failure <name> '<exact message>'`).
Match that shape. The harness must first prove the migration is
**behaviour-preserving**, then prove each check fires, then prove the
multi-source path works at all.

**Positive, migration.** The real manifest, unmodified, passes — and the
script's output for the real repository is compared against the pre-change
script's output on the same tree: **byte-identical**. That is the RFC's
"behaviour is then identical" claim, measured.

**Positive, multi-source.** A fixture-only second owner: copy a synthetic
`rfcs/accepted/094-fixture.md` carrying a heading `Gate Matrix lanes owned by
RFC 094` (level `###`, to exercise level-agnosticism) with one row `G13`, add
`"094"` to `[gate_lane_sources]`, `G13 = "094"` to `[gate_owners]`, and the
matching `G13` command to `[gates]`. Must pass. Without this case, the
single-source migration proves nothing about the mechanism it exists to add.

**Negative, one per check** — each mutating the *multi-source* positive fixture
above, so the failure is attributable to the check and not to the migration:

| Case | Mutation | Must fail with |
|---|---|---|
| `registry-check1-unowned-lane` | remove `G13 = "094"` from `[gate_owners]` | `condition 7 (check 1):` … names `G13` |
| `registry-check2-owner-not-a-source` | `[gate_owners]` `G13 = "094"` but no `"094"` in `[gate_lane_sources]` | `condition 7 (check 2):` … |
| `registry-check2-resolves-to-zero` | `"094"` in sources but no `094-*.md` in any lifecycle folder | `condition 7 (check 2):` … `0` files |
| `registry-check2-resolves-to-two` | a second `094-*.md` copied into `rfcs/archive/` | `condition 7 (check 2):` … `2` files |
| `registry-check3-source-lane-unaccounted` | remove `G13` from `[gates]` (and not in exceptions) | `condition 7 (check 3):` … names `G13` |
| `registry-check4-command-drift-in-owner-table` | change one character of `G13`'s command in the fixture RFC | `condition 7 (check 4):` … `094` |
| `registry-check4-normalisation-not-widened` | write G05's row with ` and ` replaced by ` && ` in RFC 093's copy — i.e. the *other* direction | must fail: only ` and `→`&&` is permitted, not the reverse |
| `registry-check5-both-gates-and-exception` | add `G13` to `[gate_matrix_exceptions]` with a reason, leaving it in `[gates]` | `condition 7 (check 5):` … |
| `registry-check6-ungrounded-exception` | exception key `G99` with a reason, declared by no source | `condition 7 (check 6):` … `G99` |
| `registry-heading-absent` | rename the heading in the fixture RFC 094 | heading … occurs `0` times |
| `registry-heading-twice` | duplicate the heading in the fixture RFC 094 | heading … occurs `2` times |
| `registry-heading-not-a-pattern` | recorded heading `Gate Matrix (v2)`; document heading `Gate Matrix (v2)` — **must pass**; then document heading `Gate Matrix v2` — must fail | proves equality, not regex |

Each expected message is a string you define; pin it exactly in
`expect_failure`, as the existing cases do. A fixture that fails for a
different reason than the one named is a failed fixture — check the message,
not just the exit code.

## Evidence required in the review request

1. `bash scripts/check-gate-inputs.sh --all --policy ci/gate-inputs.toml` on
   the real tree: **exit 0**, and its stdout/stderr diffed against the
   pre-change script's on the same tree: identical.
2. `bash scripts/tests/check-gate-inputs-fixtures.sh`: every existing case
   still passes, every case above added and passing. Report the count before
   and after.
3. `bash scripts/tests/check-gate-matrix-fixtures.sh` and
   `bash scripts/tests/check-ci-gate-fixtures.sh`: unchanged and green — the
   dispatcher and the matrix self-tests must not notice this change at all.
4. G11 and G10b (both scopes), since this touches `ci/` and no RFC — expected
   unaffected; say so with the output.
5. `git diff --stat`: three files. Anything else is scope creep — stop and
   say why.

## Stop and return to the architect if

- the byte-identical comparison in evidence 1 fails for any reason — do not
  "fix" the diff; report it;
- any existing fixture starts failing;
- the fenced-code-block heading hazard bites a real RFC — that is a design
  question, not a parser tweak;
- you find yourself wanting a fourth file.

## After this lands

The next dispatch registers the interim audit-matrix lane: a single-row table
in RFC 094 under the heading its design names, `[gate_owners]` and `[gates]`
entries, the A3.2 negative self-test the lane inherits (which is the desync
fixture RFC 085 promised in v0.68.0 and never delivered), and the `ci.yml`
job. Not this dispatch.

## R10-b — make the parse-error premise true

**Dispatched 2026-09-12**, after R10 landed as `153db49`. Small; one commit.

RFC 094 says, and this handoff repeated as an instruction, that ownership
conflicts are TOML parse errors and need no detector. **That holds only if
something parses the file as TOML, and nothing in this pipeline does** — every
reader is awk. Measured at review: a duplicate `[gate_owners]` key is caught by
check 1; a duplicate `[gate_lane_sources]` key whose second heading resolves
cleanly (`"093" = "Summary"`) passes with exit 0, while `tomllib` rejects the
same file. The premise was mine to get right; you followed it correctly.

**The fix is to realise the premise, not to write the detector RFC 094 forbids:**

1. At the top of `scripts/check-gate-inputs.sh`, before any condition, a
   TOML-validity precheck of the manifest via Python 3.14's `tomllib`. On
   failure: `gate-inputs: manifest is not valid TOML: <tomllib's message>`,
   exit 1, and no further conditions run — a malformed manifest has nothing
   meaningful to check. Python 3.14 is already pinned in `[tools]` and invoked
   by G10b and G11; this adds no dependency. Prefer a tiny inline
   `python3.14 -c` over a new script file.
2. Two fixtures: `registry-duplicate-source-key` — `[gate_lane_sources]` with
   `"093"` twice, the second heading one that occurs exactly once in RFC 093 —
   must fail on the precheck, pinned by `not valid TOML` and `line`; and
   `registry-duplicate-owner-key` — `G02 = "093"` twice — must now fail on the
   precheck *before* check 1 reaches it, pinned the same way. Keep check 1's
   "exactly one" logic unchanged; it is still the right check for an
   `[gates]` lane with no owner.
3. Evidence: the real tree still byte-identical to R10's output; both suites
   green; fixture count 36 → 38.

Two files: `scripts/check-gate-inputs.sh`, `scripts/tests/check-gate-inputs-fixtures.sh`.
A third is a stop condition. RFC 094's dated note that its guarantee is
realised by this precheck is the architect's, added when this lands.
