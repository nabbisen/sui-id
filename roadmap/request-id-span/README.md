# Request logs keep their request ID across `.await`

**Authorized by.** [`ROADMAP.md`](../../ROADMAP.md) §Non-RFC work packages. Owner
authorization 2026-09-17. **Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later.
**Found by.** R11 Part 1 (`e39e18b`).

## The defect
`crates/sui-id/src/http/request_id.rs` enters the request span with
`let _enter = span.enter();` and holds the guard across `next.run(req).await`. A
synchronous guard does not follow a future across suspension, so events emitted
after the handler resumes can fall outside the span and lose `request_id`. The
R11 test that checked for the span failed in 1 of 3 runs.

## Required
- **Instrument the future.** Use `next.run(req).instrument(span).await`, or the
  equivalent that attaches the span to the future rather than entering it.
- **A deterministic test.** A handler that awaits (a multi-thread runtime, a yield
  before logging) emits an event, and a captured subscriber shows that event
  inside the request span with `request_id`, repeated enough to be meaningful (at
  least 100 iterations). Show the test failing against today's middleware.
- **Keep R11's field.** `login_post`'s explicit `request_id` field stays. Removing
  it is a separate decision.

## Evidence
- The test failing before the change and passing after.
- One captured log line, before and after.
- fmt, both clippy scopes, test count before and after, MSRV 1.95.
