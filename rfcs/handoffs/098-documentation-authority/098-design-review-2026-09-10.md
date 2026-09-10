# RFC 098 — design review and acceptance record

**Date.** 2026-09-10
**RFC.** [RFC 098 — Documentation Authority and Reconciliation](../../accepted/098-documentation-authority-reconciliation.md)
**Design author.** High-capability model (architect), drafted `aae626c`
**Independent design reviewer.** `@nabbisen`
**Outcome.** **Accepted** on 2026-09-10, and authorized for the record on the
same day.

## Why this record exists

RFC 000 requires a security-sensitive RFC to carry a **named** independent
design reviewer and a repository-relative reference to the review, and states
that the implementer cannot be the sole approver. This document is that
reference. It records what was put up for review, what the reviewer decided,
and what was deliberately left open — it does not restate the design, which
is in the RFC.

## Role independence

| Role | Who |
|---|---|
| RFC author (original, 2026-07) | `codex-project-architect` (OpenAI Codex) |
| Design author (this design, 2026-09-10) | High-capability model (architect) |
| Independent design reviewer | `@nabbisen` |
| Accountable owner and approver | `@nabbisen` |
| Implementation owner | Mid-capability model, after prerequisites |

The reviewer authored neither the RFC nor the design under review and is not
the implementer. RFC 000's constraint — *"the implementer cannot be the sole
approver of a security-sensitive design"* — is satisfied: the design author
and the approver are different parties, and the implementer is a third.

**A correction was made to the RFC's own reviewer clause at acceptance.**
As written in 2026-07 it routed independent design review to the
implementation role, which is how RFCs 094, 095 and 096 were reviewed. On
2026-09-09 `@nabbisen` ruled that the implementation role is not a reviewer.
The clause was stale against that ruling and is corrected in the accepted
text; the earlier reviews of RFCs 094–096 stand as performed and are not
reopened.

## What was reviewed

The design section added in `aae626c`, measured against the tree at `ee48257`:

- the six-domain model, of which this RFC owns D1 (product documentation),
  D2 (engineering specification) and D3 (machine-consumed contracts);
- the authority table — sixteen topics, each with one authoritative document
  and a conflict rule;
- four findings, every count measured rather than estimated;
- five conflict-resolution rules;
- a seven-step file-by-file reconciliation plan ordered by severity;
- historical-document treatment in three classes;
- three mechanical checks to stop the reconciliation drifting back.

## Findings carried into acceptance

The design is accepted with these recorded as its work, not as objections:

| ID | Finding | Severity |
|---|---|---|
| **F1** | `docs/threat-model.md` declares itself current as of **v0.26.0**; the workspace is at **v0.77.0**. Everything shipped since — LDAP user sources, upstream OIDC federation, dynamic client registration, the metrics endpoint, step-up, master-key rotation — is absent from the document `README.md` presents publicly as what sui-id defends against. A public **security claim**, not a documentation gap. | Highest |
| **F2** | Three user-facing guides exist twice; the book copies are near-supersets (18, 106, 65 unique lines) and `README.md` links to the stale copy of each. `docs/integrators.md` still lists dynamic client registration under *what sui-id does not do (yet)*, which shipped in v0.76.3. `crates/sui-id/src/cli.rs` and `http/handlers/setup.rs` cite a copy 106 lines behind, so the binary directs operators to it. Both operator copies were last touched in one commit — hand-synchronised and still diverging. | High |
| **F3** | Eight pages under `docs/src/` are absent from `SUMMARY.md`; mdBook never builds them, and G10a cannot see an orphan page. | Medium |
| **F4** | `docs/src/reference/audit-coverage-matrix.md` is the gate input `scripts/check-audit-matrix.sh` reads — a machine-consumed contract filed in the published-book tree, and one of F3's orphans. | Medium |

## Resolution of the RFC's original open question

> Whether duplicated root/docs pages are generated or manually synchronized is
> deferred to the authority-table design; silent duplication is not acceptable.

**Resolved.** Manual synchronisation is prohibited outright — F2 is what it
produces, and it is worse than no synchronisation because it looks maintained.
One topic has one document. Where two rendered copies are genuinely needed,
one is generated from the other by a gate that regenerates and diffs;
"generated" is not credible without that gate.

## Left open at acceptance

Three questions are recorded in the RFC rather than settled, and acceptance
does not decide them:

1. Where completed-arc `RFC-MI-NNN` records live — that identifier does not
   fit the `NNN-slug` handoff shape invariant 13 enforces.
2. How the three mechanical checks reach CI. Widening G10b's scope means
   amending RFC 093's closed lane table; the alternative is hosting them
   under G11 as invariants 12 and 13 were. Owner's call.
3. Whether `docs/threat-model.md` survives RFC 097 at all, or is superseded
   by it.

Implementation may not treat any of the three as decided.

## Implementation eligibility

RFC 098's implementation prerequisites are RFC 093 **M1b** Implemented —
satisfied when RFC 093 closed on 2026-08-27 — and this RFC Accepted, which
this record completes. Step 1 of the reconciliation plan (the F1 staleness
banner) is the first work item and is pure addition.

---

`rfcs/handoffs/098-documentation-authority/098-design-review-2026-09-10.md`
