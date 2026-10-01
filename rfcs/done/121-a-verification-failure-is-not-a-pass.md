# RFC 121 — A verification failure is not a pass

**Status.** Implemented (v0.79.0)
**Closure reviewed on.** 2026-10-02
**Closure approved by.** `@nabbisen` (accountable owner), 2026-10-02: "Batch 2 is approved." The closure review was performed by **the architect, which wrote this RFC**, and is therefore **not** independent of it — `@nabbisen` is the approver, which is what RFC 000 requires when no independent role exists. The implementation role measured the review's claims separately, and corrected two of them; that corroboration is recorded beside the review and is not approval.
**Closure evidence.** [Closure review batch 2, 2026-10-02](../handoffs/120-consent-and-setup-prove-the-caller/closure-review-batch-2-2026-10-02.md), with [independent verification](../handoffs/120-consent-and-setup-prove-the-caller/closure-verification-batch-2-2026-10-02.md)
**Accepted on.** 2026-09-30
**Approved by.** `@nabbisen`, 2026-09-30: "RFC 121 is accepted." Accepted on the
**amended** text — its independent design review returned "accept with changes"
and, outside this RFC's scope, the blocker that became RFC 125 and shipped
first. **This acceptance is an ordinary lifecycle act, not a ratification after
the fact:** unlike RFCs 120 and 125, nothing here was exploitable, so nothing
justified landing ahead of acceptance.
**Security review.** Required
**Independent design review.** [Design review 2026-09-29](../handoffs/121-a-verification-failure-is-not-a-pass/design-review-2026-09-29.md) by the implementation role. Its verdict was **accept with changes**, and it also returned a **blocker outside this RFC's scope** — that `verify_chain_tail` did not verify linkage at all — which became [RFC 125](../done/125-the-chain-must-be-verified-as-a-chain.md) and shipped first. Every change it named is folded in below.
**Amended 2026-09-30 on that review, and on RFC 125.** The amendment is material: this RFC was written believing the check it displays was sound.
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** No surface reports the audit chain as intact when verification did not complete: a failure to verify is distinguishable from a verification that found nothing wrong, on every surface that shows either, and the two surfaces that show it agree. A failure is recorded where an operator will meet it.
**Tracks.** Audit integrity. Found by the RFC 119 design review, 2026-09-26; confirmed by the implementation role in the RFC 120 triage.
**Touches.** `crates/sui-id/src/http/handlers/admin/audit.rs`, `crates/sui-id/src/http/handlers/settings.rs`, `crates/sui-id/src/runtime/startup.rs`, `crates/sui-id-web/`, and the tests.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/121-a-verification-failure-is-not-a-pass/README.md`](../handoffs/121-a-verification-failure-is-not-a-pass/README.md)

## Summary

The audit viewer calls `verify_chain_tail`, and on an **error** substitutes a
report meaning "nothing wrong", from which it computes a green banner. So the
one screen an operator would look at to ask whether the audit log has been
tampered with answers "no" both when the answer is no and when the question
could not be asked.

This is the defect that hides other defects, which is why it goes first.

## Why it is worse than one bad banner

Three facts compound:

1. **The two viewers disagree.** The settings logs page maps the same error to
   an error page; the audit page maps it to success. An operator who checks the
   other screen gets a different answer to the same question.
2. **Startup does not close the gap.** It warns that verification *could not
   run*; it never reports that verification *failed*.
3. **RFC 094's entire argument** for the hash chain is that tampering is
   evident. Evidence that reports itself intact when it could not be read is
   not evidence, and this project has said so before, about a different subject:
   RFC 102's rule is that a sign-in that cannot be audited does not succeed.

## Decision

**D1 — Three outcomes, never two, and each states its window.** Verification
yields *verified intact*, *verified and broken at N*, or *could not verify,
because E*. No surface collapses the third into either of the first two, and no
default value stands in for a result that was not obtained.

**"Intact" also states what it covers.** `verify_chain_tail(db, limit)` walks the
most recent `limit` rows — 5,000 at startup, 500 on the audit page, 100 on the
settings page. Three surfaces answer the same question at three silently
different scopes today. Each states its own, because on a log longer than its
limit, "intact" stopped meaning "the whole history" and nothing said so.

**`legacy_unhashed` is shown alongside the verdict, on every outcome.** After
RFC 125 stage 2 a nonzero value can only mean genuine pre-v0.17.0 rows, so the
number became worth showing precisely when it stopped being a signal of tampering
— and an operator on a deployment newer than v0.17.0 should see exactly zero, and
can then notice a change.

**D2 — The surfaces agree, in severity as well as in words.** Wherever chain
state is shown, the same three outcomes appear with the same words. The present
disagreement between the audit page and the settings logs page is the evidence
that one shared answer is needed rather than two independent mappings.

**And the levels must order correctly.** Startup reports the three outcomes at
`error!` / `info!` / `warn!` respectively, so an operator filtering at `error` —
a common baseline — sees a broken chain but **never sees a chain that could not
be checked at all**. "Could not verify" is not less severe than "broken"; it is
an unknown where a known was expected.

**D3 — A failure to verify is recorded, not only displayed, and the record does
not depend on the thing that failed.** The primary record is one
`tracing::error!` — that channel does not require the store to be healthy. An
audit row is attempted as a **secondary, best-effort** signal only, in the
codebase's existing `let _ = audit::append(...)` shape, never on the critical
path.

**The reason it cannot be primary is circular, and that settles it:** `append`
uses the same connection that just failed to be read, so a store-wide cause makes
the write fail too — and if the cause is the tampering this system exists to
catch, the record of "I could not read myself" would live in the thing that is
the problem.

**D4 — Startup distinguishes the same three, and none of them is fatal.**
"Could not run" and "ran and found a break" are different sentences today only by
accident; they become different by rule, at levels that order correctly (D2).

**Not fatal, as a reasoned decision rather than an unexamined default.** The code
already argues it for a *broken* chain: refusing to start would let an attacker
deny service by corrupting one row. The same applies at least as strongly to
"could not verify", whose overwhelmingly common cause is a transient I/O error or
a lock timeout.

**And the RFC 112 analogy does not hold** — it was worth testing and it fails.
RFC 112's refusal is a **structural precondition**: can this binary operate on
this file at all, deterministic, checked before any write, and every legitimate
database eventually satisfies it. Chain verification is a **best-effort
monitor**; the store operates correctly whether or not it has ever run. Making
the third outcome fatal would harden the *least* dangerous failure shape while a
false "intact" — the dangerous one — went untouched.

**D5 — The test is the mutation.** A test injects a verification error and
asserts that no surface shows an intact chain. The present code passes every test
it has, which is why this defect survived.

**The injection needs no production seam.** A malformed `actor` value written by
ordinary SQL — `UPDATE audit_log SET actor = 'not-a-uuid'` — makes
`verify_chain_tail`'s row mapping fail and surfaces as the same `StoreResult::Err`
any real corruption would take. One malformed byte is a more faithful stand-in
for corruption than dropping a table, and it costs no `#[cfg(test)]` branch.

**D6 — The settings page fails no more than it must.** A chain-verify error
today propagates through one `?` on the whole `logs_get` handler, so it takes
down the **entire** logs tab — the 24-hour login, lockout and password-change
counters too, not just the chain widget. That is the opposite failure from the
audit page's silent pass, and both are wrong. The chain status is one widget and
it fails as one widget.
