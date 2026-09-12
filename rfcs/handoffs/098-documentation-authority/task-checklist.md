# RFC 098 reconciliation checklist

**Governing RFC:** [RFC 098](../../accepted/098-documentation-authority-reconciliation.md)

## 0 — Order of work, added at acceptance 2026-09-10

**The authoritative order is RFC 098 §Design 5**, a seven-step plan ordered by
severity and measured against `ee48257`. Work it in that order; this checklist
is the wider content sweep that follows, and its §4 contradictions merge into
step 6.

Steps 1–3 close every public-facing falsehood and come first:

1. Staleness banner on `docs/threat-model.md` — it declares itself current as
   of v0.26.0 at a v0.77.0 workspace, and `README.md` presents it publicly.
2. Delete `docs/deployment.md`, `docs/operators.md`, `docs/integrators.md` —
   stale forks of the book pages, holding no unique content and one false
   claim — and repoint `README.md` at the book pages with **repo-relative**
   links, not `https://github.com/...` URLs, which G10b cannot see.
3. Repoint `crates/sui-id/src/cli.rs` and
   `crates/sui-id/src/http/handlers/setup.rs`, which cite a deleted path.

Step 3 is the only one that touches `crates/`. Do not start step 4 or later
without a dispatch; steps 5 and 7 depend on open question 1, which acceptance
did **not** decide.

### Dispatch 2 — 2026-09-12: the last absolute self-URLs, then step 4

**Landed 2026-09-12** — 2a `e06b8b8` (two of three; the third was correct under
rule 6 and stays), 2b `c451c52`. Review:
`.git-exclude/reviewed/rfc-098-dispatch-2-and-g14-g15-2026-09-12.md`.

Steps 1–3 landed as `2671822` (reviewed:
`.git-exclude/reviewed/rfc-098-steps-1-3-2026-09-10.md`). Two small items,
both fully specified; do them as **two commits**, in this order.

**2a — Replace the remaining absolute self-URLs.** The count of twenty-six
in `README.md` §Findings above was measured before steps 1–3 and is stale.
Re-measured at `c97027c`, excluding `rfcs/done/`, `docs/changelog/` and
`CHANGELOG.md` (historical, rule 4), and excluding RFC 098's own text that
*describes* the pattern with `…` placeholders, **three** remain:

| File | Line | Target | Replace with |
|---|---|---|---|
| `README.md` | 205 | `…/blob/main/docs/threat-model.md` | `docs/threat-model.md` |
| `README.md` | 208 | `…/blob/main/PUBLISHING.md` | `PUBLISHING.md` |
| `docs/src/getting-started/overview.md` | 37 | `…/blob/main/ROADMAP.md` | `../../../ROADMAP.md` |

Why: an absolute URL to the project's own file is a repo-relative link
written wrongly. G10b skips external links, so it cannot see whether the
target exists — the README's link to F1's own document was invisible to the
gate for that reason. The third one is a book page: mdBook renders a
relative link to a file outside `docs/src/` as a broken page link on the
published site, so check the rendered output, not only the link checker.
If the rendered link is broken, say so and stop; do not substitute a
different target on your own.

Verify by re-running the measurement above: it must return zero. Report the
command and the count.

**2b — Step 4: evict the audit-coverage matrix to `ci/`.** Measured blast
radius at `c97027c`; smaller than RFC 098 §5 step 4 says — **RFC 093 does
not reference the path**, so there is no closed-RFC edit. That sentence in
step 4 was wrong.

```
git mv docs/src/reference/audit-coverage-matrix.md ci/audit-coverage-matrix.md
```

then repoint exactly these:

| File | What |
|---|---|
| `scripts/check-audit-matrix.sh` | line 4 (comment) and line 14 (`MATRIX=`) |
| `crates/sui-id-store/src/commands.rs` | line 543, a code comment naming the path |

Leave as written: the three dated RFC 093 review records under
`rfcs/handoffs/093-…/`, the superseded note at
`rfcs/handoffs/094-transactional-audit/migration-checklist.md:341`, and
RFC 101 §6.2 — those name the bare filename or are historical prose, and
stay true or stay dated after the move. RFC 098 §2 already names
`ci/audit-coverage-matrix.md` as the authoritative location; its F4 and
step 4 text are the record of why and stay.

The file has no outward links and nothing under `docs/` links to it, so
mdBook is unaffected; it was never in `SUMMARY.md`. `ci/` holds only
`.toml` today — a `.md` there is new, and correct: it is D3, a
machine-consumed contract, and the gate that reads it is the only thing
that makes it true.

Evidence: `bash scripts/check-audit-matrix.sh` must still report
**54 matrix entries, 54 source literals** — the same numbers as before the
move, which is the proof the script found the file at its new path rather
than passing on an empty read. Plus G10a, G10b, G11, fmt, both clippy
scopes (the `commands.rs` comment edit touches a crate).

**Not in this dispatch:** step 5 (the seven MI matrices) and step 7 wait on
open question 1; step 6 (`development-specification.md`) is a separate
dispatch. One further item is with the owner, not with you.

### Dispatch 3 — 2026-09-12: RFC 098's own lanes, G14 and G15

**Landed 2026-09-12** — G14 registered `73e49a2`; G15 built and held `336b9ab`.
G14 caught a real break on its first run (the matrix link in a dated 093 record).

Independent of dispatch 2. RFC 098 now declares its lanes in §Gate Matrix lanes
owned by RFC 098, registered through RFC 094's R10 registry; open question 2 is
answered by that. Handoff: [`g14-g15-doc-lanes.md`](g14-g15-doc-lanes.md).
G14 registers now (zero new code, green today). G15 is built now and held in
`[gate_matrix_exceptions]` until steps 4–6 clear the tree.

### Dispatch 4 — 2026-09-12: the outside-book link class, and check (B) under rule 6

**Landed 2026-09-12** — 4a `9bfaecf` (nine links, not the stated ten: five
threat-model links, not six), 4b `30f16a6`. G15 on the tree: 7 (A), 0 (B), 1 (C).
Review: `.git-exclude/reviewed/rfc-098-dispatch-4-and-g13-2026-09-12.md`.

Dispatches 2 and 3 landed (review:
`.git-exclude/reviewed/rfc-098-dispatch-2-and-g14-g15-2026-09-12.md`). The §1
stop condition produced a rule — RFC 098 §4 rule 6 — and this dispatch applies
it. Two commits.

**4a — four links, the other direction.** These render to files the book never
produces (`../../../X.html`). Convert each to the absolute repository URL, the
form `overview.md:37` already has and which was right all along:

| File | Line | Now | Becomes |
|---|---|---|---|
| `docs/src/contributing/local-dev.md` | 118 | `../../../rfcs/done/000-rfc-lifecycle-policy.md` | `https://github.com/nabbisen/sui-id/blob/main/rfcs/done/000-rfc-lifecycle-policy.md` |
| `docs/src/contributing/state-contract.md` | 121 | `../../../crates/sui-id-i18n/STATE_WORDS.md` | `…/blob/main/crates/sui-id-i18n/STATE_WORDS.md` |
| `docs/src/guides/operators.md` | 6 | `../../../README.md` | `…/blob/main/README.md` |
| `docs/src/reference/oidc-api.md` | 482 | `../../../ROADMAP.md` | `…/blob/main/ROADMAP.md` |

The other `../../threat-model.md` links inside `docs/src` — the six the
steps 1–3 review left for the owner — are the **same class** and are in scope
here too: `docs/threat-model.md` is outside the book. Convert them the same
way. Measure the full set with `grep -rnoE '\]\((\.\./)+[^)]*\)' docs/src`
filtered to targets that resolve outside `docs/src/`, and report the count
before and after; the after must be zero.

**4b — check (B) enforces rule 6.** In `scripts/check-doc-authority.py`: a link
whose source is under `docs/src/` and whose target starts with the self-URL
prefix is **allowed if and only if** the stripped path (after `blob/<ref>/`)
resolves to a tracked file *outside* `docs/src/`; it fails if the file does not
exist, or if it is inside `docs/src/` (a book page linking to a book page must
be relative). Outside `docs/src/`, every self-URL still fails. Also add the
inverse: a relative link from a `docs/src/` page whose resolved target is
outside `docs/src/` fails, naming rule 6 — that is the four links above, and
without this half the rule is enforced in one direction only. Tests: absolute
outside-book from a book page passes; absolute to a missing file fails;
absolute to a book page from a book page fails; relative outside-book from a
book page fails; the existing outside-`docs/src` cases unchanged.

**Evidence.** G15 on the tree after both commits: **7 (A), 0 (B), 1 (C)** —
the (B) count falls to zero because `overview.md:37` becomes sanctioned and
the four converted links stop being relative-outside. Report the run verbatim.
mdBook build clean; G10b, G11, G14 green; Python suite green with the new
tests counted.

**Not in scope.** Registering G15 (still waits on steps 5 and 6); any change to
`check-markdown-links.py`; the MI matrices (open question 1).

### Dispatch 6 — 2026-09-12: step 6c, cut the specification to what holds

**Landed 2026-09-12** — one commit, one file; 59 kept sections byte-identical, G15
7/0/0 with no banner. Review: `.git-exclude/reviewed/rfc-098-dispatch-6-2026-09-12.md`.
Step 6 is complete. Every remaining G15 violation is step 5.

Dispatch 5's audit (review: `.git-exclude/reviewed/r10-c-and-spec-drift-audit-2026-09-12.md`)
decided the disposition: **cut, do not reconcile.** Every principle section
holds; every enumeration drifted because it is a second copy of an
authoritative artifact. One commit, one file, plus `ci/doc-authority.toml` only
if the claim regex needs the new header form (it should not — keep the phrase
"reflecting the v0.77.0 codebase").

**Header.** `*v4 — reflecting the v0.77.0 codebase (2026-09-12). Supersedes v3
(v0.48.4). Under RFC 098 this document is a synthesis of policy and principle,
not a source: inventories were removed in v4 and each section that held one
now names where the authoritative copy lives.*` Remove the v3 paragraph and
the step-6a staleness banner — within tolerance again, and check (C) will
force the banner back if it lags.

**Keep verbatim** — the sections dispatch 5 marked *holds*: §0, §1, §2, §5.1,
§6.2, §6.3, §7 (intro), §7.2, §7.3, §8.3, §10, §11.3, §11.4, §11.5, §11.7–11.14,
§12, §14, §15.1, §16, §17.1–17.3, §18, §19.1–19.3, §19.5, §19.7, §20, §22.1,
§22.2, §24, §25. Do not "improve" them.

**Amend briefly** — one to four sentences each, from the audit's findings,
citing the RFC:
| § | Change |
|---|---|
| 3 | Remove "Social login" and "External IdP federation" (RFC 004, 005 shipped). State today's out-of-scope from `ROADMAP.md` §Constraints and non-goals: multi-tenancy (RFC 025), alternative SQL backends beyond RFC 009 step 1, user-facing theming API. |
| 4 | Add: federation provider, user source (LDAP), client registration token, metrics token — one line each, wording from the RFC that introduced it. |
| 5.2 | Add OAuth 2.0 Dynamic Client Registration (RFC 7591) — RFC 008. |
| 5.3 | "OP" → "OP and, since RFC 004, relying party to upstream OIDC providers". |
| 6.1 | Add the four secrets: federation client secrets, LDAP bind credential, metrics bearer token, registration tokens. |
| 7.1 | Do **not** add threats here — that is `docs/threat-model.md`'s and RFC 097's. Add one sentence: the list predates RFCs 004/005 and the threat model's banner records the gap. |
| 11.1, 11.2, 11.6 | One sentence each naming the shipped subsystem and its page: federation and dynamic registration → `docs/src/reference/oidc-api.md`; LDAP user sources and the metrics endpoint → `docs/src/guides/operators.md`. |
| 13.2 | Delete "Consent retention is reserved for future expansion"; consent shipped (migration 0025). |

**Replace with a pointer** — delete the inventory, keep the section heading and
one or two sentences of principle if the section has any, then one line naming
the authoritative source:
| § | Pointer |
|---|---|
| 8.1 | Rust 2024 edition stays; toolchain floor and matrix → RFC 093 §Gate Matrix v1 and `rust-version` in `Cargo.toml`. |
| 8.2 | Delete the table → `Cargo.toml` `[workspace.dependencies]`. |
| 9, 9.1 | Delete the tree and the CLI list; keep the crate-responsibility sentences with the two paths corrected (`src/http/handlers/`, `src/runtime/dev_mode.rs`) → RFC 000 for `rfcs/`, `README.md` §Project layout, `sui-id --help`. |
| 13.1 | Delete the entity list → `crates/sui-id-store/src/migrations/`. |
| 15.2 | Delete the event list (and with it `auth.refresh.family_revoked`, which does not exist) → `ci/audit-coverage-matrix.md` and `docs/src/reference/audit-events.md`. |
| 17.4 | Keep the design-system principles; delete the JS-file table and the two named gates → `crates/sui-id/static/` and `ci/ui-invariants.toml` (G12). |
| 19.4 | Delete the list → the tree, RFC 000, §19.7. |
| 19.6 | Replace the body → RFC 098 §4 rule 6, one sentence stating it. |
| 21 | Delete the table and the "228/228" floor → RFC 093 §Gate Matrix v1, the lane tables in RFCs 094 and 098, `ci/gate-inputs.toml`. |
| 22.3 | Delete the `/mnt/user-data/outputs/` mechanism → `PUBLISHING.md` and RFC 093. |
| 23 | Delete the checklist → `ROADMAP.md` §Programme outcomes. |
| App. A | Delete → `CHANGELOG.md`. |
| App. B | Delete outright. |

**Evidence.** G15 on the tree: 7 (A), 0 (B), 0 (C) — (C) must stay zero with
the banner *removed*, which proves the re-pin is within tolerance. Every
pointer's target exists (G10b covers `docs`; run it). Line count before and
after. A section-by-section diff summary in the request, in the table's order,
so the review can check each disposition against this list. Nothing outside
the one file.

**Stop if** a *holds* section turns out to need a change, or a pointer's
target does not exist — both are findings, not edits.

### Dispatch 7 — 2026-09-12: step 6d, the specification's §7 under rule 7

Owner ruling: security and the threat model have a single source of truth in
documentation — RFC 098 §4 rule 7. `docs/development-specification.md` §7
opens by deferring to `docs/threat-model.md` and then restates it: §7.1 twelve
threats (already missing four shipped surfaces), §7.2 eight properties, §7.3
known limits. A summary is a copy. One commit, one file.

- **Keep** the §7 heading and its first paragraph, amended to: *"The threat
  model is `docs/threat-model.md`, and it is the only place threats, defensive
  properties and known limits are stated (RFC 098 §4 rule 7). This
  specification does not summarise it; a summary is a copy, and the one this
  section carried through v3 had drifted."*
- **Delete** §7.1, §7.2 and §7.3 in full, headings included.
- Nothing else changes. The 56 other kept sections stay byte-identical — prove
  it the way dispatch 6 did.

**Evidence.** G15: 7 (A), 0 (B), 0 (C) — (C) stays zero, the v4 claim is
untouched. G10b, G11, G14 green. Section-split byte-identity for everything
outside §7.

### Step 6a — landed 2026-09-12

Staleness banner on `docs/development-specification.md`, the rule-5 sanctioned
state. G15's (C) is now zero; (A) = 7, all step 5.

### Dispatch 5 — 2026-09-12: step 6b, the specification's drift audit

`docs/development-specification.md` is 1,229 lines written against v0.48.4. Do
**not** rewrite it. Produce the drift table first, as a review request: for
each numbered section, one row — *holds / contradicted / superseded / absent*
— naming the accepted RFC or the code path that decides it, and quoting the
contradicted sentence. RFC 098 rule 1: code wins over prose; rule 2: an
accepted RFC wins over any document except code. The §4 contradictions table
below is the seed; the audit must reach every section. No file changes in this
dispatch. The architect decides from the table whether the document is
reconciled section by section, or superseded by the authority table plus the
book and retired — that is a design decision the table informs.

### Dispatch 8 — 2026-09-12: step 5, retire the MI records; register G15

**Authorized by `@nabbisen`, 2026-09-12** — deletion of the 23 files included.
Open question 1 is ruled: no new folder, no gate change.

The first draft (move both MI packages into `rfcs/handoffs/RFC-MI-…/` and
extend invariant 13) is withdrawn on self-review — see RFC 098 §Open questions
1 and `.git-exclude/reviewed/oq1-oq3-self-review-2026-09-12.md`. If ruled as now
proposed, one commit:

1. `git rm -r docs/src/mockup-integration docs/mockup-integration` — 23 files.
   Record the pre-deletion commit SHA; it goes in every note below.
2. Dated notes, the RFC 085 form, in `rfcs/done/RFC-MI-000-baseline-delta-inventory.md`
   and `rfcs/done/RFC-MI-080-ui-regression-a11y-hardening.md`: the companion
   artifacts this RFC describes were retired to history on 2026-09-12 under
   RFC 098 step 5; recover with `git show <sha>:<path>`. The RFCs' own text is
   not rewritten.
3. The five link-form references become named paths, each with the same
   one-line note: `ROADMAP.md` lines 507 and 554, `CHANGELOG.md` line 3504 (a
   link target in a historical file — rule 4 permits it), `rfcs/README.md`
   line 106, RFC-MI-000 line 19. G11 link-checks every RFC file including the
   MI ones, so RFC-MI-000's must not be left as a dead link.
4. **No change** to `scripts/check-rfc-integrity.py`, `ci/rfc-policy.toml`, or
   `SUMMARY.md`.
5. Then, in the same commit: G15 moves from `[gate_matrix_exceptions]` to
   `[gates]`, with a `G15` job shaped like G14's. A3.4 must pass — check 3
   requires the lane in exactly one of the two tables.

**Evidence.** `python3.14 scripts/check-doc-authority.py --root . --policy ci/doc-authority.toml`
→ `all conditions satisfied`, exit 0 — the first green run in the lane's
life. G10a (mdBook must not miss the deleted pages: they were never in
`SUMMARY.md`). G10b, G14, G11 green. `bash scripts/ci-gate.sh G15` on a clean
tree, exit 0.

Independent of dispatch 7 (§7 of the specification); either order.

## 1 — Publish the authority map first

Before changing any document, publish a map naming, per topic, **one**
authoritative document and the rule that resolves a conflict. Suggested topics:
product scope and claims; security posture; operations; integration; API
reference; lifecycle and governance; release history.

Everything downstream depends on this map, so it is reviewed on its own before
reconciliation begins.

## 2 — Separate normative from historical

Current normative documentation describes what the system is now. Historical RFC
rationale and changelog records describe what was decided when.

**Never rewrite a historical decision to pretend it always matched current
state.** Two dated review-evidence files intentionally reference
`accepted/09{4,6}-*` even though those RFCs are now in `proposed/` — that is
correct and must stay. The same principle governs every RFC in `done/`.

## 3 — Reconcile the public surface

Work through each against implemented behaviour:

- `README.md` — scope claims, feature list, workspace description, MSRV and the
  `rustup` expectation added in M1a, quick start, links;
- `ROADMAP.md` — programme state, milestone structure, what is frozen;
- `docs/` and the mdBook sources — resolve the divergent duplicate operator and
  integrator sets into one authoritative set;
- `docs/src/contributing/architecture.md` — current module layout and the actual
  database access pattern;
- package metadata and source-path references;
- `PUBLISHING.md` placement, per RFC 024's unfinished consolidation.

## 4 — Known contradictions to close

| Location | Contradiction |
|---|---|
| `README.md:82-84` | Says LDAP is not offered; LDAP, federation and dynamic registration are shipped |
| `README.md:181-189` | Omits the i18n crate |
| `docs/src/contributing/architecture.md:23-38` | Moved paths; pre-`Backend` `Arc<Mutex<Connection>>` design |
| `docs/src/contributing/architecture.md:75-79` | Claims every mutation uses `events::emit` — false |
| Root vs mdBook operator/integrator docs | Divergent duplicate sets |
| RFC 024 | Promised consolidation visibly incomplete |

Re-verify each before editing; several may have been overtaken by M1a–M4
implementation work, and a stale finding is as bad as a stale claim.

## 5 — Claims that need particular care

- **Anything describing the project as security-reviewed or production-ready.**
  Nothing before M7 carries that designation, and M7's outcome is a readiness
  *discussion*.
- **The audit chain.** It is tamper-evident within its trust boundary and is not
  evidence against a malicious database writer. Wording that implies otherwise
  is a security-claim defect, not a style issue.
- **Federation, LDAP and dynamic registration.** State what is actually
  supported after M2–M4, not what the original RFCs proposed.
- **MSRV.** 1.95, with the `rustup` expectation — most distribution-packaged
  toolchains are older.

## 6 — Exit

Authoritative documents, README, roadmap, development specification,
operator/integrator guidance, public claims, source paths and lifecycle metadata
all agree; mdBook and the integrity gates pass; and every remaining
known-inaccurate statement has been either corrected or explicitly recorded as a
limitation with an owner.
