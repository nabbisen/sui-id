# RFC 097 — Current Threat Model and Security-Assurance Baseline

**Status.** Accepted
**Accepted on.** 2026-11-23, by `@nabbisen`: *"Your recommendation is accepted."* That answered the architect's [method draft](../handoffs/097-threat-model/method-2026-11-23.md), §7, and accepts all three of its points: the method, the owner as security reviewer, and stage 1 in cycle P.
**Approved by.** `@nabbisen`, 2026-11-23: *"Your recommendation is accepted."*
**Security review.** Required
**Independent design review.** By `@nabbisen`, who did not author this RFC or its [method](../handoffs/097-threat-model/method-2026-11-23.md). His review is recorded as that acceptance of the method draft's §7, with no separate findings document. The method's own stages put each boundary's evidence to review as it is built.
**Design prerequisites.** RFC 093 (M1a **and** M1b), RFC 094 (M2a **and** M2b), RFC 095, and RFC 096 (096-A **and** 096-B) Implemented; RFC 098 authority decision Accepted. Stage names are load-bearing after the 2026-07-28 restructure: a partially converted M2a or a validation-only 096-A does not satisfy this prerequisite, because the threat model must describe finished behaviour.
**Implementation prerequisites.** This RFC Accepted; verified behaviour and evidence available for every stage named above.
**Master-key boundary.** Offline master-key rotation is designed in [RFC 100](100-master-key-rotation-recovery.md), not RFC 094. If RFC 100 is not Implemented when this baseline is drafted, the threat model must record master-key rotation as a **manual, not-crash-safe operator procedure** with its interruption modes as explicit residual risk, rather than omitting the boundary or implying it is covered.
**Closure prerequisites.** Every shipped external trust boundary and material cross-boundary attack has an owner, control/evidence link, failure/rollback analysis, and independently accepted residual risk. **Consolidated 2026-10-06 with `ROADMAP.md`'s M5 row**, which required that **authoritative documents are identified, the mdBook and integrity gates pass, and public claims match code** — the documentation half of M5, which belongs to RFC 098. **RFC 098 is closed in `done/` and is deliberately not edited**; its clauses are recorded here so M5 has one place to be read from, and the ROADMAP row points at both.
**Tracks.** ROADMAP M5 — Threat-model reconciliation.
**Handoff.** [`../handoffs/097-threat-model/README.md`](../handoffs/097-threat-model/README.md)
**Touches.** `docs/threat-model.md`, security assurance documentation, boundary diagrams and evidence references.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Implementation owner.** The mid-capability model (implementation and testing agent), after acceptance and prerequisites.

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

## Refresh, toward acceptance (2026-11-23)

M4-B closed on 2026-11-23, which cycle O named as the trigger for this
draft. **The entry gate is met at `0.93.0`'s cut**, measured: RFC 096 is
accepted in every phase, and moves to `done/` then. **The method this RFC
requires before acceptance is drafted** in
[`../handoffs/097-threat-model/method-2026-11-23.md`](../handoffs/097-threat-model/method-2026-11-23.md).
It proposes answers to both open questions above:
- **risk scale:** 3 × 3, with anchors for who can reach a threat and what
  it costs;
- **diagram tooling:** none; text diagrams beside the tables.

It also proposes:
- a versioned inventory contract, whose gate makes an anonymous residual
  acceptance impossible;
- evidence links by symbol rather than line;
- five stages;
- a **classification-consistency** check, from the RFC 103 T10 lapse found
  on 2026-11-22.

**Master-key rotation** is recorded as this RFC requires while RFC 100 is
part-built: a manual, not-crash-safe procedure, with the stage 1–4
protections recorded as protected. **Accepted 2026-11-23**, with that
method.
