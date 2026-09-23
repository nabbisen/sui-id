# RFC 116 — independent design review

**Date:** 2026-09-24
**RFC:** [RFC 116 — Gate contracts: one source, one gate each](../../proposed/116-gate-contracts.md)
**Request:** [`design-review-request.md`](design-review-request.md)
**Reviewer:** Mid-capability model, implementation role. Authored neither the RFC nor its handoff.
**Baseline read:** `f33c7e5`, working tree clean before and after (`git status --short` empty). Read-only: no code, no RFC text and no `ci/` file changed. Every mutation below ran on a scratch copy of the matrix in the session scratchpad, against a scratch copy of the G13 script; nothing under the repository was touched.
**Outcome:** The RFC's diagnosis of `ci/` is **right in every measured particular** (§1: every number agrees). Its *remedies* are not all buildable as written. Three blockers:

- **B1** D3, run by hand, disproves **12 Class-A rows** in `ci/audit-coverage-matrix.md`, plus one wrong `actor` cell. That is the security-claim correction the RFC predicted; it needs an owner decision before stage 2 can be green.
- **B2** Stage 1's gate, as worded, cannot be green: 76 of the 99 inventory rows name no command in `commands.rs`, because the inventory records *planned* commands too. The "currently correct" copy also carries two `files` paths that never existed.
- **B3** D4 says the workflow is generated *from the table*. The table lacks half of what a workflow needs, and "eight conditions collapse to one" is refuted: two of the three conditions the RFC names survive in part, and the largest condition (7) has nothing to do with `ci.yml`.

Two further findings the RFC does not mention: a **third copy of every lane command** lives in RFC prose tables (H2), and RFC 094's planned `audit-structure` gate is a **second authority over the same file** (H3). My views on the open question and the sequence are in §5.

---

## 1. The measurements (request §1)

Commands run from the repository root. "Handoff" = the number in `rfcs/handoffs/116-gate-contracts/README.md`.

| # | Claim | Command | I got | Agrees |
|---|---|---|---:|:---:|
| 1 | `write-commands.toml`: no reader in `scripts/` | `grep -rn write-commands scripts` | 0 lines | yes |
| 1b | …nor in workflows, build scripts, tests, `include_str!` | `grep -rn write-commands scripts .github crates Cargo.toml $(find . -name build.rs)`; `grep -rn 'include_str!\|include_bytes!' crates \| grep -i 'ci/\|\.toml'` | one hit: `crates/sui-id-store/src/registry.rs:439`, a `///` doc comment ("`ci/write-commands.toml` (e.g. `"K01"`)"). No `include_str!` of any `ci/` file. No `build.rs` mentions it. | yes — **D2's conclusion for this file stands.** The registry.rs comment is a reference, not a reader |
| 1c | every other file that mentions the name | `grep -rIl write-commands . --exclude-dir={.git,target,.git-exclude}` | 14 files, all `.md`, plus `registry.rs` and `ci/audit-coverage-matrix.md` | — (none is a gate) |
| 2 | 99 entries | `grep -c '^\[\[command\]\]' ci/write-commands.toml`; `tomllib` | 99 | yes |
| 2b | 89 `implemented` | `grep -c 'status = "implemented"'` | 89 (also 7 `target-legacy`, 3 `target-absent`) | yes |
| 2c | header claims 82 | `grep -m1 'implemented    --' ci/write-commands.toml` | "(82)" | yes — stale by 7 |
| 2d | `event`/`descriptor` empty in all 99 | `grep -c 'event = ""'`; `grep -c 'descriptor = ""'` | 99 and 99 | yes |
| 2e | markdown twin has 92 rows | `grep -cE '^\| [A-Z][0-9]{2} \|' rfcs/handoffs/094-transactional-audit/command-inventory.md` | 92 | yes |
| 2f | drift: `L01`–`L07`, `U37` only in TOML; `U06` only in markdown | `comm` of the two sorted id lists (99 vs 92) | TOML-only: **L01 L02 L03 L04 L05 L06 L07 U37**. Markdown-only: **U06**. Nine rows. | yes. Same result at `0c15eca` and `f33c7e5` |
| 3 | G13 checks names only | read `scripts/check-audit-matrix.sh` in full; three mutations (§3 answer 3) | see answer 3 — **names only, and weaker than that**: a backticked name anywhere in the file satisfies it | yes, with a sharper finding (H4) |
| 4a | runner label 20× | `grep -c 'runs-on: ubuntu-24.04' .github/workflows/ci.yml` | 20 | yes |
| 4b | system packages 10× | `grep -c 'libssl-dev pkg-config'` | 10 | yes |
| 4c | stable 8×, MSRV 6× | `grep -c 'toolchain: stable'`; `grep -c 'toolchain: "1.95"'` | 8; 6 | yes |
| 4d | Python 6×, mdbook 2×, components 4× | `grep -c 'python-version: "3.14"'`; `grep -c 'mdbook --version'`-family; `grep -c 'components:'` | 6; 2; 4 | yes |
| 4e | "seven action pins across 57 uses" | `grep -c 'uses:'` per workflow | ci.yml **52**, audit.yml **1**, fuzz.yml **4** = 57 | yes, but **only summed over all three workflows** — see B3/H1 |
| 4f | "the lane list as 17 jobs" | list job keys with `grep -n '^  [A-Za-z0-9_-]*:$'` | 20 jobs = **18 lane jobs** (G01–G09b 11, G10a, G10b, G11, `ui-invariants-v1` = G12, G13, G14, G15) + 2 that are not lanes (`gate-inputs`, `gate-matrix-fixtures`). `[gates]` has 17 keys; the 18th is G12 | **disagrees by one**: 17 is `[gates]`, not jobs. Not load-bearing (G12 is the known exception) |
| 5 | G12 is the only `[gate_matrix_exceptions]` entry | `sed -n '/^\[gate_matrix_exceptions\]/,/^\[actions\]/p' ci/gate-inputs.toml` | one key, G12 | yes |

**Two claims the RFC makes that the handoff does not measure, and that I checked:**

| Claim (RFC 116 text) | Measured | Verdict |
|---|---|---|
| "`write-commands.toml` … is the copy that is **currently correct**" (Open question 1) | Header says "0 unlisted, 0 stale". `python` over `files`: **two paths do not exist** — `O04` → `crates/sui-id-store/src/repos/backup.rs`, `X02` → `crates/sui-id-store/src/repos/runtime.rs`. `git log --all` shows neither file ever existed (the rows entered at `d70ead3`). Real homes are `crates/sui-id-store/src/backup.rs` and `crates/sui-id/src/runtime/`. | **Not currently correct.** Better than the markdown (which is missing nine rows, and the TOML is the one being maintained: 13 commits in all, 12 of them since 2026-09-16, against the markdown's last edit being an RFC-closure commit), but two of its rows have been wrong since the day it was generated |
| "the matrix is cited by `docs/threat-model.md` and by RFCs 094, 102 and 103" (Summary/Risks) | `grep -in matrix docs/threat-model.md` → **0 lines**; the threat model does not name the file. Citers of `audit-coverage-matrix`: RFCs 085, 094 handoff, 098, 101, 102, 103, 104, 116; `docs/development-specification.md:691`; `docs/src/contributing/architecture.md:88`; `ci/doc-authority.toml:22`. | The threat model rests on RFC 102's Class-A claim, not on the file. Say "RFCs 094, 102, 103 and the two contributor docs", or the statement is unmeasured |

---

## 2. Findings

### Blocker

**B1 — D3 disproves 12 Class-A rows and one `actor` cell (an owner decision, not a fix for stage 2).**
The full by-hand result is in §4. In short: of **37** rows whose `class` cell claims Class A (in whole or part), **25** are backed by a sealed `EventDescriptor { class: AuditClass::Atomic }` in `crates/sui-id-store/src/commands.rs`. **Twelve are not**, and the code that writes each of them is a fire-and-forget `let _ = audit::append(…)` *after* the state change, i.e. Class B in fact:

`client.create`, `client.update`, `client.set_allowed_scopes`, `client.set_post_logout_redirect_uris`, `client.disable`, `client.enable`, `client.delete`, `client.rotate_secret`, `signing_key.delete`, `admin.master_key.rotated`, `auth.federation.takeover_blocked`, `auth.smtp_config.changed`.

The matrix's `A — atomic` row (`ci/audit-coverage-matrix.md:23`) reads as a guarantee ("A crash between them is impossible by construction"). I will not overstate: the file's own preamble (lines 3-4, 18) says the rows are the coverage the project *requires* and that the gate does not enforce it, so these rows are partly a plan. But rows 70-77, 84, 90, 137 and 205 say `A` with no qualifier, unlike the auth section which says "A row in this section is Class B only until its command is converted" (lines 219-220) — and a reader of RFC 102 or the threat model who looks up `client.rotate_secret` finds `A`. Also, line 261 states **"Each row below was checked against its command's descriptor on this date"** (2026-09-22); that is contradicted by the `actor` finding below, so that sentence is itself a false statement in a normative file.

One `actor` cell disagrees with its descriptor: `auth.refresh.theft_detected` (line 280) says actor `user id`; its descriptor (`commands.rs:1687-1692`) says `ActorRequirement::None`, and its command runs under `for_system_actor(None)` (`commands.rs:1792`). The matrix's own note for `auth.refresh.rotated` says the same shape.

I have **not corrected any of it**. Owner call, per the handoff (README stage 2, last paragraph): either the 12 rows become `B` (the code is B), or a conversion is scheduled and the rows say so as the auth section already does. Both are edits to a security-claim file.

**B2 — Stage 1's gate, as worded, cannot be green (and the TOML has two dead paths).**
Handoff stage 1: "every `id` has a command in `crates/sui-id-store/src/commands.rs`, every command has an `id`, and each `files` path exists. Bidirectional."

- *Every command has an id*: holds. The 23 sealed commands (`K01 L01–L07 T04 U01–U05 U07–U10 U12 U14 U15 U22 U37`, from `grep -oE '^    command [A-Za-z0-9_]+ = "[A-Z][0-9]{2}"' commands.rs`) are all in the TOML; none is in code but absent from it.
- *Every id has a command*: **fails for 76 of 99 rows** (66 of them `status = "implemented"`). The inventory is a plan: `class = "A"` on **67** rows is the *target* class, and only 23 of those 67 have a sealed command. `status = "implemented"` means "code exists" — for 62 of the 67 that code is the legacy path the row will *replace*. So as worded the gate is red on the tree it lands in on 76 rows, and "fix the inventory to make it green" is not available: those rows are true.
- *Each `files` path exists*: **fails for 2 rows** (O04, X02, above).
- The header's counts are stale (82 vs 89) — the RFC already covers this ("derived and asserted by the gate, or deleted").

Fix (small, and it is a schema decision for the owner): add a state that separates *sealed* from *planned* (e.g. `status = "sealed"`), and make the direction rules: code → TOML complete for every sealed command; TOML → code only for rows in that state; `files` checked for every non-`target-absent` row. The two dead paths get listed in the package, not silently fixed (handoff stage 1, Evidence).

**B3 — D4's premise ("generated from the single table") is false: the table does not hold what a workflow needs.**
Item 8 asks for everything in today's `ci.yml` not derivable from `ci/gate-inputs.toml`. The list (measured from the 639-line file; 76 of its lines are comments):

| Not in the table | Where in `ci.yml` |
|---|---|
| **Which setup each lane needs.** The table has `[rust_components]` (G01–G09b only) and a command string. It has no "this lane is MSRV / stable / Python / mdBook / none". G13 needs only checkout; G10a needs stable + cargo cache + `cargo install mdbook`; G10b/G11/G14/G15 need setup-python; G07/G07b/G08 need components. That mapping is *inferred from the command text today by a human*. | every job body |
| Triggers | lines 8-11: `push: branches: [main]`, `pull_request`, `workflow_dispatch` |
| `permissions: contents: read` and the top-level `env: CARGO_TERM_COLOR: always` | lines 13-17 |
| Per-job `env: TZ: UTC` — on G02, G04, G05, G06 only | lines 84-86, 137-138, 164-165, 191-192 |
| Step order, and the apt-deps step (`sudo apt-get update && … libssl-dev pkg-config`, 10 times) | each Rust job |
| The cargo cache: `path`, `key: ${{ runner.os }}-gate-input-v1-cargo-${{ hashFiles('**/Cargo.lock') }}`, `restore-keys` — 12 uses of `actions/cache` | Rust, G10a and fixtures jobs |
| The `name:` display strings (e.g. `"G01 — build (1.95, default)"`) | every job |
| **Three jobs with no `[gates]` entry:** `ui-invariants-v1` (G12: Bash ≥5.2 assertion, negative self-tests, blocking gate — 60 lines), `gate-inputs` (Python setup, environment record, negative self-tests, blocking check), `gate-matrix-fixtures` (both toolchains, cache, mdBook, Python, and **six** evidence-block self-test steps for G01–G08, ci-gate, G10a, G10b, G11, G15) | lines 365-425, 473-520, 521-639 |
| The evidence-block shell (`set +e; echo command=…; started_at; …`) repeated inline in 10 steps that do not go through `ci-gate.sh` | the three jobs above |
| **Design rationale in comments** (76 lines): why no `target/` cache (D3), why G13 "had never run in CI", why G15 was held as an exception, why the G12 advisory annotations are gone | throughout |
| No `concurrency`, `timeout-minutes`, `needs`, `if`, `continue-on-error`, `strategy`, `upload-artifact` (measured absent) | — nothing to preserve, which helps |

So "generate `ci.yml` from the table" is really "generate it from the table **plus a new lane-profile schema plus a template**", and the three non-lane jobs (about a third of the file) need either their own input or to be written by hand into the template. That is a design, not a rename. The RFC should say what the generator's *inputs* are, and what it does with the comments (a generated file cannot carry per-lane rationale; the rationale moves to the template or the table, or is lost — the last is a real cost, because those comments are where a reader learns why G13 exists).

### High

**H1 — "Eight conditions collapse to one" is refuted; the handoff's per-condition question has a different answer.**
`scripts/check-gate-inputs.sh` says of itself (lines 10-22) that it runs **nine** things (precheck + conditions 1-8), and condition 7 is six checks plus a rule. My reading of each (item 9), with lines:

| # | What it checks | After generation of `ci.yml` | RFC's claim |
|---|---|---|---|
| 0 | manifest is valid TOML | **Survives** — the generator's own input; also feeds `ci-gate.sh` | — |
| 1 (`:130-136`) | every `uses:` in **every** workflow is a 40-hex SHA | **Survives, at least for `audit.yml` and `fuzz.yml`** — the grep runs over `$workflows_dir` (`:127`), not `ci.yml` only. Trivially true for a generated `ci.yml`, so it can be *scoped*, not removed | **Refuted as "replaced outright"** |
| 2, 3 (`:142-154`) | every workflow SHA is in `[actions]`; every `[actions]` SHA is used somewhere | **Survive.** Per-workflow use (measured): `checkout_v6` ci 20 / audit 1; `checkout_v4`, `upload_artifact_v4`, `rust_toolchain_nightly` are used **only in `fuzz.yml`**; `cache_v5` ci 12 / fuzz 1; `rust_toolchain_stable` and `setup_python_v7` only in ci. Three of seven pins never appear in `ci.yml`, so a generator over `[actions]` for `ci.yml` cannot own the table | not claimed, but D4 implies it |
| 4 (`:156-216`) | `[rust_components]` has each G01–G09b with the right components | **Survives, and is stranger than it looks:** it checks the TOML against an array **hard-coded in the script** (`expected_components`, `:160-163`). That is a *third* copy of `[rust_components]`, and it never reads `ci.yml`. Generation makes the script's array redundant only if the generator consumes `[rust_components]` | not addressed |
| 5 (`:218-236`) | `version`, `gate_matrix_version` = 1 | **Survives** (manifest-internal; the generator can assert it) | not addressed |
| 6 (`:237-282`) | every gate-lane job runs on `[runner]` | **Replaced.** Confirmed | replaced — **confirmed** |
| 7 (`:285-…`) | six checks: owner per lane; owners resolve to one RFC; every lane in a source RFC's table is in `[gates]` or the exceptions; **every `[gates]` command byte-matches its owning RFC's table row**; disjointness; exceptions grounded | **Survives whole.** It reads no workflow. It is about RFC 093/094/098's markdown | not mentioned |
| 8 (`:563-…`) | each `[tools]` version equals what `ci.yml` installs | **Replaced for `ci.yml`.** Confirmed — with one hole (below) | replaced — **confirmed** |

The hole in 8: `check_tool_pin`'s own comment says `rust_msrv` and `python` are "also checked transitively, since the gate commands embed them and condition 7 compares those". Measured: `[gates]` G01-G04 hard-code `cargo +1.95 …` and the RFC 093 table does too. So the MSRV lives in `[tools]`, in four `[gates]` commands, in four RFC-093 table rows and in `ci.yml`. Generation removes the `ci.yml` copy and leaves the other eight, and it removes the one check (8) that compared `[tools]` to anything. After D4, bumping MSRV in `[tools]` alone would be caught only by nothing.

Net: 6 and 8 collapse into the generator; 0, 1 (scoped), 2, 3, 4, 5, 7 stay. "Collapse to one" should read "collapse from nine to seven, and two of the survivors change job."

**H2 — A third copy of every lane command lives in RFC prose, and the RFC never mentions it.**
`[gates]` is "the source". But condition 7 check 4 (`check-gate-inputs.sh:519-541`) requires each `[gates]` command to **byte-match a row in the owning RFC's markdown table** (`[gate_owners]`: 093 for 14 lanes, 094 for G13, 098 for G14/G15). Measured: RFC 093 line 117 carries `bash scripts/check-ui-invariants.sh --all --policy ci/ui-invariants.toml`, and the same table carries `cargo +1.95 …`. So a command exists in `gate-inputs.toml`, in a `done/` RFC, and in `ci.yml`'s echo lines, and a gate reconciles the first two. D4 removes the third and calls the job done, but the pattern RFC 116 condemns — a fact stated twice with a reconciler — survives for the most important fact (what a lane runs). Any command change edits an RFC whose status is Done. The RFC should either say the RFC tables are the accepted, deliberate exception (they are the *reviewed* record) or make the tables generated/checked from `[gates]` the way it wants the inventory done.

**H3 — Overlap with RFC 094's planned `audit-structure` gate: two gates, two authorities over one file.**
RFC 094 (`rfcs/accepted/094-transactional-audit-registry.md`, "Structural coverage gate", lines 701-740) plans `cargo +stable xtask audit-structure --policy ci/write-authority.toml --commands ci/write-commands.toml`, which "fails if … a manifest row lacks a registry descriptor … generated documentation differs", checks each row's "event kind and class" and its failure-injection test, and is to be "a registered Gate Matrix lane … the authority". RFC 116 stage 1 builds a gate over the same `ci/write-commands.toml` for the same purpose, and item 13 asks to fill `event`/`descriptor` — which is 094's row schema. Measured: there is **no `xtask` directory**, no `syn` in `Cargo.toml`, and `ci/write-authority.toml` does not exist (`ls ci` shows six files). RFC 116 does not mention `audit-structure`, and RFC 094 does not know about RFC 116.

Nothing is wrong yet, but the RFC should say, in its own words: stage 1 is an **interim gate** that 094's `audit-structure` will subsume, which conditions of 094's list it takes now (id ↔ code, `files` exist, counts derived), and which it leaves (the `syn` AST boundary, failure-test presence). Otherwise the project will have two gates over the same file with two definitions of "true" — the very thing D1–D3 exist to prevent. RFC 094 needs a one-line pointer; that edit is the architect's.

**H4 — G13 is weaker than "bidirectional on names" says: 24 of 56 rows can be deleted without the gate noticing.**
Read of `scripts/check-audit-matrix.sh`: `MATRIX_NAMES` (`:67-70`) is every backtick-quoted `word.word` string **anywhere in the file**, in a derived namespace — table cells, prose, notes, the "Corrected …" paragraphs. It is not "the Event name column". Three mutations on a scratch copy of the matrix (script copy pointed at it; tree untouched):

| Mutation | Result |
|---|---|
| baseline | PASS: 56 matrix entries, 56 source literals |
| delete the `user.enable` row (its name still appears in prose at line 29) | **PASS** (56/56) |
| flip `user.create` A → B | **PASS** |
| flip all 36 `A` cells (`| A |` and `| **A** |`) to `B` | **PASS** |

`python` over the file: **24 of the 56 rows** have their name backticked somewhere else in the file, so deleting the row leaves G13 green. The RFC's premise ("G13 checks names in both directions") holds for a *new source event with no mention anywhere*, which is the case it was built for; it does not hold for "every event has a row". Any D3 implementation has to parse *rows* (the Event-name cell of table lines), not names, or the new class check inherits the hole.

### Medium

**M1 — The class check is decidable by text scan, but the matrix is a harder parse than the RFC assumes, and G13's lane is a bash-and-grep job.**
- The matrix has **five table schemas**: `Event name|Operation|Actor|Target|Note fields|Class` (user, client, signing key, admin, setup, pending, federation, dynamic-register, SMTP, step-up); `Event name|Trigger|Actor|Target|Note fields|Class` (mfa, passkeys, tokens, step-up); `Event name|Trigger|Actor|Class` (the auth-flow table — **10 of the 25 descriptor rows live here and have no Target and no Note column at all**); `Event name|Trigger|Actor|Class` again under OAuth2; and the class-definition table at the top. A parser must read each table's header to know where `Class` and `Actor` are.
- The `Class` cell has a grammar: `A`, `B`, `**A**`, and `**A** (wrong password) / B (refused before a credential check)` for `auth.login.failure`. The check can only verify the `A` half of a mixed cell; the B half is a separate best-effort writer and is not visible to a descriptor scan. State that.
- Markdown cells contain `\|` (e.g. `via=web\|cli` in the `user.recovery_link.issued` row), which a naive `split('|')` cuts in two — I hit exactly this while measuring (the note column of that row truncates to `reason=… via=web\`).
- `scripts/check-audit-matrix.sh` is "bash and grep only — no toolchain and no Python" and the G13 job is checkout plus dispatcher, no `setup-python` step. A five-schema markdown parse in awk is possible and would be the least maintainable code in `scripts/`. My view: the parse belongs in Python (as G11/G15 already are), called from the same lane — but the job then needs Python (system `python3` on `ubuntu-24.04` or the pinned setup step), the `[gates]` G13 command must stay byte-equal to RFC 094's table row (H2) or that row must change, and `scripts/tests/fixtures/gate-matrix/audit-in-sync` and `audit-desync` (used by `check-gate-matrix-fixtures.sh:180-200`) have no `commands.rs`, so the new check must either tolerate its absence (as the existing name-derivation already does, `:44-46`) or the fixtures grow one. Both are fine; neither is in the RFC.

**M2 — Actor is derivable; target and attributes are not, and I would not attempt those two.** (Detail in answer 6.)

**M3 — The local-run property is safe from generation, and was never as exact as the RFC says.**
(Answer 10.) Generation does not touch `scripts/ci-gate.sh` or `[gates]`, so it does not threaten the property. But "runs a lane exactly as CI runs it" is already false in one place: `G02`, `G04`, `G05` and `G06` set `TZ: UTC` in the workflow only (`ci.yml:84-86` etc.); the dispatcher does not, so `bash scripts/ci-gate.sh G02` on a machine with another timezone can differ from CI. That is a pre-existing gap, and D4 is the moment to close it (make `TZ` part of the lane's input and have the dispatcher export it), or the "exactly" in the RFC should be softened.

**M4 — Stage 2 needs a negative fixture per checked column, and the registry has no runtime enumeration.**
The handoff asks for a mutation per column. That is achievable, but the `Class` check needs a fixture `commands.rs`-shaped file (see M1) and the descriptor list has to come from source text: `all_descriptors()` (`commands/tests.rs:31-57`) is hand-kept, and its own comment (`:103-107`) says whether a command implements `SystemPrincipalPermitted` "isn't queryable as runtime data". A scan of `name:` / `class:` pairs found **25** of them, equal to the 25 entries in `all_descriptors()` and to the 25 `.class_a(` call sites in `commands.rs` — so today the text scan and the hand-kept list agree. Say in the RFC that the scan, not the list, is the authority, and that a test asserting scan == list is cheap.

### Low

**L1** The header of `ci/write-commands.toml` (lines 1-23) contains three claims a gate would falsify: "82" (89), "0 stale" (2 dead paths), and the `event`/`descriptor` "placeholders" text. The RFC already plans to derive or delete the counts; add "and the stale-path sentence".

**L2** `registry.rs:254` already has `generate_reference_markdown(&[&EventDescriptor])`, tested (`commands/tests.rs:226-239`) but deliberately not wired to a `docs/` file ("would publish a reference document that reads as authoritative while covering roughly 5% of the eventual registry"). It is the cheapest way to make D3's *actor/target/attributes* columns true by construction for the 25 registry rows (and the RFC does not mention it). It now covers 25 of 56 matrix rows, which changes that comment's premise.

**L3** RFC 104 (Proposed) says the matrix has "55 registered events"; it has 56 (G13 prints 56). It also lists `ci/audit-coverage-matrix.md` as a file it edits and binds labels to it by test. Stage 2 and RFC 104 touch the same file; whichever lands second re-runs the by-hand check.

**L4** Stage 4 (G12 through the dispatcher): RFC 093's table (line 117) already carries G12's command, byte-equal to the workflow's blocking step, so routing it through `ci-gate.sh` passes condition 7 check 4 as-is. What it would lose or need to move: the job's Bash ≥ 5.2 assertion (`ci.yml:401-402`), which the dispatcher does not carry; and `G12` would leave `[gate_matrix_exceptions]`, so check 5 (disjointness) must see it in `[gates]` only. The dated-decision alternative is cheaper and equally honest.

---

## 3. Answers to items 1–4 (request §1) — beyond the table

**1. Nothing reads `ci/write-commands.toml`.** Confirmed at the width asked: `scripts/`, `.github/`, `crates/**` (incl. every `build.rs` and every `include_str!`/`include_bytes!`), `Cargo.toml`, and a whole-tree `grep -rIl`. The only non-doc hit is a `///` comment at `crates/sui-id-store/src/registry.rs:439`. D2's conclusion for this file stands. (RFC 094 *plans* a reader — H3 — but has not built it.)

**2. The nine-row drift.** TOML ids (99): 92 markdown ids + **L01 L02 L03 L04 L05 L06 L07 U37** (8). Markdown-only: **U06** (1). Difference: nine. `U06` is the one RFC 103 stage 5 retired in the TOML (`37c10c2`); the markdown was never updated. The TOML has taken 12 commits since 2026-09-16 (13 in all); the markdown's last was an RFC-closure commit.

**3. Which matrix columns G13 verifies.** None of *the columns*. It extracts every backtick-quoted `word.word` string in a derived namespace from the whole file (`check-audit-matrix.sh:67-70`) and compares that *set* to the string literals in non-test `.rs` files (`:80-87`). Verified: the *existence of a name* somewhere in the file, in both directions. Not verified: operation, actor, target, note fields, class; that a name is in a *row*; that the row's cell says anything true. See H4 for what that lets through (24 of 56 rows deletable). The matrix's own "CI gate" section (lines 312-345) is accurate about this.

**4. The `ci.yml` counts.** All seven agree (table §1). The 57-use figure is a three-file sum (52 + 1 + 4).

---

## 4. D3 by hand over today's matrix (items 5-7)

### Item 5 — is Class-A decidable by text scan?

**Yes, for the `class` column, without `syn`.** The construct to key on is the **descriptor**, not the macro or `class_a` call:

- Every event a Class-A command can write is an `EventDescriptor { … name: "…", class: AuditClass::Atomic, … }` static in `crates/sui-id-store/src/commands.rs` (25 of them).
- `AuditClass` has only `Atomic` and `MustAttempt` (`registry.rs:118-124`); `MustAttempt` is **used by nothing** ("not implemented in this Stage-1 slice"), so *class B = no descriptor*, and the class check reduces to: a row says A ⇔ a descriptor with that `name` exists.
- Why a descriptor is enough, without the AST: `Database::class_a` (`registry.rs:637`) is the only runner; the only non-test `.class_a(` calls are in `commands.rs` (25 sites, 0 elsewhere — `grep -rn '\.class_a(' crates`); sealed commands come only from `declare_write_command!` (23 invocations in `commands.rs`, 1 in `registry.rs:996` which is the `proof_only` test artefact the existing G13 script already excludes, `check-audit-matrix.sh:37-39`). A sealed command carries its descriptors as an exhaustive match, so a descriptor named *X* is reachable only through a command that runs under `class_a`.
- What the scan cannot decide — and RFC 094's `syn` AST work (M2b) is for — is the *negative*: that no raw write bypasses the seam, i.e. that an event with **no** descriptor is not written inside a transaction by some other route. That is not D3's question (D3 asks whether a row's *claim of A* is backed), so **D3 does not need the AST and its cost and sequence are not changed by it.** State that limit in the RFC; the matrix's "What it does not catch" list should survive D3 with one bullet struck, not all four.
- Pattern: `name: "([a-z0-9_.]+)",\s*\n\s*class: AuditClass::Atomic` finds all 25. `AttributeSpec` also has a `name:` field, so the pattern must require `class:` on the following line (my first regex, without that, mis-parsed).

### Item 6 — actor, target, attributes

| Column | Registry supplies | Derivable today? | My recommendation |
|---|---|---|---|
| **actor** | `ActorRequirement::{Required, Optional, None}` on every descriptor | **Yes, for the 25 registry rows, with a three-token vocabulary:** cell starting `—` → `None`; cell containing `none for` → `Optional`; otherwise → `Required`. Fail closed on any cell that fits none. I ran it: **24 agree, 1 disagrees** (`auth.refresh.theft_detected`, B1) | **Build it.** Exact, small, and it already found a disagreement |
| **target** | `TargetRequirement::{Required, Optional, None}` — **all 25 are `Required`** | Vacuous. The matrix cell holds *which* id ("new key id", "target user id"), which the registry does not carry. 10 of the 25 rows have no Target column. Every cell that exists is non-empty, so the check passes by construction | **Do not attempt.** It cannot fail |
| **attributes** | `attributes: &[AttributeSpec { name, description, required }]` | Names: **not reliably.** The Note column is free prose. Measured on the 15 rows that have it: backtick extraction yields false names (`totp`, `webauthn`, `recovery_codes`, `return_to` are *values* or *sources*, not attribute names, in the step-up and factor rows); `reason (optional)` in the disable/delete rows is not backticked and is missed; 10 rows have no Note column at all. **`(required)` markers only:** exact — the five step-up rows (`user.disable`, `user.enable`, `user.delete`, `mfa.admin_reset`, `signing_key.rotate`) say `step_up` (required) and the descriptors carry `required: true` on exactly `step_up` (`commands.rs:47-52`) | **Do not attempt name parsing** (approximate, so worse than none). If wanted, check only the `step_up` (required) marker ↔ `STEP_UP_ATTRIBUTE` — five rows, exact. For the rest, generate the cell from the descriptors (`generate_reference_markdown`, L2) instead of parsing prose |

**The one I would not attempt:** target, and attribute *names*. The handoff's own rule ("an approximate check is worse than none") is the reason.

### Item 7 — the check, run over all 56 rows

Scripts: `d3.py` (class), `d3b.py`/`d3c.py` (actor, attributes) in the session scratchpad; not committed. Results:

- 56 rows; **37** claim Class A in whole or part; 19 claim B only.
- 25 sealed descriptors, all `AuditClass::Atomic`; **all 25 have a row claiming A** (reverse direction clean).
- **Every row claiming B has no descriptor** (the other direction of the class check is clean: 0 rows claim B while backed by an Atomic descriptor).
- **12 rows claim A and have no descriptor:**

| Matrix line | Row | Actual writer (measured) | Shape |
|---:|---|---|---|
| 70 | `client.create` | `crates/sui-id-core/src/identity/admin/clients.rs:101` → `audit_ok` | `let _ = audit::append(…)` after the write (`identity/admin.rs:31-52`) |
| 71 | `client.update` | `clients.rs:200`, `:250` → `audit_ok` | same |
| 72 | `client.set_allowed_scopes` | `clients.rs:135` → `audit_ok` | same |
| 73 | `client.set_post_logout_redirect_uris` | `clients.rs:162` → `audit_ok` | same |
| 74, 75 | `client.disable`, `client.enable` | `clients.rs:277-279` → `audit_with_note` | same |
| 76 | `client.delete` | `clients.rs:312` → `audit_with_note` | same |
| 77 | `client.rotate_secret` | `clients.rs:415` → `audit_with_note` | same |
| 84 | `signing_key.delete` | `identity/admin/signing_keys.rs:97` → `audit_with_note` | same |
| 90 | `admin.master_key.rotated` | `crates/sui-id/src/cli.rs:418-423` | `let _ = sui_id_store::repos::audit::append(…)` |
| 137 | `auth.federation.takeover_blocked` | `crates/sui-id/src/http/handlers/federation.rs:471-476` (via `AUDIT_TAKEOVER_BLOCKED`, `repos/federation_provider.rs:200`) | `let _ = …append(…)`, and the row's `result` is `"denied"` |
| 205 | `auth.smtp_config.changed` | `crates/sui-id/src/http/handlers/settings.rs:504-509` | `let _ = …append(…)` after `smtp_config::upsert` |

  None of these routes touches `append_within_tx`; its only production callers are in `registry.rs`, `backend.rs` and `repos/audit.rs`. All are **Class B in fact**. The matching inventory rows (`C01`–`C15`, `K02`–`K03`, …) are `class = "A"` with `status = "implemented"` — *target* class, per the B2 reading — which is probably how the matrix came to say `A`.
- **1 actor cell disagrees:** `auth.refresh.theft_detected`, line 280 (`user id`) vs `ActorRequirement::None`.
- **Not disagreements, but worth a line each:** `auth.login.failure` is mixed (`A` for a wrong password, `B` for pre-check refusals) — the scan verifies only the `A` half; `admin.user.unlock` says `— (see note)` for actor and the vocabulary handles it (`None` ✓); `mfa.admin_reset` and `user.recovery_link.issued` say "admin user id; none for the CLI" ↔ `Optional` ✓.

I have **not** edited a row.

---

## 5. Answers to items 8-15 and my views

**8. What in `ci.yml` is not derivable from the table.** The full list is in B3. In one sentence: the table has no *lane → setup* mapping, no triggers/permissions/env/cache/step-order, three of the twenty jobs have no lane at all, and 76 lines of rationale have nowhere to go.

**9. The eight conditions.** The per-condition table is in H1. Confirmed replaced: **6** and **8** (with the MSRV hole). Refuted: **1** (also applies to `audit.yml`/`fuzz.yml`). Survive: 0, 2, 3, 4, 5, 7. Item 7 is about RFC tables, not workflows, and is the largest.

**10. The local-run property.** Generation does not threaten it, provided the generator does not rewrite `[gates]` or move it to a multi-line TOML value: `scripts/ci-gate.sh` parses the table with a line-based `awk` on `KEY = ` (`ci-gate.sh:60-67`) and `check-gate-inputs.sh` does the same in four places — a formatter or generator that re-wraps a `[gates]` line breaks both silently (`ci-gate.sh` would then print "no [gates] entry"). Say "single-line values" in the RFC. The pre-existing gap is `TZ` (M3). Also: `ci-gate.sh` requires a git repository and a *clean tree* (`:96-104`), so a local run of a generated lane also stops on a dirty checkout; that is a feature and unchanged.

**11. `audit.yml` and `fuzz.yml`.** Neither reads the table. Evidence: `grep -n 'gate-inputs\|ci-gate\|ci/' .github/workflows/audit.yml fuzz.yml` finds one comment in `audit.yml:57-59` ("Gate Matrix pins in `ci/gate-inputs.toml`"), and no `run:` step executes or parses anything under `ci/`. Both hard-code their inputs: `audit.yml:60` has `runs-on: ubuntu-24.04` (a 21st copy of the runner label, outside every gate's view); `fuzz.yml:17` has `runs-on: ubuntu-latest` (a different, unpinned runner). `fuzz.yml` uses four SHA-pinned actions, all four recorded in `[actions]` (`checkout_v4`, `cache_v5`, `rust_toolchain_nightly`, `upload_artifact_v4`), so they *are* covered by conditions 1-3 — which is why those conditions must survive. Out of scope for generation, as the handoff says; but the RFC should say the runner label in `audit.yml` is a second, unchecked copy.

**12. Which inventory copy survives — my view (a view, not a ruling).** **The TOML.** Reasons, all measured:
- It is the copy being maintained (12 commits since 2026-09-16; the markdown's `U06` is the only row it has that the TOML does not, and that row is a *retirement*).
- It has the fields a gate needs that the markdown lacks: `status`, `files`, `test_id` (99 of 99 populated), and it parses in one call (`tomllib`, 99 rows, 10 keys, zero special cases).
- The markdown parse would cost: five tables, two header spellings ("Current mutation surface" vs "Atomic mutation surface"), six columns, **no `files` and no `status` column at all**, so a "every `files` path exists" gate is *impossible* against the markdown without adding those columns first. (The rows themselves are clean: 92 rows, all 8 pipe-fields, no escaped pipes.)
- The counter-argument (the handoff is where a reader looks first) is real but answerable: the markdown keeps the prose and gets a **generated** table (or a link and one paragraph), as RFC 116 says. If the owner prefers a table in the handoff, generate it from the TOML rather than parse it.

**13. Can `event` and `descriptor` be filled now?** **Yes, but only for the 23 sealed commands, and I would not fill them from a hand-written list.** Source: `command U01 = "U01" { … }` in `commands.rs` names the id, and each command's `descriptors` (or the event enum's `descriptor()` match) names the `EventDescriptor` statics; `name:` gives `event` (e.g. U01 → `user.create`, `user.create_warned_hibp`). It is regex-parsable — but a command has *several* descriptors (U01 two, U22 two, T04 two, L06 two, L07 two, L04, K01…), so `event`/`descriptor` would have to become lists, which is a schema change. For the other 76 rows the honest value is empty *and the column must allow empty for a stated reason*. Recommendation: **keep the columns, make them lists, fill them by a generator over `commands.rs`, and let the gate assert them** for sealed rows — rather than removing them (removing loses the `id ↔ event` link that the matrix and the code both need) or hand-filling. Note H3: this is 094's row schema; do it in one place.

**14. Is stage 3 correctly placed after 1 and 2?** Yes, and I would go one step further: **stage 3 should not start until its inputs are decided** (B3). Also, `ROADMAP.md` puts "RFC 116 stages 1–2, then RFC 112, then RFC 106, then RFC 116 stages 3–4" in cycle B, after cycle A's RFC 115, which changes `crates/sui-id-store/src/commands.rs` (its *Touches* list). RFC 115 may add or change descriptors, so **the by-hand D3 count above must be re-run at stage 2's baseline**, not taken from this review. Stage 4 (G12) is independent of stage 3 and could go earlier; I would leave it where it is.

**15. What cannot be built as described, and what the RFC omits.**
- *Cannot be built as described:* stage 1's "every `id` has a command" (B2); stage 3's "generated from the table" and "collapse to one" (B3, H1); stage 2 "green on the tree it lands in" (B1 — the rows must be *decided* first, not fixed in the same commit).
- *Better built another way:* the `class` check belongs in a Python parser called from G13's lane (M1); the inventory `event`/`descriptor` columns should be generated (13); the actor column is the only one worth a check (6).
- *Omitted:* RFC 094's `audit-structure` overlap (H3); the RFC-table copy of every lane command (H2); the hard-coded `expected_components` array in the checker (H1); the unchecked runner label in `audit.yml` (11); G13 keys on names not rows, so it does not detect row deletion (H4); `TZ` in the workflow only (M3); the `files` field is checked by nobody and two are wrong (B2); the comments in `ci.yml` are the record of why (B3).
- *Open questions 2 and 3* (`tolerance_minor = 2`, `maximum = 20`) were not put to me, and I have no measurement that speaks to them; I did not take a position.

---

## 6. RFC changes I recommend (for the architect; I changed none)

1. **D3 / Risks:** name the twelve rows and the one actor cell; make "owner rules on each row" a prerequisite of stage 2 rather than an expected finding. Restate "cited by the threat model" per §1.
2. **D1/D2 stage 1:** add a `sealed` (or equivalent) state to the inventory schema; write the direction rules per state; list `O04` and `X02` as findings; correct "currently correct".
3. **D3 stage 2:** key the check on the Event-name *cell of a table row*, not on names anywhere in the file (H4); scope to `class` and `actor`, plus the `step_up (required)` marker; say target and attribute names are deliberately unchecked and why; say Python for the parse.
4. **D4:** say what the generator's inputs are (new lane-profile schema; template for the three non-lane jobs; what happens to comments); replace "eight conditions collapse to one" with the per-condition disposition in H1; decide what to do about the RFC-table copy of commands (H2) and the `+1.95` in `[gates]`.
5. **D1–D3 vs RFC 094:** one paragraph that stage 1 is interim and `audit-structure` subsumes it, with the pointer added to 094 (H3).
6. **Local-run property:** "single-line `[gates]` values" and the `TZ` decision (M3).

---

