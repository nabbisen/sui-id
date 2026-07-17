# RFC 095 — Dynamic Client Registration Transaction and Validation

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFC 094 Accepted with baseline dynamic registration assigned to its Class-A transaction seam; RFC 7591 metadata scope and redirect/logout URI policy independently reviewed.
**Implementation prerequisites.** RFC 094 Implemented and its baseline dynamic-registration mutation/audit transaction passes; this RFC Accepted; adversarial test plan approved.
**Closure prerequisites.** Validate-first, single-transaction behavior passes invalid-input, rollback, retry, and concurrency tests; independent closure review accepts evidence.
**Tracks.** ROADMAP M3 — Atomic dynamic registration.
**Touches.** Dynamic registration handlers/core, `repos/client_registration_token.rs`, client repository, typed audit registry, metadata documentation and tests.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Implementation owner.** `codex-developer` (OpenAI Codex), after acceptance and prerequisites.
**Independent security and closure reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex).

## Summary

Complete dynamic registration on RFC 094's already-atomic baseline: validate
all supported metadata before entering that transaction, persist the complete
validated metadata set, and prove guarded limited-use concurrency/retry
semantics adversarially. RFC 094 already makes use consumption, disabled-client
creation, registration-source stamp, and typed audit one transaction, so no
production Class-A best-effort gap crosses the M2 boundary.

## Requirements and design boundary

- Parse and normalize the complete supported RFC 7591 subset before opening
  the write transaction. Validate every redirect URI and post-logout redirect
  URI, grant/response/auth method compatibility, scope policy, name bounds, and
  prohibited schemes/hosts according to the existing client policy.
- Re-check transaction-sensitive facts inside the transaction: token exists,
  is unexpired/unrevoked, remaining use is positive, and normalized client
  identity does not conflict.
- Use one guarded update/deletion for use consumption. Rows affected determines
  the winner; read-then-write arbitration is forbidden.
- Create the client disabled, persist all validated metadata, and append
  `client.dynamic_register` in the same RFC 094 transaction.
- A database, audit, uniqueness, serialization, or commit failure rolls back
  token consumption and client creation. A safe retry can succeed.
- Responses and timing do not disclose whether an opaque token was unknown,
  expired, revoked, or exhausted beyond the protocol’s approved error shape.
- Broad client-management API expansion is out of scope.

## Security invariants

- Invalid metadata never consumes a use.
- For a token with `n` remaining uses, at most `n` concurrent valid requests
  commit; every committed use corresponds to exactly one disabled client and
  one audit event.
- No client row is reachable with a partially validated URI set.
- Failure after guarded consumption but before commit leaves all three durable
  resources unchanged.
- Audit notes contain client ID and sanitized name only, never registration
  token or generated secret.

## Planned design work

Before acceptance, this RFC will be expanded with the exact supported metadata
table, normalization order, transaction signature, SQL guard, response/error
mapping, redirect/logout URI test corpus, and a focused implementation/QA
handoff. Those details are intentionally not guessed before RFC 094 fixes the
transaction API.

## Tests required for acceptance of implementation

- table-driven validation for every supported and rejected metadata field;
- multiple URI validation, loopback/native exceptions, fragments, userinfo,
  mixed schemes, Unicode/IDNA, duplicates, and oversized collections;
- failure injection at consumption, client insert, audit append, and commit;
- N-way concurrency at one remaining use: exactly one client/event/use commit;
- retries after rollback and opaque errors for invalid token states.

## Open questions

The exact RFC 7591 subset and native-app URI exceptions must be frozen during
detailed design. This proposal assigns ownership; it does not authorize coding.
