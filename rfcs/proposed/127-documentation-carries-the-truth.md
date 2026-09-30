# RFC 127 — Documentation carries the truth; a test enforces it

**Status.** Proposed
**Security review.** Not required — reason: this RFC moves facts into documentation and changes no behaviour. Approval of that classification is `@nabbisen`'s.
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** No security-relevant fact about this system is discoverable only by reading source or tests: each such fact has a stated home in `docs/`, the test that enforces it cites that home, and the document names the test that keeps it true.
**Tracks.** Documentation authority. Raised by `@nabbisen`, 2026-09-30: *"The case when truth exists only in code should be rare. I mean documentation should help instead of code. We have to maintain and improve documentation to achieve it."*
**Touches.** `docs/src/reference/`, `docs/src/SUMMARY.md`, and the two enumeration tests that currently hold the facts.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/127-documentation-carries-the-truth/README.md`](../handoffs/127-documentation-carries-the-truth/README.md)

## Summary

Two facts this programme established at real cost now live **only as constants in
test files**:

- **Which routes answer without an authenticated caller** — 107 method/path
  pairs, 72 requiring an actor, 35 not, each with a reason. In
  `crates/sui-id/tests/e2e/r120_routes.rs`.
- **Which surfaces show a secret once** — six, each with the transport and the
  headers that protect it. In `crates/sui-id/tests/e2e/r122_routes.rs`.

Both were written as tests because a test can *enforce* a fact. Neither was
written into `docs/`, so the only way to learn either is to read the test — and
an operator assessing this system, or an architect designing against it, reads
documentation.

## Why this is a defect and not a preference

**It is the cause of the expensive part of the last two weeks.** The consent
bypass, the inverted setup guard, the unverified audit chain and the six
one-time-secret surfaces were each found by someone reading thousands of lines
of source, because no document stated what was supposed to be true. RFC 119's
review walked 11,347 lines of `http/` to produce an enumeration that should
have been a page.

**And it is the shape RFC 098 already named**, from the other direction:
RFC 098 corrected documents that claimed more than the code did. This is the
same fault inverted — the code is right and no document says so, which is
equally a failure of documentary authority, and equally something a reader
cannot detect.

## Decision

**D1 — A security-relevant fact has a home in `docs/`.** The two enumerations
above get one. The document is the statement; it is not a duplicate of the test,
because a reader is not expected to read the test.

**D2 — The test cites the document, and the document names the test.** Each
enumeration test's hand-list carries the path of the page that states the same
fact; the page names the test that keeps it true. A fact stated in one and not
the other is drift, and the pairing is what makes the drift visible.

**D3 — The pairing is checked.** G15 owns documentary authority; this becomes a
condition it can check rather than a convention someone maintains. If a cheap
check is not available, the RFC says so plainly rather than claiming one.

**D4 — This is the standing rule, not a one-off repair.** A future enumeration —
the write-command inventory, the audit event vocabulary, whatever RFC 126 and
RFC 124 produce — is written into `docs/` when it is created, not retrofitted.
The two repairs here are the first application, not the scope.

## What this RFC does not do

It does not document *how* anything is implemented. The route table states which
routes answer without an authenticated caller and why that is intended; it does
not describe the extractor that enforces it. The distinction is the one that
matters throughout this programme: **a document states what the system promises;
the code is how it keeps the promise.**
