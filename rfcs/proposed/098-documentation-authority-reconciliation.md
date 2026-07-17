# RFC 098 — Documentation Authority and Reconciliation

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFC 093 Accepted for mechanical integrity ownership; authoritative-document hierarchy approved before RFC 097 final drafting.
**Implementation prerequisites.** RFC 093 Implemented; this RFC Accepted.
**Closure prerequisites.** Authoritative documents, README, roadmap, development specification, operator/integrator guidance, public claims, source paths, and lifecycle metadata agree; mdBook and integrity gates pass.
**Tracks.** ROADMAP M5 — Documentation authority and reconciliation.
**Touches.** `README.md`, `ROADMAP.md`, `docs/`, RFC links/metadata not mechanically closed by RFC 093, public package metadata and source-path references.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Implementation owner.** `codex-developer` (OpenAI Codex), after acceptance and prerequisites.
**Independent security and closure reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex).

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

## Planned design work

Before acceptance, provide the exact authority table, content inventory,
conflict-resolution rules, claim/evidence matrix, file-by-file reconciliation
plan, historical-document treatment, and optional mechanical task checklist.
The authority decision must land early enough for RFC 097 to cite it.

## Open questions

Whether duplicated root/docs pages are generated or manually synchronized is
deferred to the authority-table design; silent duplication is not acceptable.
