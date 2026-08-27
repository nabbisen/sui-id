# RFCs 094, 095, 096 — independent design review of the 2026-07-28 amendments

**Date:** 2026-08-03 (Asia/Tokyo)
**Requested by:** `@nabbisen` (accountable owner and approver)
**Requested of:** `codex-independent-architecture-security-reviewer` (OpenAI Codex)
**Amendment author:** high-capability model (Anthropic Claude) — **not available as
reviewer of these amendments**
**Baseline:** `1d58da2`

## Why you are being asked

All three RFCs carry `Security review: Required` and sit in `rfcs/proposed/`. They
were returned there on 2026-07-28 for owner-approved material amendments. **None
carries an `Independent design review` field**, and RFC 093's integrity gate (G11,
now live and blocking) refuses to let a security-required RFC reach
`Status: Accepted` without one, backed by a tracked, resolving evidence link.

So this review is not advisory. Until it exists, RFCs 094, 095 and 096 cannot be
re-accepted, and M2a cannot start.

**Independence required.** The amendments were authored by the Anthropic
high-capability model acting as requirements architect. Under `@nabbisen`'s S1
ruling, these three need review from outside the authoring vendor. You satisfy that.
The original designs are yours-lineage (`codex-project-architect`); the amendments
are not.

**This brief deliberately omits the amendment author's reasoning.** You are given
what changed and where, not why the author believed it correct. If you want that
rationale after forming your own view, it is in
`.git-exclude/reviewed/m1-rfc-093-096-100-governance-amendment-review-*.md`. Reading
it first would defeat the purpose of asking you.

## Artifacts

| Path | What it is |
|---|---|
| `rfcs/accepted/094-transactional-audit-registry.md` | Amended RFC |
| `rfcs/accepted/095-dynamic-client-registration-transaction.md` | Amended RFC |
| `rfcs/accepted/096-upstream-oidc-federation-validation.md` | Amended RFC |
| `rfcs/reviews/094-design-review-2026-07-17.md` | Original independent review |
| `rfcs/reviews/094-federation-command-amendment-review-2026-07-21.md` | Prior amendment review |
| `rfcs/reviews/096-design-review-2026-07-21.md` | Original independent review |
| `rfcs/done/018-rfc-lifecycle-policy.md` | Governs the security-review classification |
| `ROADMAP.md` | Milestone structure, risk register, execution order |

Each RFC's `Lifecycle history` and `Amendment summary` fields state what changed.

## What changed, factually

**RFC 094 — transactional audit registry.** Master-key rotation crash recovery
removed to a new RFC 100. `ReadConn` narrowed to a typed wrapper plus static denial,
deferring per-statement runtime interrogation. The `syn` AST boundary gate sequenced
into the M2b authority switch. Conversion of the 62 Class-A commands phased across
M2a and M2b, with C15 pinned to M2a. Acceptance criteria split per stage. Interim
documentation honesty made normative.

**RFC 095 — dynamic client registration.** Implementation prerequisite re-pointed
from "RFC 094 Implemented" to "RFC 094 M2a Implemented".

**RFC 096 — upstream OIDC federation validation.** Implementation prerequisite
re-pointed from full RFC 093 to M1a. A previously inline validation-versus-mutation
caveat promoted into two normative stages, 096-A and 096-B. Preparatory
`federation.rs` split added as a prerequisite. File ownership against RFC 094 named.

## Questions this review must answer

1. **Does splitting RFC 094 into M2a/M2b leave a window in which audit writes are
   neither atomic under the old path nor covered by the new one?** C15 is pinned to
   M2a; the rest of the conversion is M2b. Is that boundary safe, or does it create
   a partially-converted state with weaker guarantees than either endpoint?

2. **Is the narrowed `ReadConn` sufficient?** It is now a typed wrapper plus static
   denial, with per-statement runtime interrogation deferred. Can a read path still
   perform a write, and if so what is the consequence?

3. **Does deferring the `syn` AST gate to M2b leave M2a's conversions unverified by
   anything mechanical?**

4. **Is "interim documentation honesty made normative" adequate**, or does the
   partially-converted state require a stronger claim-suppression control?

5. **RFC 095:** is "RFC 094 M2a Implemented" genuinely sufficient for dynamic client
   registration, or does it depend on M2b work?

6. **RFC 096:** does the 096-A / 096-B split allow a state where ID tokens are
   validated but the mutation path is not yet guarded, or vice versa? Is the
   `federation.rs` split a real prerequisite or a convenience?

7. **Across all three:** does any amendment weaken a security property that the
   original independent reviews relied on when accepting the unamended designs?

## Required output

**Path:** `rfcs/reviews/094-095-096-amendment-review-2026-08-<DD>.md` — one document
covering all three, or three documents named per RFC, at your discretion.

**It must be committed and tracked.** G11 verifies the `Independent design review`
target resolves, is tracked (`git ls-files --error-unmatch`), and is **not**
gitignored. A path under `.git-exclude/` will fail the gate.

**Outcome vocabulary** (project framework): Approved / Conditionally Approved /
Corrections Required / Design Revision Required / Requirements Clarification
Required / Human Owner Decision Required.

State per RFC, not once for all three.

## After the review

If approved, each RFC needs `Accepted on`, `Approved by`, and
`Independent design review` (naming you, linking your document) added when it moves
back to `rfcs/accepted/`. G11 will verify all three fields.

Reject the framing of any question above if you think it is the wrong question. The
questions are the amendment author's guess at where the risk is, and that guess is
exactly what an independent review exists to check.

---

`.git-exclude/review-requests/094-095-096-amendment-independent-design-review-request-2026-08-03.md`
