# RFCs 094, 095, 096 — correction-round review request

**Date:** 2026-08-12 (Asia/Tokyo)
**Requested by:** `@nabbisen` (accountable owner and approver)
**Requested of:** a reviewer satisfying §1 — **not yet assigned**
**Baseline:** `0fcb423`
**Supersedes:** `094-095-096-amendment-review-request-2026-08-03.md`, which
misstated the independence requirement. It is retained as evidence, **not** as
instructions.

## 1. Who may perform this review

Two constraints apply at once, and they exclude different parties.

**Vendor independence — excludes OpenAI.** RFCs 094 and 096 state it in their own
headers:

> "the reviewer of record must be outside the vendor that authored and implemented
> this RFC. `codex-independent-architecture-security-reviewer` (OpenAI Codex) may
> review, but cannot alone satisfy this RFC's independence requirement."

Both were authored by `codex-project-architect` and are implementation-owned by
`codex-developer`, both OpenAI.

**Role independence — excludes the Anthropic high-capability model.** It authored
the 2026-07-28 amendments *and* the 2026-08-12 corrections this round reviews. It
cannot review either.

**Consequence.** Full independence requires a party that is neither OpenAI nor the
Anthropic model that wrote the amendments. If no such party is available, the owner
may proceed with a partially-independent reviewer — but the RFC metadata must then
record **which** independence was achieved and which was not. It must not read as
an unqualified independent review. That misrecording is exactly the defect that
occurred on 2026-08-03 and was caught by the reviewer rather than by us.

**RFC 095 is different.** Its header says role independence alone is sufficient,
because its amendment is a single prerequisite re-point. It has already received a
qualifying review; only its correction needs confirming.

## 2. What this round is, and is not

The 2026-08-12 Codex review found six blocking issues. All six now have
corrections, authored by the same party that wrote the amendments they correct.

**Your job is not to re-review the original amendments.** It is:

1. **Do the corrections actually close the findings?**
2. **Did any correction introduce a new problem?**
3. **Is the one explicitly unconfirmed inference correct?** — see §4.

## 3. Artifacts

| Path | Role |
|---|---|
| `rfcs/proposed/094-transactional-audit-registry.md` | Corrected RFC |
| `rfcs/proposed/095-dynamic-client-registration-transaction.md` | Unchanged; its handoff was corrected |
| `rfcs/proposed/096-upstream-oidc-federation-validation.md` | Corrected RFC |
| `rfcs/reviews/094-095-096-amendment-review-2026-08-12.md` | The findings being answered |
| `rfcs/handoffs/094-transactional-audit/` | Corrected: `migration-checklist.md` |
| `rfcs/handoffs/095-dynamic-client-registration/README.md` | Corrected entry gate |
| `rfcs/handoffs/096-upstream-oidc-federation/README.md` | Re-cut delivery map |
| `ROADMAP.md` | `prep` item gained the split's ownership and evidence bar |

Corrections landed in commits `6a02055` and `0fcb423`.

## 4. The claim to attack hardest

**096-B was split into 096-B1 and 096-B2 on an inference that has not been
verified.**

The reasoning: RFC 096's single 096-B stage named RFC 094 **M2a** as its
prerequisite while bundling work from two RFC 094 conversion waves — session
security (M2a) and federation configuration (M2b). The split puts session
establishment in 096-B1 on M2a, and C17/C18/C23 in 096-B2 on M2b.

**Unverified, and load-bearing:**

- Does **federated** session establishment actually fall inside M2a's "credential,
  consent and session security" wave, or is it a distinct command absent from
  M2a's inventory?
- Is a durable login-attempt row **Class-A** at all, or Protocol/Operational class?
  If it is not Class-A, does 096-B1 depend on the RFC 094 seam at all?

Both are answerable from RFC 094's command inventory. If either fails, the split is
wrong and the owner's B-096-2 choice reopens.

The author of the split has a recorded history of reasoning confidently about what
a control "really" requires and being wrong — B-094-1 is precisely that failure.
Treat this inference accordingly.

## 5. Specific questions per finding

**B-094-1 — `ReadConn`.** The correction makes `sqlite3_stmt_readonly` a required
M2a control. Is interrogating the prepared statement *before stepping* sufficient
to reject every data-modifying form, or are there statements SQLite reports as
read-only that still change durable state — application-defined functions,
`PRAGMA` variants, virtual tables? The static denial list is retained alongside;
are the two together complete?

**B-094-2 — AST gate.** The gate stays in M2b, and M2a's interim control is now
named: sealed capability, private executor module, runtime read-only rejection. Is
that interim control genuinely sufficient for M2a, or does deferring the AST check
still leave an unregistered authority path reachable?

**B-095-1 — entry gate.** Corrected to RFC 094 M2a with C15 evidence. Confirm no
other companion document retains the full-RFC prerequisite.

**B-096-1 — stages.** The numbered stages are deleted and replaced by an
A/B1/B2/C map in both RFC and handoff. 096-A no longer claims to close the shipped
defect. Does any residual text still imply 096-A is deployable or defect-closing?

**B-096-3 — the split.** Ownership assigned to the ROADMAP `prep` item, with a
required observational-equivalence record. Is the evidence bar sufficient to catch
a silently dropped security check, which is the stated risk?

## 6. Required output

**Path:** `rfcs/reviews/094-095-096-correction-review-2026-08-<DD>.md`, committed
and tracked. G11 requires evidence targets to resolve, be tracked, and not be
gitignored; `.git-exclude/` fails.

**Outcome vocabulary:** Approved / Conditionally Approved / Corrections Required /
Design Revision Required / Requirements Clarification Required / Human Owner
Decision Required. State it **per RFC**.

**State your independence position explicitly** — which of the two constraints in
§1 you satisfy — so the RFC metadata can record it truthfully.

## 7. If approved

Each RFC returns to `rfcs/accepted/` with `Accepted on`, `Approved by`, and
`Independent design review` naming you and linking your document. G11 verifies all
three, and verifies the link resolves and is tracked.

Reject the framing of any question here if it is the wrong question. Every question
above was written by the author of the work under review.

---

`rfcs/reviews/094-095-096-correction-review-request-2026-08-12.md`
