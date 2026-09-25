# RFC 119 — What a host must promise before sui-id can be embedded in it

**Status.** Proposed
**Security review.** Required
**Owner acceptance.** Recorded 2026-09-26: `@nabbisen` said "Accepted." **The status stays Proposed**, because G11 refuses an Accepted RFC whose security review is Required and which cites no independent design review — the guard adopted after RFC 105 shipped without one. His acceptance is not in doubt; the status changes the moment the review lands, and the architect does not work around the gate to record it sooner.
**Design prerequisites.** None. This RFC is the prerequisite: RFC 121 (the split; not yet written) may not be written until this is Accepted.
**Implementation prerequisites.** None — **this RFC ships no code.** It is a contract and a conformance suite's specification.
**Closure prerequisites.** Every security property sui-id claims today is either (a) carried by the embeddable module itself, (b) written here as a numbered obligation on the host with a stated failure mode and a conformance test, or (c) named as a property an embedding cannot have, in which case sui-id says so in its own documentation. No property is left unassigned, and sui-id's own service crate satisfies the contract through the published interface rather than around it.
**Tracks.** Architecture. Raised by `@nabbisen` on 2026-09-25 as one of two themes for the future.
**Touches.** This RFC and its handoff only. No crate, no gate, no document changes until it is Accepted.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/119-embedding-contract/README.md`](../handoffs/119-embedding-contract/README.md)

## Summary

sui-id is a web service. The proposal is to split it into an **embeddable
authentication module** other frameworks and HTTP servers can introduce, and
the service that remains. This RFC writes the contract that split has to
satisfy. It is deliberately first: a module that compiles into any framework
and guarantees nothing is worse than no module, because it looks like an
identity provider and is not one.

## The measurement this RFC starts from

Taken 2026-09-25 at `2adf796`. **The web boundary already largely exists:**
`axum` appears outside `crates/sui-id/` exactly once, in a doc comment at
`crates/sui-id-core/src/authn/hibp.rs:241`, and 11,347 of that crate's 14,878
lines are `src/http/`.

**But security-relevant behaviour lives in the part that would stay behind.**
Measured, in `crates/sui-id/src/http/`:

| Behaviour | Where it lives today | What an embedder taking core + store gets |
|---|---|---|
| CSRF protection (double submit, `SameSite=Lax`, `Secure` per operator config) | `http/csrf.rs` | **nothing** |
| The client address the audit trail records as the actor, with its deliberate trusted-proxy policy — `X-Forwarded-For` is read **only** when proxies are configured | `http/handlers.rs:332-361` | **nothing** |
| Session cookie carriage and attributes | `http/` | **nothing** (the session *logic* is in `sui-id-core/src/authn/session.rs` and `sui-id-store/src/repos/sessions.rs`) |

So "embed the core" silently drops CSRF and makes every audit record's actor a
guess. **This is the whole reason the contract comes before the split**: the
properties at risk are invisible from the crate graph, and an embedder who
reads only the dependency list will not find them.

## Decision

**D1 — The contract is a numbered, versioned artifact, and an embedding that
does not state which version it satisfies is unsupported.** sui-id's
documentation says so. A security property that rests on an unstated
assumption is not a property.

**D2 — Each obligation names its failure mode.** An obligation whose breach
has no stated consequence will be skipped. The obligations are drafted in the
handoff; each carries what breaks when it is not met — for example, a host
that does not rotate its session identifier on authentication gives an
attacker session fixation against an IdP, and a host that forwards a
client-supplied `X-Forwarded-For` without a trusted-proxy policy makes every
audit actor forgeable.

**D3 — The module owns its own transaction, and that is not negotiable.**
RFC 094's Class-A seam is defined as the audit row being written *in the same
transaction as the effect it describes*. A host that owns or wraps the
database transaction can break that without any visible failure, so the
embeddable module holds its own connection and its own transaction boundary.
If an embedder wants one transaction across their data and sui-id's, the
answer is no, and the reason is stated rather than negotiated.

**D4 — Nothing whose failure is silent is delegated to the host.** This is the
test that decides, for each item in the table above, whether it moves into the
module, becomes a host obligation, or blocks embedding. CSRF failure is
silent; clock skew is silent; a forged actor in an audit record is silent.
Where a host obligation would fail silently, the behaviour moves into the
module instead, even when that makes the module larger than a purist would
like.

**D5 — sui-id's own service crate is the first embedder.** `crates/sui-id`
consumes the module through **exactly** the published interface, not around
it. Then every existing end-to-end test exercises the contract, and the
contract cannot drift from the only implementation known to be correct. An
interface with one privileged internal caller and one public one is two
interfaces, and the public one is the one that rots.

**D6 — Non-goals, stated so they are not rediscovered.** This is not a plugin
ABI, not dynamic loading, and not a stable Rust ABI; the boundary is a Rust
API under semver. It does not make sui-id framework-agnostic at the HTTP
layer by itself — that is RFC 121 (the split; not yet written). It does not
commit to publishing anything: publishing is an outward-facing act and
`@nabbisen`'s decision separately.

**D7 — The enumeration is the deliverable.** Before RFC 120 (the store boundary; not yet written)
moves a line, every security-relevant behaviour in `crates/sui-id/src/http/`
is enumerated and assigned one of D4's three outcomes. The table above has
three rows because that is what one afternoon's measurement found; the real
set is larger, and finding it is this RFC's work, not RFC 121's.

## Open questions

1. **Does the host or the module own rate limiting and lockout counting?** The
   lockout counts failed attempts. A host that retries a failed call inflates
   the count and locks a user out; a host that deduplicates suppresses it.
   Both are silent, so D4 suggests the module owns it — but the module cannot
   see the host's retries. This may be the first obligation that cannot be
   made safe, and if so this RFC says so rather than papering over it.
2. **What does an embedded sui-id's audit trail claim to prove?** The hash
   chain is tamper-evident within its trust boundary. Embedded, the host is
   inside that boundary. The honest answer may be a narrower claim.
3. **Is there a supported embedding that does not include the UI?** The
   `sui-id-web` crate is leptos. A host with its own UI wants the module
   without it, which changes what "the user is told" means for RFC 118 and
   every other decision that promises the user a message.

Questions 1–3 are for the independent design review to attack and for
`@nabbisen` to settle; the architect does not treat them as decided.
