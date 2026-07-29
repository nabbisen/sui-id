# RFC 097 — Current Threat Model and Security-Assurance Baseline

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFC 093 (M1a **and** M1b), RFC 094 (M2a **and** M2b), RFC 095, and RFC 096 (096-A **and** 096-B) Implemented; RFC 098 authority decision Accepted. Stage names are load-bearing after the 2026-07-28 restructure: a partially converted M2a or a validation-only 096-A does not satisfy this prerequisite, because the threat model must describe finished behaviour.
**Implementation prerequisites.** This RFC Accepted; verified behaviour and evidence available for every stage named above.
**Master-key boundary.** Offline master-key rotation is designed in [RFC 100](./100-master-key-rotation-recovery.md), not RFC 094. If RFC 100 is not Implemented when this baseline is drafted, the threat model must record master-key rotation as a **manual, not-crash-safe operator procedure** with its interruption modes as explicit residual risk, rather than omitting the boundary or implying it is covered.
**Closure prerequisites.** Every shipped external trust boundary and material cross-boundary attack has an owner, control/evidence link, failure/rollback analysis, and independently accepted residual risk.
**Tracks.** ROADMAP M5 — Threat-model reconciliation.
**Handoff.** [`../handoffs/097-threat-model/README.md`](../handoffs/097-threat-model/README.md)
**Touches.** `docs/threat-model.md`, security assurance documentation, boundary diagrams and evidence references.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Implementation owner.** `codex-developer` (OpenAI Codex), after acceptance and prerequisites.
**Independent security and closure reviewer.** **Vendor independence required** by the owner's 2026-07-28 S1 ruling: the reviewer of record must be outside the vendor that authored and implemented this RFC. `codex-independent-architecture-security-reviewer` (OpenAI Codex) may review, but cannot alone satisfy this RFC's independence requirement.

## Summary

Replace historical or aspirational threat claims with a current baseline built
from verified behavior after RFCs 093–096. Model every shipped external trust
boundary, STRIDE threats and cross-boundary compositions, SSRF and secret
boundaries, failure/rollback behavior, operational assumptions, evidence, and
residual risks.

## Requirements

- Inventory actors, assets, entry points, data stores, processes, outbound
  connections, administrative paths, packaging/runtime files, and trust zones.
- Cover local/password/MFA/passkey auth, OAuth/OIDC, sessions/tokens, dynamic
  registration, upstream federation, LDAP, SMTP, HIBP, metrics, audit, setup,
  backup/restore, key rotation, upgrades, and operator/CI supply-chain paths.
- Analyze threats per boundary and material compositions across boundaries;
  include confused deputy, replay, race, rollback, SSRF, injection, disclosure,
  availability, and privileged-operator compromise.
- Tie each claimed mitigation to current code, an Accepted/Done RFC, and
  observed evidence where applicable. Mark unverified assumptions explicitly.
- Record residual risk, risk owner, detection/recovery, and review decision.
- Do not introduce implementation changes owned by RFCs 093–096; discovered
  gaps return to the owning RFC or receive a new RFC and block closure.

## Deliverables and method

Before acceptance, define a versioned boundary/asset inventory schema, diagram
format, STRIDE worksheet, risk-rating rubric, evidence-link syntax, residual
risk register, and independent review checklist. The final model is derived
only after upstream behavior is implemented and observed; copied historical
claims are inputs, not evidence.

## Open questions

Exact diagram tooling and risk-rating scale remain for detailed design after
RFC 098 identifies the authoritative document set.
