# RFC 126 — Password hashing must not block the request runtime

**Status.** Proposed
**Security review.** Required
**Independent design review.** Found by the [RFC 123 design review](../handoffs/123-authenticating-a-client-costs-the-caller/design-review-2026-09-30.md), 2026-09-30, by the implementation role, while answering whether a rate limit was sufficient. Confirmed independently by the architect, who corrected one detail of it: `spawn_blocking` **is** used in this codebase, which strengthens rather than weakens the finding.
**Design prerequisites.** None.
**Implementation prerequisites.** None. Independent of [RFC 123](../accepted/123-authenticating-a-client-costs-the-caller.md), which closes a different half of the same exposure.
**Closure prerequisites.** No request-path call to Argon2 runs on a runtime worker thread: hashing and verification are executed where blocking is expected, every call site is converted, and a test demonstrates that concurrent authentication attempts do not stall unrelated requests.
**Tracks.** Availability.
**Touches.** `crates/sui-id-core/src/authn/password.rs` and every call site of `verify_password` / `hash_password`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/126-password-hashing-must-not-block-the-runtime/README.md`](../handoffs/126-password-hashing-must-not-block-the-runtime/README.md)

## Summary

Argon2 is configured at **64 MiB and two passes** (`authn/password.rs:11-13`),
and one verification was measured at **~34 ms** (RFC 123's design review). Every
call runs **synchronously on the Tokio worker thread serving the request**. The
runtime is a plain `#[tokio::main]` with no `worker_threads` override
(`crates/sui-id/src/main.rs:22`), so there is one worker per core.

On the deployment shape this project describes — self-hosted, single-tenant,
plausibly one to four cores — a handful of simultaneous sign-ins or token
requests occupies every worker for 34 ms each. **Nothing else the service is
doing runs during that time**: not another user's sign-in, not a health probe,
not an unrelated OIDC discovery request.

## Why a rate limit does not close it

RFC 123 adds rate limits to the two endpoints that lacked them. A limiter bounds
**requests per window**. This is **requests in flight at once**, and the two are
not the same: a burst well inside any per-window budget still lands on every
worker simultaneously. `/oauth2/token` has had a limiter all along and is
exposed to this today.

## The correction that makes the fix smaller

The design review reported that `spawn_blocking` appears nowhere in the
codebase. It does: `sui-id-store/src/backend.rs` uses it for database work, and
`authn/hibp.rs` uses it too. **The review's load-bearing claim — that
`verify_password` and `hash_password` are never wrapped — is correct**, and I
verified it.

So this is not a new idiom. It is **the idiom this project already applies to
database reads, withheld from the one operation that costs a thousand times
more.** Microsecond SQLite reads are moved off the worker threads; 34 ms of
Argon2 is not.

## Decision

**D1 — Argon2 does not run on a runtime worker thread.** Every request-path
`verify_password` and `hash_password` executes where blocking is expected.

**D2 — The boundary is in one place.** The wrapping lives at the password
module's own API, not at each of the ~20 call sites, so a new caller cannot
reintroduce the defect by forgetting. If that is not achievable — because a
caller needs a borrowed value across the boundary — the RFC says so and names
what each call site must then do instead.

**D3 — The dummy verifications keep their timing property.**
`authn/session.rs`'s `DUMMY_PHC` calls, and the ones RFC 123 adds to
`authenticate_client`, exist to equalise timing. Moving hashing off the worker
threads must not make a real verification and a dummy one distinguishable again.
A test holds that.

**D4 — Concurrency is bounded deliberately, not by accident.** Moving the work to
a blocking pool converts "every worker stalls" into "the pool saturates". That is
better, and it is not free: an unbounded pool under load becomes memory pressure
at 64 MiB per concurrent hash. The RFC states the bound it chooses and why.

**D5 — The evidence is a demonstration, not an assertion.** A test issues
concurrent authenticating requests and shows an unrelated request served during
the burst — failing before the change and passing after. Until that exists this
RFC has not been shown to do anything.

## Scope

Not a change to the Argon2 parameters. 64 MiB and two passes are a deliberate
cost chosen against offline cracking, and lowering them to make this problem
smaller would trade a credential-strength property for an availability one. That
trade is not proposed here and should not be made incidentally.
