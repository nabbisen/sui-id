# RFC 094 developer handoff

**Governing RFC:** [RFC 094](../../accepted/094-transactional-audit-registry.md)
**Audience:** `codex-developer` after RFC acceptance and prerequisite evidence
**Status:** Planning companion; inherits the governing RFC's Accepted status, with implementation still blocked on the entry gate below

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
- RFC 093 is Implemented and its current clean-tree gate matrix passes;
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
- master-key recovery cannot deterministically reach OldReady or Complete from
  a newly discovered crash point;
- failure injection cannot prove rollback for an inventory row;
- a change materially alters RFC security invariants or prerequisites.
