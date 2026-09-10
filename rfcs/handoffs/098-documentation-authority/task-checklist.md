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
