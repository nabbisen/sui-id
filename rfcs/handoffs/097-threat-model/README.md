# RFC 097 handoff — current threat model baseline

**Governing RFC:** [RFC 097](../../proposed/097-current-threat-model.md) — Proposed
**Milestone:** M5
**Audience:** security reviewer and documentation owner
**Status:** Planning companion; inherits the governing RFC's current status —
Proposed.

## Entry gate

RFC 093 (**M1a and M1b**), RFC 094 (**M2a and M2b**), RFC 095, and RFC 096
(**096-A and 096-B**) Implemented; RFC 098 authority decision Accepted.

Stage names are load-bearing. A partially converted M2a or a validation-only
096-A does **not** satisfy the gate: this RFC documents finished behaviour, and
a threat model written against half-implemented controls would be exactly the
kind of aspirational claim it exists to eliminate.

## Why this RFC exists

`docs/threat-model.md` is current only through v0.26.0. Its scope lists HIBP and
SMTP as the external services, and its six trust boundaries omit LDAP, upstream
OIDC federation, dynamic registration, and metrics. Its limitations section
still describes completed work as future.

The replacement must be built from **verified behaviour**, not from design
intent — every control claim cites observed evidence from the milestone that
delivered it.

## Documents

| Document | Contents |
|---|---|
| [`review-checklist.md`](./review-checklist.md) | Boundary inventory, STRIDE coverage, and the residual-risk contract |

## Known boundaries the current model omits

LDAP bind and shadow-user cascade; upstream OIDC discovery, token, userinfo and
JWKS; registration-token issuance and consumption; metrics authentication;
secret and environment boundaries; callback SSRF; and failure/rollback
behaviour.

## Master-key boundary — do not omit

Offline master-key rotation is designed in RFC 100, not RFC 094. If RFC 100 is
not Implemented when this baseline is drafted, record master-key rotation as a
**manual, not-crash-safe operator procedure** and carry its interruption modes
as explicit residual risk. Do not omit the boundary, and do not imply it is
covered.

## Stop and return to the architect if

- a shipped boundary has no identifiable owner;
- a control claim cannot be traced to observed evidence;
- residual risk would have to be accepted without a named accepter;
- the model would have to describe intended rather than implemented behaviour.
