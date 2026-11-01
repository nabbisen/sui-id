# RFC 119 — independent design review request

**RFC.** [RFC 119 — What a host must promise before sui-id can be embedded in it](../../proposed/119-embedding-contract.md). **Proposed**, with `@nabbisen`'s acceptance recorded 2026-09-26 and the status held until this review exists.
**Reviewer.** **`@nabbisen`, or an external architect he engages.**
**Corrected 2026-11-01, and this is why the RFC sat for 36 days.** This
request originally named the *mid-capability model, implementation role* as
the reviewer, routed on `ROADMAP.md` §S1 as it stood on 2026-08-26. Five days
after the request was written, **§S1a (ruled 2026-10-01)** settled the routing
differently: its table reads *"Design review | `@nabbisen`, or an external
architect he engages. **Not the dev team.**"* So from 2026-10-01 this request
named a reviewer who may not perform it, and there was nobody it could
legitimately be handed to — which is why nothing happened to it.
**Route.** `ROADMAP.md` §S1a (2026-10-01), superseding §S1 (2026-08-26).

## The quickest route, and it is not the architect's to take

**Route 1 below may already be satisfied, and RFC 141 is the precedent.**
That RFC's `Independent design review` field cites a record whose own first
section states that an approval, under §S1a's table, **is** the design review
rather than a substitute for one. §S1a places the role with the approver, so
an approval can discharge the field.

**The difficulty is the date.** RFC 119's approval is recorded 2026-09-26 —
**five days before §S1a existed.** The rule placing the review role there was
not yet written, so nobody filling that role at the time could have known they
were. **The architect is not writing that record unprompted.** Doing so would
assign a role backwards in time on the strength of the architect's own reading,
and this project has a standing rule against recording the architect's reading
as a ruling. The question below is therefore genuinely open, not rhetorical.

**So one of three, and it is his to pick:**

1. **His 2026-09-26 acceptance was the §S1a design review** — say so, and the
   architect writes the record citing his words, RFC 141's shape exactly. The
   chain unblocks the same day with no new work.
2. **He reviews it now**, against the attack list below.
3. **He engages an external architect** — §S1a makes that his option, not a
   requirement, and §1 below is written for that reader.

**Until one of those happens, RFC 119 cannot become Accepted, and the
store-boundary RFC may not be written at all** — RFC 119's own header makes
itself that prerequisite. One sentence in reply moves a chain that has not
moved in 36 days; nothing else in it is waiting on work.

## Which parts suit which reviewer

**§1's measurement extension is written for an external architect**, and is
not a reasonable ask of the owner: it wants the remaining rows of a
code-measurement counted. If route 2 is chosen, **the review that matters from
him is §2 onward** — whether the contract's *shape* is right, which is a
judgment call rather than a counting exercise, and the half this project's
philosophy makes decisive: a contract an embedder misreads is worse than no
contract.
**Why this review is the thing standing between an owner decision and a status
change.** G11 refuses an Accepted RFC whose security review is Required and
which cites no review. That guard exists because RFC 105 shipped with none.
**Baseline.** `0d78614` or later.
**Scope.** Read-only. Change no code and no RFC text. Report findings.

## What this RFC claims, and what to attack

That sui-id can be split into an embeddable authentication module and a
service, **but only against a written contract**, because security-relevant
behaviour lives in the part that would stay behind. It ships no code. The
deliverable is the contract, and a contract that is wrong is worse than none,
because an embedder will rely on it.

## 1. The measurement — confirm, refute, and above all **extend**

The RFC rests on three measured rows. **The most valuable thing you can return
is the rest of them.**

1. `axum` appears outside `crates/sui-id/` exactly once, a doc comment at
   `sui-id-core/src/authn/hibp.rs:241`; `src/http/` is 11,347 of that crate's
   14,878 lines.
2. **CSRF** is entirely in `http/csrf.rs` and comes to an embedder as nothing.
3. The **audit actor's client address**, with its deliberate policy of reading
   `X-Forwarded-For` only when proxies are configured
   (`http/handlers.rs:332-361`), likewise.
4. **Walk `crates/sui-id/src/http/` and finish D7's enumeration.** Every
   security-relevant behaviour, with `file:line`, and for each one: does it
   move into the module, become a host obligation, or block embedding (D4's
   three outcomes)? The handoff's table is a **sample and says so**; this is
   what turns it into a contract. Expect to find things neither the architect
   nor `@nabbisen` has thought of — rate limiting, step-up, session limits,
   the setup flow, metrics, request IDs, and the UI's own assumptions are
   where I would start, and that list is not exhaustive either.

## 2. The obligations — attack each one

For **H1–H8** in the handoff: is the obligation **correct**, is its stated
failure mode **real**, and is it **checkable by a conformance test**? An
obligation nobody can test is a wish. Name the test for each, or say it cannot
be tested and why.

Then the three the RFC marks as unresolved. **These are the review's centre:**

5. **Open question 1 — lockout counting (H6).** D4 says nothing whose failure
   is silent is delegated, but the module cannot see a host's retries or
   deduplication. Is there **any** design that makes this safe — an idempotency
   token, a caller-supplied attempt identifier, the module owning the transport
   for this one call? Or is H6 genuinely unsatisfiable, in which case the RFC
   must say so and say what an embedder loses. **"It cannot be made safe" is a
   finding, not a failure.**
6. **Open question 2 — what an embedded audit trail proves.** The hash chain is
   tamper-evident within its trust boundary; embedded, the host is inside it.
   State the narrower claim precisely enough to put in `docs/`, and say what
   `@nabbisen` would have to stop claiming.
7. **Open question 3 — an embedding without the UI.** `sui-id-web` is leptos. A
   host with its own UI wants the module without it. What happens to every
   decision that promises the user a message — RFC 118's "the user is told",
   RFC 115's, RFC 103's? Does the contract owe the embedder the *message*, the
   *string catalogue*, or only a typed outcome they must render themselves?

## 3. The decisions

8. **D3 (the module keeps its own transaction) — is refusing right?** It
   forecloses a legitimate use: a host that wants one transaction across their
   tables and sui-id's. Make the case *for* allowing it, then say whether
   RFC 094's Class-A guarantee can survive it in any form.
9. **D5 (sui-id's own service crate is the first embedder) — is it
   achievable?** Look for what `crates/sui-id` does today that no published
   interface would expose, and say what it would cost. If D5 is unachievable,
   the contract loses its only continuous test and the RFC needs to know now.
10. **D4's rule — does it swallow the module?** If nothing whose failure is
    silent may be delegated, and every obligation found so far fails silently,
    does "embeddable module" collapse into "the whole service"? If so, say
    where the honest line is.

## 4. Anything else

11. **Threats the contract introduces**, including an embedder who satisfies
    every obligation on paper and is still insecure, and a *malicious* host —
    state plainly whether the host is inside the trust boundary, because the
    RFC does not.
12. **Anything better built another way**, and anything the RFC does not
    mention and should.

## What to return

A review-request package under `.git-exclude/review-requests/`, containing:
- the confirmation or refutation of items 1–3, with `file:line`;
- **the completed D7 enumeration** (item 4), each entry assigned one of D4's
  three outcomes;
- a verdict per obligation H1–H8, each with its conformance test or the reason
  there can be none;
- your answers to items 5–12;
- findings ranked blocker / high / medium / low;
- a recommendation: accept as written, accept with the changes you name, or do
  not accept.

**"Do not accept" is a legitimate outcome.** So is "this contract cannot be
written as scoped" — that would be the most useful finding of all, and it would
be better learned now than after RFC 120 moves 269 call sites.
