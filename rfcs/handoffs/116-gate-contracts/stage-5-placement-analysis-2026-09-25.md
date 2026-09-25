# RFC 116 stage 5 — where `ci/` should live: analysis and options

**Date:** 2026-09-25
**Handoff:** [`README.md`](README.md) §Stage 5 (re-walked 2026-09-25): an **analysis and options with their costs; not a move, and no name proposed as settled.**
**Baseline:** `76b2904`. **No file in the repository was changed** (`git status` clean); this package is the whole output.
**Implementer:** mid-capability model.

## 1. Re-measurement of the four premises

| Premise | Measured at `76b2904` | Agrees? |
|---|---|---|
| `ci/` holds nine files | 9 (`ls ci`) | yes |
| five of them are named in a `[gates]` command | G11 `rfc-policy.toml`, G12 `ui-invariants.toml`, G15 `doc-authority.toml`, G16 `owner-attributions.toml`, G17 `write-commands.toml` (parsed from `[gates]`, each with its `[gate_owners]` value) | yes |
| owning RFCs: 093 Done, 093 Done, 098 Done, 117 Accepted, 116 Accepted | `Implemented`, `Implemented`, `Implemented`, `Accepted`, `Accepted` | yes |
| 47 files → 74 files reference `ci/` | **77** tracked files contain `ci/` (**70** outside `ci/` itself); **73** name a specific `ci/<file>`. My 73 vs your 74 is a pattern difference, not a disagreement | close; the count is method-dependent, so I give the split instead (§4) |
| "the other four … could move without touching an RFC table" | **True of the tables. False of Done RFCs generally**: `audit-coverage-matrix.md` is named in Done RFCs **098, 102, 103**; `gate-inputs.toml` in Done RFC **093**; `owner-attribution-baseline.txt` and `workflow-template.toml` in none | **partly** (§3) |

## 2. What each of the nine is

| File | Kind | Read by | Named in a `[gates]` command |
|---|---|---|---|
| `rfc-policy.toml` | **policy fed to a checker** | G11 | yes (093, Done) |
| `ui-invariants.toml` | **policy fed to a checker** | G12 | yes (093, Done) |
| `doc-authority.toml` | **policy fed to a checker** | G15 | yes (098, Done) |
| `owner-attributions.toml` | **policy fed to a checker** | G16 | yes (117) |
| `owner-attribution-baseline.txt` | **data owned by a policy** (G16's closed baseline; edited only as a reviewed diff) | G16, via `owner-attributions.toml`'s `baseline =` | no |
| `audit-coverage-matrix.md` | **registry that docs and RFCs cite** (also a human-readable normative table) | G13 + its column check; G15's event-reference check | no (hard-coded `MATRIX=` in `check-audit-matrix.sh`) |
| `write-commands.toml` | **registry that RFCs cite** (RFC 094's command inventory) | G17 | yes (116) |
| `gate-inputs.toml` | **registry *and* generator input**: the lane table, tool pins, action pins, profiles | `ci-gate.sh`, A3.4, the generator | no (`ci-gate.sh`'s default `--manifest`; `--policy` in the gate-inputs job) |
| `workflow-template.toml` | **input to a generator** | `generate-ci-workflow.py` | no |

**What `ci/` is now mostly made of: policy fed to checkers** (four files, five with the baseline). Then two registries cited as sources of truth, then two generator inputs. **D2 is satisfied for all nine**: each is read by a gate that fails when it stops being true, so its clause "anything that cannot be given a gate leaves `ci/`" has no candidate. One more file is **expected**: RFC 094 plans `ci/write-authority.toml` (policy for `audit-structure`); it is named in eight places and does not exist. Any placement should assume the directory grows by at least that.

## 3. The constraint, and a correction to how it was framed

**The five `[gates]`-named files.** Condition 7 check 4 byte-matches every `[gates]` command to its owning RFC's table row, so moving any of the five edits: the G11 and G12 rows of Done RFC **093** (lines 116, 117), the G15 row of Done RFC **098** (line 301), and, in Accepted RFCs, the G16 and G17 rows. Confirmed.

**What the framing missed: `ci/` is itself a decision of Done RFC 098.** RFC 098's taxonomy has six document layers, and **D3, "Machine-consumed contracts … The gate that reads it … `ci/`"** names the directory (098 line 73; it also places the matrix at `ci/audit-coverage-matrix.md`, lines 97 and 234, and says `doc-authority.toml` "is D3", line 316). `docs/development-specification.md:311` repeats it ("the machine-consumed gate inputs under `ci/`"). So **renaming or splitting `ci/` amends a decision of a Done RFC, not just a table row**, and that holds for **every option except doing nothing**, including the "split" option, because D3's location column names one directory. The two lane rows are the mechanical part; the D3 row is the part that needs the owner.

**Precedent for editing a Done RFC exists** (093 was edited after it closed, e.g. `363e75b`; RFC 116 D4b already says a lane-command change edits a Done RFC), and `rfcs/README.md` says only that files do not *move out of* `done/`. So it is permitted; it is a lifecycle act, as the handoff says.

**Baseline exposure** (G16 keys on `(path, sentence hash)`): three baseline lines have a `ci/` path (`audit-coverage-matrix.md`, `owner-attributions.toml`, `workflow-template.toml`) and would be rewritten by a move of those files; Done RFCs 093 and 098 carry 9 baseline lines each, which change only if an edited sentence itself contains an attribution (I have not checked which sentences a lane-row or D3 edit would touch).

## 4. Where the 77 references are, and which are enforced

| Area | Files naming `ci/` | Breaks a gate if not updated? |
|---|---|---|
| `scripts/`, `.github/`, `ci/` itself, one `crates/` doc comment | **27** | **Yes, for the constants and commands**: `ci-gate.sh`'s default manifest, `MATRIX=` in `check-audit-matrix.sh`, the generator's two paths, `--policy`/`--inventory` arguments in `[gates]`, `doc-authority.toml`'s `matrix =`, `owner-attributions.toml`'s `baseline =`, the gate-inputs job, ~30 fixture `sed`/`cp` lines; the rest are comments. `ci.yml` is regenerated, not edited |
| Done RFCs (093, 098, 102, 103) | 4 | the 3 lane rows: yes (condition 7); the rest is prose: no |
| Accepted / Proposed RFCs | 7 | rows for 116, 117: yes; rest prose: no |
| `rfcs/handoffs/` | 34 | **No** (historical records; markdown links to `ci/` from RFCs and handoffs number 2) |
| `docs/`, `ROADMAP.md`, `CHANGELOG.md`, `README.md` | 5 | **No**: G10b and G14 check markdown links, and references to `ci/` are backticked paths, not links |

So a rename is **~27 files that must change and ~50 that would go silently stale**; nothing gates a stale backticked path. That silent staleness is the same class of defect RFC 116 exists to remove, and it is a cost of every option that changes a path. (Today 4 distinct `ci/…` names in the tree do not exist: `write-authority.toml` is planned, `matrix.md` and `decisions.txt` are test fixtures, `audit-commands.toml` is a 2026-08 name; none is a live breakage.)

## 5. The options, with costs

**A. Leave `ci/` as it is and say what it means.** Cost: one paragraph, in RFC 098's D3 row or `docs/development-specification.md`, saying `ci/` holds the machine-consumed contracts a gate reads (not only CI). **No path changes, no Done RFC touched** if the statement goes in the spec; touching 098 is optional. What it does not fix: the name still says "CI" over files that are policies, registries and generator inputs; the original complaint ("messy and partially duplicate of `.github/workflows/`") is, however, **already answered** because `ci.yml` is generated and nothing in `ci/` duplicates it any more.

**B. Rename the whole directory (name is `@nabbisen`'s).** Cost: the ~27 enforced files; edit Done RFCs **093** (2 rows), **098** (1 row **and the D3 row**, plus its matrix and `doc-authority` mentions) and, for consistency, 102/103's mentions; the 116 and 117 rows; `docs/development-specification.md`; rewrite the three `ci/` baseline lines; in **one** commit, with G14 and G10b green in the same commit (the original design). Benefit: the name matches the contents; the unenforced ~50 references can be left stale (they are records) or swept. Risk: every stale backticked path in a handoff or RFC is now wrong with no gate to say so.

**C. Split: move the four "free" files, leave five.** The four are `audit-coverage-matrix.md`, `gate-inputs.toml`, `owner-attribution-baseline.txt`, `workflow-template.toml`. Measured: it avoids the two `[gates]` rows in Done RFCs, but it **does not avoid touching Done RFCs**: the matrix is 098's canonical D3 example and `gate-inputs.toml` is described in 093, and **D3's single-directory statement has to be amended anyway**. It moves `owner-attribution-baseline.txt` away from the `owner-attributions.toml` whose `baseline =` names it, and leaves the manifest that lists the five staying files' commands in a different directory from them. It trades a coherent directory for a cheaper commit, and the commit is not much cheaper. **This is the option I would not choose.**

**D. Sub-directories inside `ci/`** (`policy/`, `registry/`, …). Every path still changes, so it costs what B costs (the same 27 enforced files, the same Done-RFC rows) and leaves the name alone. Dominated by B or A.

**E. A directory-level alias** (a symlink or a copy). A copy is exactly the duplication this RFC removes; a symlink in a git tree is a fragile fact nothing checks. Not recommended, listed so it is not rediscovered.

| | Enforced edits | Done RFCs touched | 098 D3 amended | Silent staleness | Name fixed |
|---|---|---|---|---|---|
| **A** | 0 | 0 (optional: 098) | no | none new | no |
| **B** | ~27 files | 093, 098 (+102, 103 prose) | **yes** | ~50 files | yes |
| **C** | ~20 files | 093 (prose), 098, 102, 103 | **yes** | ~40 files | partly |
| **D** | ~27 files | 093, 098 | **yes** | ~50 files | no |

## 6. A view, marked as one

- **The evidence says the directory's structural problem is solved and its remaining defect is a name.** If the name is not worth a Done-RFC decision amendment and ~50 silently stale references, **A** is enough, and it can be done by a paragraph. If it is worth it, **B** in one commit is the coherent move; **C** and **D** cost most of B's price for less.
- **Whichever is chosen, two decisions are `@nabbisen`'s and cannot be inferred from the code**: the name, and whether RFC 098's D3 is amended.
- **Independent of the choice**, a cheap gate would remove the "silent staleness" cost of ever moving anything: check that every `ci/<file>` path named in a tracked file exists (allow-listing the planned `write-authority.toml` and the test-fixture names). I did not build it and it is not in the stage; it is what would make B safe rather than merely possible.

## 7. Not done, on purpose

Nothing was moved, renamed, edited or proposed as a name. I did not check which 093/098 sentences carry baselined attributions (§3); that matters only if B or C is chosen.

