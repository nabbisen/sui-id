# RFC 119 — What a host must promise before sui-id can be embedded in it

**Status.** Accepted
**Accepted on.** 2026-11-01
**Approved by.** `@nabbisen`, 2026-11-01: *"Accepted."* — on the review below, which he asked for the same day. His first acceptance was recorded **2026-09-26**; the status could not move then, because G11 refuses an Accepted RFC whose security review is Required and which cites no design review, the guard adopted after RFC 105 shipped without one. **The 36 days between were not deliberation**: the review request named a reviewer that §S1a (2026-10-01) then forbade, so there was nobody it could go to. The architect did not work around the gate to record the status sooner.
**Security review.** Required
**Independent design review.** [Design review record, 2026-11-01](../handoffs/119-embedding-contract/design-review-2026-11-01.md). **By the architect, at `@nabbisen`'s request** — *"Review the questions carefully by yourself with my fundamental philosophy"* — which is the exception `ROADMAP.md` §S1c reserves to him, and the record says he asked. **The architect wrote this RFC and is therefore not independent of it.** §S1a places design review with him or an external architect he engages; none was engaged, which §S1a makes his option rather than a requirement.
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

**D7 — The enumeration is the deliverable.** Before the store boundary refactor (unwritten, and **not** RFC 120 — that number was reassigned to `consent-and-setup-prove-the-caller`, Implemented in v0.79.0; corrected 2026-10-20)
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

## Answers, 2026-11-01

`@nabbisen` asked the architect to review these *"carefully by yourself
with my fundamental philosophy"* — `ROADMAP.md` §S1c's owner-invoked
exception, and the record says he asked.

### Q1 — the module owns the count; the host owns a stable attempt id

**Half of this is solvable and the RFC was too pessimistic.**
*Inflation* — a host retrying a failed call and driving the count up —
is closed by requiring the host to supply an **attempt identifier
stable across its own retries**, with the module counting once per
distinct id. The machinery already exists: `federation_login_attempt`
is durable attempt state with one-time consumption (RFC 096-B1).

*Suppression* — a host reusing one id across genuinely distinct
attempts — **cannot** be closed, and it is not lockout-specific: a host
that can call the module can also decline to. It belongs in the
closure prerequisite's category (c).

### Q2 — the guarantee does not narrow, it goes away. Category (c)

**Settled by measurement, and the earlier framing of "a narrower
claim" was wrong.** Three properties would have to survive embedding
and none can:

- **Completeness** — the host decides whether to call at all.
- **Attribution** — the actor is whatever the host asserts.
- **Integrity** — the chain is `SHA-256(prev_hash || canonical_row_bytes)`
  (`repos/audit.rs:3`) with **no key and no external anchor**. A host
  with database write access recomputes it and leaves nothing to find.

**And it cannot be fixed in-process**: the module cannot keep a secret
from the host — same address space — and cannot reach a sink the host
cannot block. So **no construction makes an embedded audit log
trustworthy against the host.**

`@nabbisen`, 2026-11-01: *"When embedded, it seems impossible to me if
sui-id can not guarantee the log."* That is the correct reading, and it
is structural rather than a matter of policy or wording.

**So: category (c).** The embedded module's audit rows are a record
*the host keeps* — sui-id's schema, sui-id's algorithm, and sui-id
vouching for neither its completeness nor its integrity. The
documentation says so plainly, in the embedder's terms.

**The line this draws, which is the useful part.** If an embedder needs
a trail sui-id vouches for, the sink must sit **outside the host's
trust boundary** — which is not an embedding of that component but a
service the host talks to. So: embed the authentication logic, policy
and validation; **do not embed the thing that must be trustworthy
against the embedder.**

*This review also found a live defect outside this RFC: the operators
guide claimed a SQL-level rewrite always leaves a detectable mismatch.
It does not, for the same reason as above. Corrected 2026-11-01 in
`docs/src/guides/operators.md`.*

### Q3 — settled: a UI-less embedding is supported, and it is the primary case

**`@nabbisen`, 2026-11-01: *"Agreed. It is contract instead of UI
function."*** — which is the better framing and is adopted as the
section's own.

A host with its own UI is the realistic consumer, so UI-less is not an
edge case. The difficulty the contract must carry: **several security
properties here are produced by the UI rather than the logic** — RFC
124's uniform response, the neutral forgot-password reply, the
invalid-link page identical for unknown, used and expired tokens. A
host reimplementing the presentation reproduces the look and loses the
property, because non-enumeration looks like a cosmetic choice.

**So the obligation is on the API's shape, not on the host's
discipline**: where indistinguishability *is* the property, the module
must not expose the distinction at all. A leak the host cannot commit
beats a leak the host is told not to commit.
