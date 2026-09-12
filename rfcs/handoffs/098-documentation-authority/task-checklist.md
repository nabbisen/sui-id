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
