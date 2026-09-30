# RFC 126 — Password hashing must not block the request runtime

**Status.** Accepted
**Accepted on.** 2026-09-30
**Approved by.** `@nabbisen`, 2026-09-30: "Accepted."
**Design review, held open after acceptance and now closed.** This RFC was
accepted before its own design had been reviewed — the field below first cited
the review that *found* the problem, which examined RFC 123's design, not this
one's. G11 could not see that, because the citation resolved to a tracked
document. The architect held implementation undispatched until the four open
questions were settled. They were, by [a design review of this RFC](../handoffs/126-password-hashing-must-not-block-the-runtime/design-review-2026-09-30.md)
on 2026-09-30, and this RFC is amended on it.
**Security review.** Required
**Independent design review.** [Design review 2026-09-30](../handoffs/126-password-hashing-must-not-block-the-runtime/design-review-2026-09-30.md) by the implementation role — which also **found** the problem, while reviewing RFC 123, a closer involvement disclosed in the review itself. Verdict **accept with the changes named**; all are folded in below. The problem was found by the [RFC 123 design review](../handoffs/123-authenticating-a-client-costs-the-caller/design-review-2026-09-30.md).
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

**And ~34 ms is the floor, not the cost.** `authn::mfa::match_recovery_code`
checks a guess against **every** stored recovery code in a loop, and
`RECOVERY_CODE_COUNT = 8` — so a wrong recovery-code guess costs up to
**8 × ~34 ms ≈ 270 ms** of synchronous blocking in one request. That is the
worst single-request blocking cost in this codebase, and the RFC's own summary
understated it eightfold until its design review measured against the code.

On the deployment shape this project describes — self-hosted, single-tenant,
plausibly one to four cores — a handful of simultaneous sign-ins or token
requests occupies every worker for 34 ms each, and one wrong recovery-code guess
occupies one for a quarter of a second. **Nothing else the service is
doing runs during that time**: not another user's sign-in, not a health probe,
not an unrelated OIDC discovery request.

## Why a rate limit does not close it

RFC 123 adds rate limits to the two endpoints that lacked them. A limiter bounds
**requests per window**. This is **requests in flight at once**, and the two are
not the same: a burst well inside any per-window budget still lands on every
worker simultaneously. `/oauth2/token` has had a limiter all along and is
exposed to this today.

## The correction that makes the fix smaller

The RFC 123 design review reported that `spawn_blocking` appears nowhere in the
codebase. It does — but **twice, not seven times, and only in one file**:
`sui-id-store/src/backend.rs:146` and `:159`, both wrapping SQLite work.

**This RFC's first draft said `authn/hibp.rs` uses it too. That was wrong**, and
it was the architect's error: a `grep` for the *word* returns hits in `hibp.rs`,
but they are doc comments — one describing a design **RFC 070 removed** when it
replaced a synchronous client with an async one, the other advising a
hypothetical future implementation. Counting grep hits instead of reading them is
the same mistake the correction was written to fix. **The load-bearing claim — that
`verify_password` and `hash_password` are never wrapped — is correct** and was
verified.

So this is not a new idiom. It is **the idiom this project already applies to
database reads, withheld from the one operation that costs a thousand times
more.** Microsecond SQLite reads are moved off the worker threads; 34 ms of
Argon2 is not.

## Decision

**D1 — Argon2 does not run on a runtime worker thread.** Every request-path
`verify_password` and `hash_password` executes where blocking is expected.

**D2 — The boundary is in one place, and it is achievable.** The wrapping lives
at the password module's own API, not at each of the **24** call sites, so a new
caller cannot reintroduce the defect by forgetting.

**Measured, not assumed: 23 of the 24 sites are already inside `async fn`** and
need only an added `.await`. **One is not, and the obstacle is not the one this
RFC first guessed.** It anticipated an ownership problem — a caller needing a
borrowed value across the boundary. The real obstacle at `authn/mfa.rs:363` is a
**control-flow shape**: `hashes.iter().position(|h| verify_password(...).is_ok())`
passes a *synchronous* closure to `Iterator::position`, which cannot host an
`.await` however ownership is arranged. It is replaced by an explicit loop — a
small mechanical rewrite, named here because D2's claim is only true if that site
is converted too. Keeping a synchronous wrapper for it alone would quietly
reintroduce exactly what D2 exists to prevent.

**D3 — The dummy verifications keep their timing property, and D2 is what makes
that automatic.** `authn/session.rs`'s `DUMMY_PHC` calls and the ones RFC 123
added to `authenticate_client` exist to equalise timing. Moving hashing off the
worker threads must not make a real verification and a dummy one distinguishable
again.

**This holds by construction if — and only if — D2's boundary sits at the API.**
Every dummy and every real check calls the same `verify_password`, so one wrapped
function makes both pay the identical thread-hop cost. Placing the wrap at each
call site instead would turn D3 into a second property needing independent
verification at all 24 sites. **That is a second reason for D2's placement, and a
stronger one than "a caller cannot forget".**

The test asserts that the hop's cost lands on **both** branches, not that it is
zero: same-order medians over many samples, under
`#[tokio::test(flavor = "multi_thread", worker_threads = 1)]` so the result is
deterministic rather than machine-dependent. Asserting equality would be flaky
and would be asserting the wrong thing.

**D4 — Concurrency is bounded by a semaphore, and a caller that meets the bound
waits.** Moving the work off the worker threads converts "every worker stalls"
into "the pool saturates", which is better and not free: at 64 MiB per concurrent
hash, Tokio's 512-thread default would be 32 GiB.

**The instrument is a `tokio::sync::Semaphore`, not a thread-count cap**, because
what needs bounding is concurrent 64 MiB allocations, not threads. It lives
inside `password.rs`, invisible to all 24 callers, which is D2's boundary again.
Derived rather than picked: `(available_parallelism() * 2).min(16)` — 2–8 permits
on the one-to-four-core shape this RFC describes, 128–512 MiB peak. The ceiling is
retunable by `@nabbisen` and the implementer; the derivation is the part that must
be stated.

**A caller that meets the bound queues, and that is a decision.**
`Semaphore::acquire().await` does not occupy a worker thread while pending, so
queuing does not reintroduce the defect. Rejecting instead would turn a burst of
**legitimate** concurrent sign-ins into visible failures, which is worse for
availability than a slightly slower one. A bounded wait is a reasonable later
refinement; it is not required for closure.

**D5 — The evidence is a demonstration, not an assertion.** A test issues
concurrent authenticating requests and shows an unrelated request served during
the burst — failing before the change and passing after. Until that exists this
RFC has not been shown to do anything.

**D6 — The recovery-code loops stay sequential.** `mfa.rs`'s enrollment and
regeneration hash eight codes in a loop. Once `hash_password` is async it would be
possible to run them concurrently; **do not.** It would demand eight permits at
once from D4's bound for one enrollment, competing with unrelated sign-ins, to
save 270 ms on an operation nobody has asked to be faster.

## Scope

Not a change to the Argon2 parameters. 64 MiB and two passes are a deliberate
cost chosen against offline cracking, and lowering them to make this problem
smaller would trade a credential-strength property for an availability one. That
trade is not proposed here and should not be made incidentally.
