# RFC 127 — Documentation carries the truth; a test enforces it

**Status.** Implemented (v0.79.0)
**Closure reviewed on.** 2026-10-02
**Closure approved by.** `@nabbisen` (accountable owner), 2026-10-02: "Both approved." — batch 3 and RFC 131's fork together. The closure review was performed by **the architect, which wrote this RFC**, and is therefore **not** independent of it; `@nabbisen` is the approver, which is what RFC 000 requires when no independent role exists. The implementation role verified the review's named tests separately and returned no findings; that corroboration is recorded beside the review and is not approval.
**Closure evidence.** [Closure review batch 3, 2026-10-02](../handoffs/105-audit-note-escaping/closure-review-batch-3-2026-10-02.md), with [independent verification](../handoffs/105-audit-note-escaping/closure-verification-batch-3-2026-10-02.md)
**Accepted on.** 2026-09-30
**Approved by.** `@nabbisen`, 2026-09-30: "Accepted." 
**Security review.** Not required — reason approved by `@nabbisen`, 2026-09-30: this RFC moves facts into documentation and changes no behaviour.
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

**D5 — One page, in the product reference.** `docs/src/reference/security-surfaces.md`,
RFC 098's **D1** layer, reachable from `SUMMARY.md`. One page and not two, because
both tables answer one question — *what can be reached, and what protects it* —
and a reader who has to check two places will check one. It sits beside
`audit-events.md`, which is the same kind of material for the same reader.

**D6 — It is public, and that is a decision rather than an oversight.** The route
set is derivable from `router.rs` by anyone who clones the repository, and
`docs/threat-model.md` already states this system's posture publicly. Withholding
the table protects nobody and costs the operator who needs it to assess their own
exposure.

**D7 — The pairing is enforced by the enumeration tests, not by a new gate.**
The tests that derive these sets already exist and already run. Each is extended
to assert that **the documented table matches the set it derives** — so the
document itself becomes the checked artefact, not merely cross-cited. This adds
no gate, no lane, and no amendment to a Done RFC, which a new G15 condition would
have required.

**D8 — The page is written, not generated.** A generator can produce the list; it
cannot produce the **reason** each route is intended to answer without an actor,
and the reasons are what a reader comes for. So the prose is written and the list
within it is held to the router by D7's test. `ci.yml` is generated because
nothing about it is a judgement; this page is the opposite case.

## What this RFC does not do

It does not document *how* anything is implemented. The route table states which
routes answer without an authenticated caller and why that is intended; it does
not describe the extractor that enforces it. The distinction is the one that
matters throughout this programme: **a document states what the system promises;
the code is how it keeps the promise.**
