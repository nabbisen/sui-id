# RFC 116 — Gate contracts: one source, one gate each

**Status.** Accepted
**Accepted on.** 2026-09-22
**Approved by.** `@nabbisen`, 2026-09-22: "Well, RFC-116 is accepted." He ruled its two open questions on 2026-09-24 (D1, D3a).
**Security review.** Required
**Independent design review.** [Design review 2026-09-24](../handoffs/116-gate-contracts/design-review-2026-09-24.md) by the implementation role, which authored neither this RFC nor its handoff. Every measurement in §1 of this RFC was reproduced and agrees; three blockers and four high findings against its *remedies*, all resolved in this text. It also ran D3 by hand and disproved twelve rows of `contracts/audit-coverage-matrix.md`.
**Design prerequisites.** None outstanding. The twelve disproved Class-A rows were ruled by `@nabbisen` on 2026-09-24 and corrected the same day (D3a); open question 1 was ruled the same day (D1).
**Implementation prerequisites.** None for stages 1 and 2. Stage 3 waits until stages 1 and 2 have landed, so that a change to how every lane is defined happens on a tree whose contracts are already true.
**Closure prerequisites.** No fact stated in `contracts/` exists in a second hand-maintained place; every surviving file in `contracts/` is read by a gate that fails when the file stops being true; the audit matrix's `class` and `actor` columns are checked against the code, keyed on table rows rather than on names anywhere in the file; every lane runs through one dispatcher or its exception is a dated decision; and the placement of what survives is settled.
**Tracks.** Gate integrity. Raised by `@nabbisen` on 2026-09-22: "`ci/` seems also messy and dirty because partially duplicate of `.github/workflows/`."
**Touches.** `contracts/`, `.github/workflows/ci.yml`, `scripts/check-audit-matrix.sh`, `scripts/check-gate-inputs.sh`, `scripts/ci-gate.sh`, `scripts/check-ui-invariants.sh`, `scripts/tests/`, `rfcs/handoffs/094-transactional-audit/command-inventory.md`, and a one-line pointer in `rfcs/accepted/094-transactional-audit-registry.md` (D6).
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/116-gate-contracts/README.md`](../handoffs/116-gate-contracts/README.md)

*Amended 2026-09-25 (stage 7), on `@nabbisen`'s ruling of the same day: the directory
this RFC calls `ci/` is now `contracts/`. Its normative statements (the closure
prerequisites, the touched paths, D2) say `contracts/`; its narrative (the Summary's
measurements, the quoted request, the dated corrections) keeps the name the directory
had when they were made.*

## Summary

`ci/` holds six files. Read one by one against what they claim, only two are
sound:

- **`write-commands.toml`** — 60 KB, 99 entries, **read by no gate**. It is a
  second copy of `rfcs/handoffs/094-transactional-audit/command-inventory.md`,
  and the two have drifted nine rows. Its header counts are stale, two of its
  `files` paths name files that have never existed, and `event` and
  `descriptor` are empty in all 99 entries.
- **`audit-coverage-matrix.md`** — gated, but on one column of five, and
  **more weakly than that**: G13 extracts every backticked `word.word` string
  from anywhere in the file, so 24 of its 56 rows can be **deleted entirely
  without the gate noticing**. Run by hand, its `class` column is wrong in
  twelve places.
- **`gate-inputs.toml`** — its `[gates]` table is a genuine single source and
  is kept. Every other section is copied into `.github/workflows/ci.yml`: the
  runner label 20 times, the system packages 10, the stable toolchain 8, MSRV
  6, Python 6, mdbook 2, the components 4, and the lane list as jobs.
  `scripts/check-gate-inputs.sh` exists to prove the copies still agree.
- **`rfc-policy.toml`** and **`doc-authority.toml`** — small, documented in
  place, exercised by tests, no duplication. These are the standard, and the
  second states in its own header the rule the rest of the directory breaks:
  *"a machine-consumed contract… The gate that reads it is the only thing that
  makes it true."*
- **`ui-invariants.toml`** — sound, but G12 is the only lane still outside the
  dispatcher.

## Corrections to this RFC's own text

1. **"The matrix is cited by `docs/threat-model.md`" is false.** The threat
   model never names the file; `grep -in matrix docs/threat-model.md` returns
   nothing. It rests on RFC 102's Class-A claim, not on the matrix. The citers
   are RFCs 085, 094, 098, 101, 102, 103, 104 and this one, plus
   `docs/development-specification.md` and `docs/src/contributing/architecture.md`.
2. **"`write-commands.toml` is the copy that is currently correct" is false.**
   It is the copy being *maintained* — twelve commits since 2026-09-16 against
   the markdown's last being an RFC-closure commit — but two of its `files`
   paths (`O04` → `repos/backup.rs`, `X02` → `repos/runtime.rs`) name files
   that have never existed in the repository's history.
3. **"Seventeen jobs" is seventeen `[gates]` keys**; `ci.yml` has 18 lane jobs
   (the 18th is G12, the known exception) plus 2 that are not lanes.

## Why this is not a rearrangement

The layout cannot be decided until it is known which files survive.
`write-commands.toml` duplicates `command-inventory.md`; one of the two should
stop existing. Choosing a home for a file that may be deleted, at a cost of 47
referencing files, buys nothing. **Moving a broken, ungated registry into a
better-named directory leaves a broken, ungated registry.**

## Decisions

**D1 — One source per registry: the TOML survives.** Ruled by `@nabbisen`,
2026-09-24, on the design review's measurement: `contracts/write-commands.toml` is the
copy being maintained, it carries `status`, `files` and `test_id` on 99 of 99
rows, and it parses in one call — while
`rfcs/handoffs/094-transactional-audit/command-inventory.md` has **no `files`
and no `status` column at all**, so "every `files` path exists" is not
expressible against it without adding columns first. The markdown keeps its
prose, which is the part the TOML cannot carry, and loses its table; if a table
is wanted there it is **generated** from the TOML, never parsed back out of it.

**D2 — Every file in `contracts/` is read by a gate that fails when it stops being
true.** A file no gate reads is not a contract; it is a document that looks
like one, which is worse, because a reader trusts it. Anything that cannot be
given a gate leaves `contracts/` and becomes documentation, labelled as such.

**D2a — The inventory schema separates *sealed* from *planned*.** The review
showed the gate as first worded cannot be green: **76 of 99 rows name no
command in `commands.rs`**, because the inventory records planned conversions
too — `class = "A"` on 67 rows is the *target* class and only 23 have a sealed
command. Those rows are true; the gate was wrong. So the schema gains a state
distinguishing sealed from planned, and the direction rules become: **code →
TOML complete for every sealed command; TOML → code only for rows in the sealed
state; `files` checked for every row that claims an implementation.** The two
dead paths are reported, not silently fixed.

**D3 — The matrix's `class` and `actor` columns are checked against the code.**
A row claiming Class-A while its command is not on the seam is a false security
claim. `actor` is exactly derivable from `ActorRequirement` and already found
one disagreement. **`target` and attribute *names* are deliberately not
checked**: every target cell passes by construction, and the Note column is
free prose whose backticks yield values rather than attribute names — an
approximate check is worse than none. Only the `step_up (required)` marker is
checked, which is exact over five rows.

**D3a — Twelve rows were wrong; ruled and corrected 2026-09-24.** Run by hand, twelve rows claim `A` with no descriptor, and the code
that writes each is `let _ = audit::append(…)` after the state change — Class B
in fact: `client.create`, `client.update`, `client.set_allowed_scopes`,
`client.set_post_logout_redirect_uris`, `client.disable`, `client.enable`,
`client.delete`, `client.rotate_secret`, `signing_key.delete`,
`admin.master_key.rotated`, `auth.federation.takeover_blocked`,
`auth.smtp_config.changed`. Unlike the auth section, which carries an explicit
"Class B only until its command is converted" caveat, these say `A` unqualified.
Separately, `auth.refresh.theft_detected`'s `actor` cell says "user id" where
its descriptor says `ActorRequirement::None` — which also falsifies the file's
own sentence that "each row below was checked against its command's descriptor
on this date". `@nabbisen` ruled on 2026-09-24 that the rows take the treatment the
auth-flow section already gives its own unconverted rows. They now read
`B *(A required)*` — Class B is what the code does, Class A is what the
document requires — and **RFC 094 M2b converts them**, its scope being settings,
pending settings, federation configuration and client metadata. The actor cell
is corrected to `—`. Both landed in `contracts/audit-coverage-matrix.md` under a dated
"Class corrections" block, with G13 still green because no event name changed.
**Stage 2 therefore lands on a matrix that is already true**, and its job is to
make it stay true.

**D3b — The check keys on table rows, not on names.** G13 today extracts
backticked `word.word` strings from anywhere in the file — prose, notes,
corrections — so deleting a whole row leaves it green whenever the name also
appears in prose. **Measured: 24 of 56 rows, verified by deleting the
`user.enable` row and watching G13 report 56/56 and exit 0.** Any class check
built on the same extraction inherits the hole, so the parse must read the
Event-name cell of a table line. The matrix has five table schemas and cells
containing escaped pipes, so the parse belongs in Python beside G11 and G15
rather than in the existing bash, and the lane gains a Python step.

**D4 — `ci.yml` is generated, and the RFC states what the generator's inputs
are.** "Generated from the single table" was too strong: the table carries no
lane→setup mapping, no triggers, permissions, per-job `env`, cache keys, step
order or display names, and three of the twenty jobs have no lane at all. The
inputs are therefore **the table, a new lane-profile schema, and a template**
carrying the non-lane jobs. `ci.yml` also holds 76 lines of design rationale —
why there is no `target/` cache, why G13 had never run in CI — which a
generated file cannot carry; that rationale moves to the template or the table
and is **not** to be lost.

**D4a — "Eight conditions collapse to one" is withdrawn.** Measured
per-condition: 6 and 8 are replaced by generation; 0, 2, 3, 4, 5 and 7 survive;
1 survives scoped, because it also covers `audit.yml` and `fuzz.yml`, and three
of the seven action pins appear only in `fuzz.yml`. Condition 7 is the largest
and reads no workflow at all. The honest statement is **nine to seven, with two
survivors changing job.** One hole to close in the same change: condition 8 is
the only check comparing `[tools]` to anything, so after generation an MSRV
bump in `[tools]` alone would be caught by nothing.

**D4b — The third copy of every lane command is named, not ignored.**
Condition 7 requires each `[gates]` command to byte-match a row in its owning
RFC's markdown table, so every command exists in `gate-inputs.toml`, in a
`done/` RFC, and in `ci.yml`. D4 removes the third. The RFC tables are the
**reviewed record** of what a lane was accepted to run, and that is a
deliberate, stated exception rather than an oversight — but it means any
command change edits a Done RFC, and this RFC says so instead of pretending the
duplication is gone.

**D5 — Every lane runs through the dispatcher**, or G12's exception is restated
as a dated decision with a reason that is still true. Routing it costs moving
the job's Bash ≥ 5.2 assertion, which the dispatcher does not carry.

**D6 — Stage 1 is an interim gate that RFC 094's `audit-structure` subsumes.**
RFC 094 plans `xtask audit-structure --policy contracts/write-authority.toml --commands
contracts/write-commands.toml` as "the authority" over this same file, including the
`event` and `descriptor` columns this RFC would fill. Neither RFC knows about
the other, and the xtask does not exist. Stage 1 takes the conditions available
without the AST — id ↔ code, `files` exist, counts derived — and leaves the
`syn` boundary and failure-test presence to 094. A pointer is added to RFC 094;
without it the project gets two gates over one file with two definitions of
true, which is what D1–D3 exist to prevent.

**D7 — The local-run property is preserved explicitly.** `scripts/ci-gate.sh`
parses `[gates]` with a line-based `awk`, so the generator must never re-wrap a
`[gates]` value: **single-line values only**, or both it and
`check-gate-inputs.sh` break silently. The property is also **already** weaker
than this RFC claimed: `TZ: UTC` is set in the workflow for G02, G04, G05 and
G06 and not by the dispatcher, so those lanes do not run locally exactly as CI
runs them. D4 is the moment to close that or to soften the claim.

**D8 — Placement is decided last**, on the set of files that survives, in one
commit with every reference updated in it.

## Open questions

1. **`doc-authority.toml`'s `tolerance_minor = 2`**, marked in the file as "a
   starting value for `@nabbisen` to adjust: it is chosen, not derived", and
   never adjusted.
2. **`ui-invariants.toml`'s `inline-style-bound maximum = 20`**, a ratchet with
   no record of when it was last tightened.

## Risks

- **D3a was a correction to shipped security claims, now landed.** Twelve rows
  of a normative file stated a guarantee the code does not provide. Nothing was
  *newly* broken — the code has always been what it is — but the file had been
  read as authoritative by four RFCs for as long as the rows existed.
- **D4 changes how every lane is defined**, and is sequenced last of the
  functional stages for that reason. A generated workflow missing a lane fails
  invisibly, so the generator's test asserts the lane set round-trips.
- **The by-hand D3 count must be re-run at stage 2's baseline.** RFC 115 is
  scheduled before this in cycle A and touches `commands.rs`, so it may add or
  change descriptors. The twelve are a measurement of 2026-09-24, not a
  constant.

## Gate Matrix lanes owned by RFC 116

Registered through the multi-source lane registry (RFC 094 R10), as RFC 098 and
RFC 117 do. The heading above is the recorded source heading and is matched by
plain equality; do not rename it without changing the manifest in the same
commit. Column layout mirrors RFC 093's table so one parser reads both. Added
with stage 1 (D1, D2, D2a); later stages add their own rows here (G18, stage 6).

| ID | Toolchain | Features | Blocking command / assertion |
|---|---|---|---|
| G17 | Python 3.14 | n/a | `python3.14 scripts/check-write-commands.py --root . --inventory contracts/write-commands.toml` |
| G18 | Python 3.14 | n/a | `python3.14 scripts/check-contracts.py --root . --policy contracts/contract-paths.toml` |
