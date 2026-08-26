# RFCs 094, 095, 096 — correction-round review request, cut by role

**Date:** 2026-08-26 (Asia/Tokyo)
**Requested by:** `@nabbisen` (accountable owner and approver)
**Baseline:** `main` at the time of reading
**Supersedes:** `094-095-096-correction-review-request-2026-08-12.md`, which
routed this round to a reviewer defined by vendor. That definition was withdrawn
on 2026-08-26 — see `ROADMAP.md` §S1. The 08-12 request is retained as evidence,
**not** as instructions.

## 1. Who reviews what, and why

Independence here is **role independence** (RFC 018): the reviewer did not
author, implement, or previously approve the artifact. Vendor is not a
criterion.

The corrections under review were authored by the requirements-architect role.
That role cannot review them. This request therefore cuts the round in two:

| Part | Reviewer | Why that role |
|---|---|---|
| **A** | **Implementation role** (dev team) | It must build against these RFCs; it did not author or amend them. Most of Part A is decidable by running something or reading a named file. |
| **B** | **`@nabbisen`** | Two residual-risk acceptances that are owner decisions, not technical findings. |

**Part A is not advisory.** A finding from the implementation role is a finding.
If Part A returns corrections, the RFCs do not advance.

## 2. Part A — implementation role

Six items. Each is answerable by executing something or reading a named
artifact; none requires adjudicating an acceptable level of risk.

### A1 — `ReadConn` sufficiency (finding B-094-1) — **executable**

RFC 094 M2a requires `ReadConn` to reject data-modifying statements at runtime
via rusqlite's `Statement::readonly()` (`sqlite3_stmt_readonly`), alongside the
retained static denial list.

**Do not reason about this. Test it.** Write a throwaway harness that prepares
each of the following through `Statement::readonly()` and records the verdict:

- plain `SELECT`; `SELECT` with a scalar subquery
- `INSERT` / `UPDATE` / `DELETE` / `REPLACE`
- `PRAGMA` in every form the codebase could reach — read (`PRAGMA user_version`)
  and write (`PRAGMA user_version = 1`, `PRAGMA journal_mode = WAL`)
- `CREATE TABLE` / `DROP TABLE` / `ALTER TABLE`
- `ATTACH` / `DETACH`
- a `SELECT` calling an application-defined function registered by this project
- a `SELECT` against any virtual table this project registers
- `WITH ... AS (INSERT ... RETURNING ...) SELECT` if SQLite accepts it here
- `BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT`

**Report the table of statement → `readonly()` verdict.** The question is
whether any statement SQLite reports read-only can still change durable state,
and whether `readonly()` plus the static list together are complete. A
measured table settles this; an argument does not.

### A2 — deferred AST gate, reachability (finding B-094-2) — **checkable**

The AST gate stays in M2b. M2a's interim control is a sealed capability, a
private executor module, and the runtime read-only rejection in A1.

**Question:** with the AST gate absent, is there any reachable path by which a
caller outside the executor module obtains a connection and executes an
unregistered privileged statement? Trace the module boundary and the visibility
of every constructor. Report reachable paths found, or state that none exist and
name what you traced.

Acceptance of whatever residual remains is Part B. Your job is to establish
whether the path exists.

### A3 — RFC 095 entry gate (finding B-095-1) — **grep**

The entry gate was corrected to RFC 094 M2a with C15 evidence. Confirm no
companion document still states the full-RFC-094 prerequisite:

```
grep -rn '094' rfcs/handoffs/095-dynamic-client-registration/ rfcs/proposed/095*.md
```

Report any surviving full-RFC prerequisite.

### A4 — RFC 096 residual deployability text (finding B-096-1) — **read**

The numbered stages were deleted and replaced by an A/B1/B2/C map. 096-A no
longer claims to close the shipped signature-verification defect.

**Read `rfcs/proposed/096-*.md` and its handoff README end to end.** Report any
text that still implies 096-A is deployable on its own or closes the defect.

### A5 — the 096-B1/B2 split inference — **answerable from the inventory**

This is the item to attack hardest. The split was made on an inference that was
never verified, by an author with a recorded history of being confident and
wrong about what a control "really" requires.

The claim: 096-B bundled work from two RFC 094 waves, so it splits into 096-B1
(session establishment, on M2a) and 096-B2 (C17/C18/C23, on M2b).

**Both questions are answerable from `rfcs/handoffs/094-transactional-audit/command-inventory.md`:**

1. Does **federated** session establishment appear inside M2a's
   "credential, consent and session security" wave, or is it a distinct command
   absent from M2a's inventory?
2. Is a durable login-attempt row **Class-A**, or Protocol/Operational class? If
   it is not Class-A, does 096-B1 depend on the RFC 094 seam at all?

**Quote the inventory rows you relied on.** If either answer fails, the split is
wrong and 096-B2's boundary reopens — say so plainly.

### A6 — implementability sweep — **your primary role**

Independent of the six findings: read RFCs 094, 095 and 096 as the role that
must build them. Report anything under-specified, contradictory, or impossible
as written — a command whose class is unstated, an evidence requirement with no
producible artifact, a prerequisite that cannot be satisfied in the stated
order. This is the check no other role can perform, and it is why the design
routes to you.

## 3. Part B — `@nabbisen` only

Two items. Both are acceptances of residual risk, which is an owner decision.

**B1 — interim control sufficiency (from A2).** If A2 finds no reachable path,
the question remains whether shipping M2a with the AST gate deferred to M2b is
acceptable. The control is procedural (module privacy) plus runtime
(`readonly()`); the AST gate would be structural. Accept or require the gate in
M2a.

**B2 — the split's evidence bar (finding B-096-3).** The `federation.rs` split
must produce an observational-equivalence record. The stated risk is a security
check silently dropped during the move. **Is that evidence bar sufficient to
catch it?** If a dropped check would not change observable behaviour in any
recorded scenario, the bar does not catch the risk it was written for.

## 4. Required output

**Part A →** `rfcs/reviews/094-095-096-correction-review-2026-08-<DD>.md`,
committed and tracked. G11 requires evidence targets to resolve, be tracked, and
not be gitignored; `.git-exclude/` fails.

**Outcome vocabulary, stated per RFC:** Approved / Conditionally Approved /
Corrections Required / Design Revision Required / Requirements Clarification
Required / Human Owner Decision Required.

**State your role and what you checked**, so each RFC's `Independent design
review` field can record it truthfully when the RFC is accepted. Where you could
not check something, say so — it becomes a recorded unreviewed judgment, not a
silent pass.

## 5. Standing instruction

Every question in Part A was written by the author of the work under review.
Reject the framing of any of them if it is the wrong question, and say why.
That has already happened twice in this programme and both times the reviewer
was right.

---

`rfcs/reviews/094-095-096-correction-review-request-2026-08-26.md`
