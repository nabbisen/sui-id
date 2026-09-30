# A verification failure is not a pass

**RFC.** [RFC 121](../../accepted/121-a-verification-failure-is-not-a-pass.md), **Proposed**.
**Author.** High-capability model, requirements-architect role.
**Baseline.** `4ebf0f7` or later.

## The defect, as measured

`admin/audit.rs:37-44` at `e1a251d`: `verify_chain_tail(…).await` has its error
replaced by a `ChainVerifyReport` whose `broken_at_seq` is `None`, and
`chain_ok` is then `broken_at_seq.is_none()`. An error becomes a green banner
and is not logged. `settings.rs:291` maps the same error to an error page.
Confirm both at the baseline before changing anything, and say whether any other
caller of `verify_chain_tail` exists.

## What to build

RFC 121 D1–D5 are the specification. The shape that matters: **one function
returns the three outcomes, and every surface renders that**, rather than each
surface deciding what an error means. Where the chain state is rendered in
`sui-id-web`, the third outcome needs words an operator can act on — "the chain
could not be verified" is not the same as "the chain is broken", and an operator
must not read the first as the second.

**D3 is settled and no longer yours to decide** — the review resolved it: a
`tracing::error!` is the primary record, an audit row is best-effort secondary.
The reasoning is in D3 and it is circular-dependency, not preference.

**What is yours to decide:** where the one shared answer lives, given that its
three consumers are two HTTP handlers and a startup path in a different crate
from `verify_chain_tail` itself. Name the placement and why the next consumer
cannot bypass it.

## Evidence

- A test that **injects a verification error** and asserts that no surface
  reports an intact chain, and that the two viewers agree. Show it failing at
  the baseline.
- A test per outcome for the shared function.
- Mutations: collapse the third outcome into each of the other two in turn, and
  name the test that catches it.
- fmt, both clippy scopes, the workspace test count before and after, MSRV, and
  every doc gate.

## What to return

A review-request package under `.git-exclude/review-requests/`, in the usual
form, with the before/after for the injected-error test and your D3 answer with
its reasoning.
