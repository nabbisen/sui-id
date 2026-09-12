# RFC 098 — Documentation Authority and Reconciliation

**Status.** Accepted
**Accepted on.** 2026-09-10
**Approved by.** `@nabbisen`
**Independent design review.** `@nabbisen`, [Design review 2026-09-10](../handoffs/098-documentation-authority/098-design-review-2026-09-10.md)
— design authored by the architect and reviewed by the accountable owner, who
authored neither this RFC nor its design and is not the implementer.
**Security review.** Required
**Amendment summary (2026-09-12).** Added §Gate Matrix lanes owned by RFC 098,
registering this RFC's enforcement through RFC 094's multi-source lane registry
(R10, `153db49`): **G14** now, **G15** declared and held under
`[gate_matrix_exceptions]` until the reconciliation it enforces has landed. This
answers open question 2 and supersedes §7's closed-RFC framing; RFC 093 is not
amended. No scope, prerequisite or acceptance criterion changes.
**Design prerequisites.** RFC 093 Accepted for mechanical integrity ownership; authoritative-document hierarchy approved before RFC 097 final drafting.
**Implementation prerequisites.** RFC 093 **M1b** Implemented — the mdBook, markdown-link, and RFC-integrity gates M1b owns are the mechanical foundation this RFC builds on, and reconciling claims before those gates exist would leave the result unenforced; this RFC Accepted. M1a is implied by M1b but is not independently sufficient.
**Closure prerequisites.** Authoritative documents, README, roadmap, development specification, operator/integrator guidance, public claims, source paths, and lifecycle metadata agree; mdBook and integrity gates pass.
**Tracks.** ROADMAP M5 — Documentation authority and reconciliation.
**Handoff.** [`../handoffs/098-documentation-authority/README.md`](../handoffs/098-documentation-authority/README.md)
**Touches.** `README.md`, `ROADMAP.md`, `docs/`, RFC links/metadata not mechanically closed by RFC 093, public package metadata and source-path references.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Implementation owner.** `codex-developer` (OpenAI Codex), after acceptance and prerequisites.
**Independent security and closure reviewer.** Role independence per RFC 000 —
the reviewer must not have authored, implemented, or previously approved this
RFC, and the implementer cannot be the sole approver; vendor is not a
criterion. *Corrected at acceptance, 2026-09-10: this clause previously routed
independent review to the implementation role, which is how RFCs 094–096 were
reviewed. `@nabbisen` ruled on 2026-09-09 that the implementation role is not
a reviewer. Those earlier reviews stand as performed and are not reopened.*

## Summary

Define which tracked documents are authoritative for behavior, security,
operations, integration, lifecycle, and release claims, then reconcile every
current public surface against implemented code and accepted governance.

## Requirements

- Publish a document-authority map with one owner and conflict rule per topic.
- Separate normative current documentation from historical RFC rationale and
  changelog records; never rewrite a historical decision to pretend it always
  matched current behavior.
- Reconcile production/security/readiness claims, feature lists, configuration,
  operator and integrator procedures, source paths, lifecycle state, and links.
- RFC 093 retains ownership of narrow folder/status/index/link enforcement.
  This RFC owns semantic authority, broad current-path cleanup, and public-claim
  truthfulness.
- Every change is traceable to code, an accepted design, or observed evidence.
  Unknown behavior is tested or documented as uncertainty, not guessed.
- mdBook, RFC integrity, and release-document checks are blocking at closure.

## Design

*Drafted 2026-09-10 by the architect, measured against the tree at `ee48257`.
Every count below was checked, not estimated. Proposed — not accepted.*

### 1. Document domains

A document belongs to exactly one domain, decided by **the question it
answers** and **what makes it true** — not by who wrote it or where it
happens to sit today. Six domains cover every tracked document.

| # | Domain | Question it answers | What makes it true | Home |
|---|---|---|---|---|
| **D1** | Product documentation | How do I run, integrate with, or contribute to the shipped product? | Behaviour of released code | `docs/src/`, every page reachable from `SUMMARY.md` |
| **D2** | Engineering specification | What contracts must the code satisfy? | Accepted RFCs and code | `docs/` top level |
| **D3** | Machine-consumed contracts | What does a gate compare against? | The gate that reads it | `ci/` |
| **D4** | Decision records | What was decided, and why? | RFC 000's lifecycle | `rfcs/{proposed,accepted,done,archive}/` |
| **D5** | Decision companions | How is a decided thing implemented and verified? | Its RFC; status inherited | `rfcs/handoffs/NNN-slug/` |
| **D6** | Roadmap work packages | How is work that no RFC governs carried out? | A `ROADMAP.md` entry | `roadmap/<slug>/` |

D4, D5 and D6 are settled and enforced: RFC 000 governs D4, and G11's
invariants 12 and 13 enforce D5's boundary and D6's separation as of
2026-09-10. **This RFC owns D1, D2 and D3**, which have no boundary today.

### 2. Authority table

One authoritative document per topic. Where a second document covers the
same topic it is either deleted, or reduced to a pointer — never
hand-synchronised.

| Topic | Authoritative | Conflict rule |
|---|---|---|
| Installing and first run | `docs/src/guides/deployment.md` | Code wins over prose; the guide is corrected, never the code silently |
| Day-to-day operation, CLI, admin | `docs/src/guides/operators.md` | The binary's `--help` and the CLI source win; the guide follows |
| Destructive operations | `docs/src/guides/dangerous-operations.md` | Step-up and confirmation behaviour is RFC 058's; the guide describes, never defines |
| Upgrading between versions | `docs/src/guides/upgrade.md` | Migration files win |
| Relying-party integration | `docs/src/reference/oidc-api.md` | Route table in `crates/sui-id/src/http/` wins |
| Configuration keys | `docs/src/reference/configuration.md` | `crates/sui-id/src/runtime/config.rs` wins |
| Audit event vocabulary | `docs/src/reference/audit-events.md` | The event literals in `crates/sui-id-store` win; enforced by G-audit-matrix |
| Audit coverage matrix | `ci/audit-coverage-matrix.md` (**D3**, moves from `docs/src/reference/`) | It *is* the gate input; source literals win, and the gate says so |
| Threat model | **RFC 097** once Implemented; `docs/threat-model.md` until then, marked stale | A shipped trust boundary absent from the document is a defect in the document |
| Security assurance history | `docs/security-assurance-audit-v0.63.1.md` | Historical; never updated, only superseded |
| Development specification | `docs/development-specification.md` | Accepted RFCs win over it; it is a synthesis, not a source |
| UI/UX cross-cutting contract | `docs/ui-ux-contracts.md` | RFC 017 and the component code win |
| Release history | `CHANGELOG.md`, archived at `docs/changelog/` | Append-only; never rewritten |
| Direction and non-RFC authorization | `ROADMAP.md` | Not an approval record for design; RFCs are |
| Project overview and public claims | `README.md` | Every claim traceable to shipped code |
| Publishing procedure | `PUBLISHING.md` | The release workflow files win |

### 3. What the measurement found

Four defects, all live at `ee48257`.

**F1 — The threat model is 51 releases stale, and README presents it
publicly.** `docs/threat-model.md` states "It is current as of **v0.26.0**";
the workspace is at **v0.77.0**. Security-relevant capability shipped since
is absent from the document `README.md` links to as "what sui-id defends
against": upstream OIDC federation (RFC 004, v0.76.4) and dynamic client
registration (RFC 008, v0.76.3) with zero mentions in the body, the
Prometheus metrics endpoint (RFC 006, v0.76.0) — an auth-gated network
surface with its own bearer token — with zero mentions, and LDAP user sources
(RFC 005, v0.76.1) named only in the out-of-scope list. This is a **security
claim**, not a documentation gap, and it is the highest-severity item in
this RFC.

*Corrected 2026-09-12 at step 1's review: the first version of this list
also named step-up and master-key rotation as absent. Both are analysed in
the document — 13 and 4 mentions respectively — because both predate
v0.26.0. The list was asserted from the feature history rather than grepped
against the body; the implementer grepped, and the correction is the
architect's.*

**F2 — Three user-facing guides exist twice, and the public front page
links to the stale copy of each.**

| Root copy | Book copy | Unique to root | Unique to book |
|---|---|---|---|
| `docs/deployment.md` (2026-04-30) | `docs/src/guides/deployment.md` (2026-08-01) | 2 lines | 18 lines |
| `docs/operators.md` | `docs/src/guides/operators.md` | 6 lines | 106 lines |
| `docs/integrators.md` (2026-04-30) | `docs/src/reference/oidc-api.md` (2026-08-01) | 7 lines | 65 lines |

The book copies are near-supersets carrying whole sections the root copies
lack — Roles, headless-CLI setup, LDAP/Active Directory, federated sign-in,
Prometheus metrics, dynamic client registration. **Every line unique to a
root copy is a cross-link to another root copy**, except one: `docs/integrators.md`
still lists "Dynamic client registration" under *What sui-id does not do
(yet)*, which shipped in v0.76.3. The root copies hold no content worth
keeping and one false claim.

`README.md` links to all three, and to the threat model, as absolute
`https://github.com/nabbisen/sui-id/blob/main/…` URLs. Absolute URLs to the
project's own files are invisible to G10b, so the link checker cannot see
that the front page points at superseded documents.

`docs/operators.md` is also cited from `crates/sui-id/src/cli.rs` and
`http/handlers/setup.rs` — the running binary directs operators to a copy
that is 106 lines behind. Both copies were last touched in the same commit
(`cc68b70`), so they are being hand-synchronised and still diverging: the
worst state, because it looks maintained.

**F3 — Eight pages under `docs/src/` are unreachable.** Not listed in
`SUMMARY.md`, so mdBook never builds them and no reader can reach them: the
seven `mockup-integration/` verification matrices and
`reference/audit-coverage-matrix.md`. G10a passes because mdBook builds only
what `SUMMARY.md` names — an orphan page is invisible to the gate, not an
error.

**F4 — A gate input lives in the published-book source tree.**
`docs/src/reference/audit-coverage-matrix.md` is read by
`scripts/check-audit-matrix.sh` as the 54-entry contract. It is D3 material
sitting in D1's directory, and it is one of F3's orphans — it is neither a
published page nor filed as a gate input.

`docs/development-specification.md` carries a subtler version of F1: it
describes itself as "v3 — reflecting the v0.48.4 codebase" at v0.77.0. It is
a synthesis rather than a claim surface, so it is corrected rather than
treated as a defect of the same order.

### 4. Conflict-resolution rules

1. **Code wins over prose.** Where a document and the implementation
   disagree, the document is wrong until an RFC says otherwise. Fix the
   document, or file the divergence as a defect — never adjust prose to make
   a bug read as intended.
2. **An accepted RFC wins over any document except code.** A document may
   describe a decision; it may not make one.
3. **One topic, one document.** A second document on a topic is deleted or
   reduced to a link. Manual synchronisation is prohibited outright: F2 is
   what it produces. If two rendered copies are genuinely needed, one is
   generated from the other by a gate that regenerates and diffs — this
   answers the RFC's original open question, and "generated" is only
   credible with that gate.
4. **Historical documents are never rewritten.** A dated record keeps its
   text; a superseding document is added and the old one marked superseded.
   This is RFC 000's rule for RFCs, applied to documents.
   *Clarified 2026-09-12.* **Link paths follow moved files.** Rewriting a
   link's target in a dated record is not rewriting the record: its text, its
   claims and its findings do not change. The `rfcs/reviews/` migration and
   the RFC 018 retirement both did this; G14 caught the first case this rule
   was needed for on the day it existed. A self-referential absolute URL in a
   historical file is a wrongly written path and is fixed the same way.
5. **A stale document says so.** Any document whose accuracy is pinned to a
   version carries that version in its first paragraph; where it lags the
   workspace, it carries an explicit staleness banner until reconciled. An
   undated wrong document is worse than a dated one.
6. **A book page links outside the book by absolute repository URL.**
   *Added 2026-09-12.* A page under `docs/src/` is rendered by mdBook and by
   GitHub; a link from it to a repository file outside `docs/src/` has no
   relative form that works in both — mdBook rewrites `.md` to `.html` and
   emits `../../../ROADMAP.html`, which the build never produces and which
   escapes `site-url`. From the book's standpoint the repository is an
   external site, and `book.toml` already hardcodes it for edit links. So:
   book page → outside-book file uses the absolute repository URL; every
   other tracked document links repository-relative. Check (B) enforces both
   halves, and for the sanctioned absolute form strips the prefix and
   requires the path to exist on disk, so the form is no longer gate-blind.
   Where an outside-book file genuinely belongs in the product
   documentation, `{{#include}}` it as a page instead — one source, no copy.

### 5. File-by-file reconciliation plan

Ordered by severity. Each step is separately reviewable and reversible.

| # | Action | Files | Why this order |
|---|---|---|---|
| 1 | Add a staleness banner naming v0.26.0 and the shipped-since feature list | `docs/threat-model.md` | F1 is a public security claim; the banner is minutes of work and stops the harm immediately, without waiting for RFC 097 |
| 2 | Delete the three stale forks; repoint `README.md` at the book pages with repo-relative links *(the book-page self-URL in `overview.md` is correct under rule 6 and stays)* | `docs/deployment.md`, `docs/operators.md`, `docs/integrators.md`, `README.md` | Removes the false dynamic-registration claim and puts the front page on maintained pages; repo-relative makes them visible to G10b |
| 3 | Repoint the two source-code citations | `crates/sui-id/src/cli.rs`, `crates/sui-id/src/http/handlers/setup.rs` | The binary must not direct operators to a deleted path |
| 4 | Move the gate input to `ci/audit-coverage-matrix.md`; update `scripts/check-audit-matrix.sh` and RFC 093's reference | `docs/src/reference/audit-coverage-matrix.md` | Resolves F4 and removes one of F3's orphans |
| 5 | Decide each remaining orphan: publish it in `SUMMARY.md`, or move it out of `docs/src/` | seven `docs/src/mockup-integration/` matrices | They are RFC-MI-080 verification records of a completed arc — D4/D5 material, not D1 |
| 6 | Correct the version claim and reconcile against accepted RFCs — *decided 2026-09-12 from the drift audit: **cut, do not reconcile.** 25 of 40 sections hold and every one of them states principle or policy; every section that drifted is an inventory — a second copy of something the repository holds authoritatively — which rule 3 says should not exist. Principles stay; each inventory becomes a pointer to its source; re-pinned as v4 at v0.77.0. **Landed 2026-09-12.*** | `docs/development-specification.md` | Synthesis document; lower blast radius |
| 7 | Decide the disposition of the 16-file MI planning package | `docs/mockup-integration/` | Records of a completed arc; see §6 |

Steps 1–3 close every public-facing falsehood. Steps 4–7 are structural.

### 6. Historical-document treatment

Three classes, and none of them is deleted:

- **Dated audits and reviews** (`docs/security-assurance-audit-v0.63.1.md`)
  keep their text permanently. The version in the filename is the contract.
- **Completed-arc packages** (`docs/mockup-integration/`, the
  `docs/src/mockup-integration/` matrices) are records of RFC-MI work that
  shipped. They belong with their decisions, not in the product
  documentation tree. Their destination is a D4/D5 question this RFC raises
  but does not settle, because the `RFC-MI-NNN` identifiers do not fit the
  `NNN-slug` handoff shape that invariant 13 enforces.
- **Changelog** (`CHANGELOG.md`, `docs/changelog/`) is append-only. Nothing
  in this RFC edits a shipped entry, even where a past entry describes
  behaviour later corrected.

### 7. Mechanical enforcement

Reconciliation without a gate drifts back; that is the lesson of
`rfcs/reviews/` and of the six unauthorised work packages. Three checks,
each closing one finding permanently:

1. **`SUMMARY.md` completeness** — every `.md` under `docs/src/` is
   reachable from `SUMMARY.md`, and every `SUMMARY.md` entry exists. Closes
   F3, which G10a structurally cannot see.
2. **No self-referential absolute URLs** — a link to
   `https://github.com/nabbisen/sui-id/blob/…` in a tracked document is a
   repo-relative link written wrongly, and it evades G10b. Closes half of
   F2's invisibility.
3. **Version-claim freshness** — a document declaring "current as of vX.Y.Z"
   is compared against the workspace version, and a lag beyond a declared
   tolerance fails. Closes F1's recurrence.

Checks 1 and 2 are link-shaped and belong with G10b; check 3 is a
claim-truthfulness check and is this RFC's own. **Widening G10b's command
requires amending RFC 093's lane table, which is closed** — the same
constraint as R10. That decision is `@nabbisen`'s and is recorded as an open
question below rather than assumed here.

*Superseded 2026-09-12.* The paragraph above was written before R10 landed. RFC
093's lane set is closed by its own text and RFC 093 is in `done/`; amending it
was never the mechanism. Under R10 this RFC owns its own lanes, validated by
A3.4 against the table in §Gate Matrix lanes owned by RFC 098 below. Checks 1
and 2 stay link-*shaped* but are this RFC's — the scope extension beyond what
G10b covers is "broad current-path cleanup", which §Requirements assigns here.

G10b's scope is also still `README.md ROADMAP.md docs` — it does not cover
`rfcs/` or the new `roadmap/`. Thirty-one broken links passed unseen during
the 2026-09-10 `rfcs/reviews/` migration for exactly this reason.

### Gate Matrix lanes owned by RFC 098

Registered through the multi-source lane registry (RFC 094 R10). The heading
above is the recorded source heading and is matched by plain equality; do not
rename it without changing the manifest in the same commit. Column layout
mirrors RFC 093's table so one parser reads both.

| ID | Toolchain | Features | Blocking command / assertion |
|---|---|---|---|
| G14 | Python 3.14 | n/a | `python3.14 scripts/check-markdown-links.py --root . rfcs/handoffs roadmap` |
| G15 | Python 3.14 | n/a | `python3.14 scripts/check-doc-authority.py --root . --policy ci/doc-authority.toml` |

**G14 — the links no gate checked.** G11 link-checks every RFC file and
`rfcs/README.md`; G10b checks `README.md`, `ROADMAP.md` and `docs/`. Neither
covers `rfcs/handoffs/` or `roadmap/` — which is how thirty-one broken links
passed unseen during the `rfcs/reviews/` migration. G14 runs the existing,
proven link checker over exactly that complement. Zero new code; green on the
tree at `a33fd7e`; registers immediately.

**G15 — the three checks of §7**, in one script so one failure names one cause:
(A) `SUMMARY.md` completeness in both directions; (B) no link to
`https://github.com/nabbisen/sui-id/blob/…` in a tracked document; (C) every
document declaring a version it is current as of is within the tolerance
`ci/doc-authority.toml` sets, **or carries a staleness banner** — rule 5 makes
the banner the sanctioned state for a lagging document, so a bannered document
passes and an unbannered stale one fails. `ci/doc-authority.toml` is D3: it holds
the tolerance and the list of version-pinned documents, and nothing else.

**G15 is declared now and registered later.** On the tree at `a33fd7e` it would
fail on exactly the findings this RFC records — eight orphan pages (F3), three
absolute self-URLs (F2), and `docs/development-specification.md`'s unbannered
v0.48.4 claim — and a lane that is red on `main` is not a gate. RFC 093 §Gate
entry points: *"If a new lane is initially unreliable, the change remains
Proposed/Accepted until the contract is restored."* So G15 sits in
`[gate_matrix_exceptions]` with that reason, which the registry's check 6 keeps
grounded in this table, and moves to `[gates]` in the commit that lands step 6.
The script and its fixtures are built now; proving it red on today's tree is
itself the evidence that F1–F3 are real.

**Negative self-tests.** G14 inherits `check-markdown-links.py`'s existing
Python tests. G15 carries one fixture per check and per direction — an orphan
page, a `SUMMARY.md` entry with no file, an absolute self-URL, a stale claim
with no banner — and one positive fixture proving a stale claim *with* a banner
passes, so check C cannot pass by forbidding every pinned document.

## Open questions

1. **Where do completed-arc records live?** The MI packages are companions
   to `RFC-MI-NNN` decisions, but that identifier does not fit the
   `NNN-slug` handoff shape invariant 13 enforces. Either the handoff rule
   accommodates the MI namespace, or these become a fourth disposition.
   §6 raises it; this RFC does not settle it.
   *Proposed resolution, 2026-09-12 — awaiting `@nabbisen`.* Measured: all
   sixteen `RFC-MI-NNN` RFCs are in `done/`; the seven `docs/src/mockup-integration/`
   matrices state they are RFC-MI-080's verification records; the sixteen
   `docs/mockup-integration/` files are the Phase 0 planning package RFC-MI-000
   links to eleven times; five link-form references point at them from outside.
   **Extend invariant 13 to the MI namespace** — a handoff directory named
   `RFC-MI-NNN-slug/` resolves to exactly one `rfcs/*/RFC-MI-NNN-*.md`, mirroring
   the `MI_RE` G11 already carries — and file each package as its RFC's companion:
   the matrices under `rfcs/handoffs/RFC-MI-080-ui-regression-a11y-hardening/`,
   the planning package under `rfcs/handoffs/RFC-MI-000-baseline-delta-inventory/`.
   Same disposition the RFC 093/094 review records received on 2026-09-10 — dated
   records live with their decision and inherit its status. No fourth
   disposition, no policy change, no edit to any MI RFC's text; link paths
   follow the files (rule 4). Closes F3 in full; G15 then registers.
2. ~~**How do the three enforcement checks reach CI?**~~ *Answered 2026-09-12:*
   through RFC 094's multi-source lane registry, as this RFC's own lanes G14
   and G15 (§Gate Matrix lanes owned by RFC 098). Both framings this question
   offered — amend RFC 093, or host under G11 — were wrong for the reason
   recorded in §7.
3. **Does `docs/threat-model.md` survive RFC 097?** If RFC 097 becomes the
   threat model, the file is superseded rather than reconciled, and step 1
   of §5 is the only work it ever receives again.
