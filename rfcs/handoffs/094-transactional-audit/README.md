# RFC 094 developer handoff

> **`0.85.0` is authorized** (2026-10-24 scope,
> [`cycle-g-and-0.85.0-2026-10-25.md`](cycle-g-and-0.85.0-2026-10-25.md);
> authorized 2026-10-25): **wave 4 complete**, C01–C10, C21, C22, K02,
> K03. Three stages —
> [`m2b-0.85.0-stage-plan-2026-10-25.md`](m2b-0.85.0-stage-plan-2026-10-25.md).
> **Stage 1 (C05, C21, C06) landed** `05e0050` (Level B, 27 jobs, 0
> skipped); G20's declared set is at 37, moving to 42 → 48 across the two
> remaining stages. Its dispatch stays **held in `.git-exclude/staging/`**
> until `0.85.0` is released, not merely until the stage landed: it states
> the consequence of a weakness that is fixed in the tree but not yet in
> any published version, and `.github/SECURITY.md` reserves that
> sequencing.
>
> **Stages 2–4 were re-measured before stage 2 was dispatched, and the
> original split was wrong** — stage plan §C. Nine was the number of
> `audit_ok`/`audit_with_note` **call sites**, not of commands with that
> shape: two of wave 4's commands emit **no record at all** (C09, C08),
> three have **no live write** (C10, C22, K02), and the client-update
> form is a second compound transition. **Stage 2 is dispatched** —
> [`m2b-0.85.0-stage2-c03-c04-c09-2026-10-25.md`](m2b-0.85.0-stage2-c03-c04-c09-2026-10-25.md):
> the form's three writes in one transaction. **It landed** `f03eabb`
> (Level B, 27 jobs, 0 skipped), G20 at **38** — one command, not three,
> because the runner appends exactly one event per call. **It retires two
> emitted audit event names**, `client.set_allowed_scopes` and
> `client.set_post_logout_redirect_uris`, which `0.85.0`'s summary must
> say (stage plan §G). **Stage 3 (C01, C02, C07, K03, C08) is next and
> not yet dispatched**; stage 4 is the three dormant rows, and carries a
> proposal to let the inventory say *sealed under another command's id* —
> C04 and C09 now read `sealed = false` while being sealed by C03.
>
> **The wave-3 dependency cycle is ruled** (2026-10-25): RFC 094 owns
> whether a write and its record commit together, RFC 096 owns what the
> write does. Wave 3 seals today's `federation_provider` writes onto the
> Class-A seam without waiting for 096-B2, which then extends a sealed
> command. **M2b's closure field is satisfiable as written** again, and
> 096-B2's prerequisite narrows from "all of M2b" to wave 3. Wave 3 is the
> next cycle's, not `0.85.0`'s.

> **`0.84.0` is M2b's settings area and is authorized** (2026-10-24):
> S01–S09, wave 2 before wave 1, because RFC 094's listed wave order is
> close to the reverse of the risk order —
> [`m2b-scope-and-0.84.0-2026-10-24.md`](m2b-scope-and-0.84.0-2026-10-24.md).
> Three stages: [`m2b-0.84.0-stage-plan-2026-10-24.md`](m2b-0.84.0-stage-plan-2026-10-24.md).
> **Stage 1 landed** `594c970` (Level B, 27 jobs, 0 skipped) —
> [`m2b-stage1-s07-s09-2026-10-24.md`](m2b-stage1-s07-s09-2026-10-24.md);
> G20's declared set moved 25 → 27. **Stage 2 landed** `b971955` (Level B, 27/0) —
> [`m2b-stage2-s08-2026-10-24.md`](m2b-stage2-s08-2026-10-24.md); G20 is
> at 28. **Stage 3 landed** `f77c85b` (Level B, 27/0) —
> [`m2b-stage3-s01-s06-2026-10-24.md`](m2b-stage3-s01-s06-2026-10-24.md),
> the last of `0.84.0`; G20 is at 34. **The plan described that wave
> wrongly**: four of its six leave no audit record at all on the production
> path, not "already atomic" — corrected in the scope document's §B′.
>
> **`0.84.0` is RELEASED AND PUBLISHED, 2026-10-25.** Tag `0.84.0` at
> `0ecacf5`; Level B on that exact commit (`workflow_dispatch`
> `40145716840`, 27 jobs, 0 skipped); `Security audit` green on the same
> commit (`40145718700`); six crates on crates.io. Preparation dispatched
> and returned the same day —
> [`release-0.84.0-preparation-2026-10-25.md`](release-0.84.0-preparation-2026-10-25.md).
> Reading the `Security audit` lane **before** dispatching it surfaced
> RUSTSEC-2026-0385, fixed in `5ed327d` (`ctutils` 0.4.3 → 0.4.5) and
> carried in this release; the checklist now requires that reading as
> step 0 rather than only before the tag.
> **`0.84.0` does not close M2b**: waves 3 and 4 and the authority switch
> remain after it.

**Governing RFC:** [RFC 094](../../accepted/094-transactional-audit-registry.md)
**Audience:** the mid-capability model after RFC re-acceptance and prerequisite evidence
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
- `@nabbisen` confirms the mid-capability model as implementation owner;
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
`rfcs/handoffs/094-transactional-audit/094-federation-command-amendment-review-2026-07-21.md`. The
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

## Review records

Dated review records for RFC 094 — and for the shared 094/095/096 reviews,
which live here as the lowest number they cover — relocated on 2026-09-10
when `rfcs/reviews/` was retired: RFC 000 sanctions four lifecycle folders
plus the optional `draft/` and this non-lifecycle `handoffs/` companion
folder, and per-RFC review records are companion documents, not a sixth
lifecycle.

| Record | What it is |
|---|---|
| [`094-design-review-2026-07-17.md`](094-design-review-2026-07-17.md) | Original independent design review of the base design |
| [`094-federation-command-amendment-review-2026-07-21.md`](094-federation-command-amendment-review-2026-07-21.md) | Review of the federation-command amendment |
| [`094-095-096-amendment-review-request-2026-08-03.md`](094-095-096-amendment-review-request-2026-08-03.md) | Request for the three-RFC amendment review |
| [`094-095-096-amendment-review-2026-08-12.md`](094-095-096-amendment-review-2026-08-12.md) | Three-RFC amendment review |
| [`094-095-096-correction-review-request-2026-08-12.md`](094-095-096-correction-review-request-2026-08-12.md) | First correction-review request |
| [`094-095-096-correction-review-request-2026-08-26.md`](094-095-096-correction-review-request-2026-08-26.md) | Second correction-review request |
| [`094-095-096-correction-review-2026-08-26.md`](094-095-096-correction-review-2026-08-26.md) | Correction review — the **Independent design review** evidence for RFCs 094, 095 and 096 |
| [`094-095-096-a6-sweep-request-2026-08-26.md`](094-095-096-a6-sweep-request-2026-08-26.md) | A6 implementability sweep request |
| [`094-095-096-a6-sweep-2026-08-27.md`](094-095-096-a6-sweep-2026-08-27.md) | A6 implementability sweep — cited by RFCs 094, 095 and 096 |
| [`094-lane-ownership-design-review-request-2026-08-27.md`](094-lane-ownership-design-review-request-2026-08-27.md) | Lane-ownership design review request |
| [`094-lane-ownership-design-review-2026-08-27.md`](094-lane-ownership-design-review-2026-08-27.md) | Lane-ownership design review — cited by RFC 094 |

These are historical records. They are not edited to reflect later
decisions; only their link paths were rewritten by the relocation. One
consequence: `094-lane-ownership-design-review-request-2026-08-27.md` §C
still describes `rfcs/reviews/` in the present tense. That sentence was
true on its date and is preserved as written.
