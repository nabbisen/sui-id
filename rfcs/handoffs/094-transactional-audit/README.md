# RFC 094 developer handoff

**Governing RFC:** [RFC 094](../../accepted/094-transactional-audit-registry.md)
**Audience:** `codex-developer` after RFC re-acceptance and prerequisite evidence
**Status:** Planning companion; inherits the governing RFC's current status — **Accepted 2026-08-27**, the 2026-07-28 return for review having closed after independent design review by the implementation role. Implementation remains gated on the entry gate below.

This handoff translates RFC 094 into bounded implementation stages. It does
not approve design, authorize coding, or override the RFC. If implementation
reveals a conflict with an RFC invariant, stop and amend/move the RFC before
changing this handoff.

## Files

- [architecture.md](architecture.md) — target components, ownership boundaries,
  and transaction flow.
- [command-inventory.md](command-inventory.md) — exact Stage-0 classification
  of current production durable-write commands and surfaces.
- [migration-checklist.md](migration-checklist.md) — ordered conversion waves
  and per-command checklist.
- [verification.md](verification.md) — required negative, rollback,
  concurrency, structural, and closure evidence.

## Entry gate

Implementation may begin only when all are true:

- RFC 094 is under `rfcs/accepted/` with complete approval metadata;
- RFC 093 is **Implemented** (closed 2026-08-27, `9610abb`), which satisfies both
  this gate and RFC 094's own `Implementation prerequisites` field. *Resolved
  2026-08-28. Between 2026-08-27 and RFC 093's closure this line and the RFC
  disagreed — the handoff required "093 Implemented" while the RFC, amended
  2026-07-28, said "M1a complete … M1b is not a prerequisite". The discrepancy was
  recorded rather than relaxed, on the grounds that loosening a gate on
  security-critical implementation is the owner's call; closing RFC 093 satisfied
  both readings and made the question moot.*
- the exact durable-write command inventory, generated write-site
  reconciliation, and threat delta have durable independent approval;
- `@nabbisen` confirms `codex-developer` as implementation owner;
- no competing change owns the same transaction/audit files.

## Stop conditions

Stop and return to architecture/security review if:

- a durable security mutation cannot fit the one-transaction contract;
- a production write cannot fit one of the sealed A/P/O/I/X capabilities;
- a filesystem/network side effect is required inside the SQLite transaction;
- a Class-A command is missing from the approved inventory;
- a secret-bearing value appears necessary in an audit payload;
- a legacy public API must remain capable of bypassing the transaction runner;
- failure injection cannot prove rollback for an inventory row;
- a change materially alters RFC security invariants or prerequisites.

## Accepted RFC 096 inventory amendment

C17/C18 activation handling, C23, and F01–F06 in `command-inventory.md` are an
accepted amendment made after the original RFC 094 acceptance. The independent
review and `@nabbisen` approval are recorded in
`rfcs/reviews/094-federation-command-amendment-review-2026-07-21.md`. The
owner decision is preserved in the durable review record. RFC 000 returned the
complete material RFC to Proposed in commit
`43085e38219e5eb1bfe11cc698b18f1fa5f5e4d7`, and `@nabbisen` explicitly
accepted the complete amended RFC on 2026-07-21. The generated manifest,
registry, migration checklist, verification harness, and documentation must
include the amendment; the 2026-07-17 base review alone cannot authorize it.

An already-disabled C17 disable is a non-committing no-op and does not cancel
an in-progress preflight/enable operation. An operator needing cancellation
must cancel that operation or use an explicit generation-advancing command;
the UI/docs must not describe repeated disable as revocation.

RFC 096 implementation must stop if it would persist preflight outside C17,
omit the checked activation-generation guard/increment from C17/C18/C23,
overload C17 for trust-policy replacement, compose U01/C19/U30 as nested Class-A calls, split one F01–F04
compound mutation across transactions, or treat a Class-B login observation as
atomically audited. Cache eviction for C23 is post-commit and never part of the
SQLite transaction.
