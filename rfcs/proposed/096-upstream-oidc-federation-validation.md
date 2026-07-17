# RFC 096 — Upstream OIDC Federation Validation

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFC 093 Accepted; federation threat delta and discovery/egress policy independently reviewed.
**Implementation prerequisites.** RFC 093 Implemented with passing all-feature gates; this RFC Accepted; hostile-provider test harness approved.
**Closure prerequisites.** Discovery, transport, JOSE, claims, nonce, cache/rotation, hostile-provider, and live representative integration evidence passes independent review.
**Tracks.** ROADMAP M4 — Federation trust completion.
**Touches.** Federation configuration and handlers, outbound HTTP, discovery/JWKS cache, JOSE/ID-token validation, login state, threat model and integration tests.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Implementation owner.** `codex-developer` (OpenAI Codex), after acceptance and prerequisites.
**Independent security and closure reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex).

## Summary

Complete the upstream OIDC relying-party trust boundary: constrain discovery
and egress, require HTTPS and exact issuer binding, validate JWKS signatures
and algorithms, verify required ID-token claims and a mandatory one-time nonce,
and define bounded cache/key-rotation behavior under hostile providers.

## Requirements and design boundary

- Canonical configured issuer must exactly match discovery metadata and the
  token `iss`; endpoint URLs must satisfy the approved HTTPS/egress policy.
- DNS/IP checks and redirect handling prevent loopback, link-local, private,
  metadata-service, rebinding, and cross-scheme SSRF unless an explicit
  deployment policy independently approves a narrow target.
- Responses have size/time/content-type limits and no ambient credentials.
- ID tokens require an allowed asymmetric algorithm, matching key/issuer,
  audience and `azp` rules, expiry/not-before/issued-at skew policy, and a
  cryptographically random mandatory nonce bound to one login attempt.
- State and nonce are one-time, expiry-bound, and consumed with replay-safe
  semantics. Upstream MFA claims never satisfy local MFA automatically.
- JWKS caching defines positive/negative TTLs, unknown-`kid` refresh bounds,
  rotation overlap, stale-key behavior, request coalescing, and fail-closed
  behavior without creating an attacker-controlled fetch loop.
- New providers, account-link UX expansion, and trusting upstream MFA as local
  MFA are out of scope.

## Security invariants

No authorization follows from unsigned/unverified claims. Provider
substitution, algorithm confusion, key confusion, nonce omission/mismatch,
issuer/audience/time errors, hostile endpoints, oversized bodies, redirect
escape, and bad/rotating keys fail closed with non-enumerating responses and
sanitized telemetry.

## Planned design work

Before acceptance, add exact URL canonicalization and IP-range rules, claim
validation matrix, state/nonce schema and state machine, cache algorithm,
error taxonomy, attack corpus, migration policy, shared-file ownership, and a
staged developer handoff. RFC 096 may overlap RFCs 094–095 only if an
independent second implementer and reviewer capacity are recorded.

## Tests required for acceptance of implementation

Use a local hostile-provider fixture for discovery substitution, redirects,
SSRF destinations, duplicate/ambiguous JSON, wrong algorithms/keys, missing or
mismatched nonce, state replay, bad issuer/audience/time claims, oversized/slow
responses, unknown-`kid` storms, and key rotation. A representative live
integration is required before closure but never supplies real secrets to CI.

## Open questions

Deployment-specific private-network issuer exceptions and exact cache bounds
remain for detailed design and independent threat review.
