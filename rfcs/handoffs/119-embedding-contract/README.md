# The embedding contract — what a host must promise

**RFC.** [RFC 119](../../accepted/119-embedding-contract.md), **Accepted 2026-11-01**.
**Author.** High-capability model, requirements-architect role.
**Status of this document.** The specification the RFC's decisions point at.
**Updated 2026-11-25**: this header and *What is still missing* had not followed
the RFC's acceptance; H3's failure mode is corrected (RFC 119, *Amended*).
**The table below is still a first draft in one sense**: it predates the
application of D4, which moves a silent behaviour into the module rather than
leaving it to the host, and all eight are silent. D7's enumeration decides
which survive as host obligations.

## What this handoff is for

RFC 119 decides *that* there is a contract and what shape it has. This document
is where the obligations are written, one at a time, each with its failure mode
and the conformance test that would catch a breach. It is the deliverable of
D7's enumeration.

## The obligations, first draft

Numbered `H` for host. **Each states what breaks.** An obligation with no
stated consequence gets skipped by the reader who most needs it.

| # | The host must | What breaks if it does not | Silent? |
|---|---|---|---|
| **H1** | Rotate its session identifier at the moment authentication succeeds | Session fixation against an identity provider: an attacker who plants a session identifier before sign-in holds the authenticated session after it | **yes** |
| **H2** | Enforce CSRF on every state-changing route it exposes to the module | Every account-mutating operation becomes cross-site reachable. sui-id's own protection is in `crates/sui-id/src/http/csrf.rs` and does **not** come with the module | **yes** |
| **H3** | Give the module the true client address, or declare that it cannot | Per-address rate limits on sign-in, token, setup, forgot-password, step-up and introspection either share one bucket — one client throttles every user — or are bypassed, and log lines name the wrong client. sui-id reads `X-Forwarded-For` **only** when proxies are configured (`ClientIp`, `http/handlers.rs`). **Corrected 2026-11-25**: this cell said every audit record's actor would be a guess; measured, the audit actor is the authenticated user's id, and the client address is not in audit rows | **yes** |
| **H4** | Set cookie attributes at least as strict as the module asks (`HttpOnly` where sui-id sets it, `SameSite`, `Secure` under TLS) | Session theft by script or by cross-site request | **yes** |
| **H5** | Provide a clock that does not run backwards and is roughly correct | Lockout backoff, token expiry and audit ordering all mis-evaluate. The backoff schedule reaches 24 hours; a skewed clock either frees a locked account or strands a legitimate one | **yes** |
| **H6** | Not retry a failed authentication call, and not deduplicate one | A retry inflates the failure count and locks a user out; deduplication suppresses the lockout. See RFC 119 open question 1 — this may be the obligation that cannot be made safe | **yes** |
| **H7** | Preserve the module's response as given, including status and timing, for authentication outcomes | sui-id returns a uniform failure deliberately. A host that maps outcomes to distinct statuses restores user enumeration | **yes** |
| **H8** | Not wrap or share the module's database transaction | RFC 094's Class-A guarantee — the audit row in the same transaction as its effect — silently stops holding. RFC 119 D3 makes this non-negotiable | **yes** |

**Every one of them is silent.** That is the finding, not a coincidence: an
authentication module's obligations are exactly the ones whose breach produces
no error. It is why D4 exists, and it is the argument for moving more into the
module than a minimal boundary would suggest.

## What is still missing from this draft

1. **The enumeration is incomplete** (RFC 119 D7). Three behaviours were
   measured; `crates/sui-id/src/http/` is 11,347 lines and has not been walked.
   Until it has, this table is a sample, not a contract.
2. **No conformance test exists.** D1 says an embedding states which contract
   version it satisfies; nothing yet checks that it does. The natural shape is
   a test suite the embedder runs, plus sui-id's own service crate as the first
   subject (D5).
3. ~~The three open questions~~ **Answered 2026-11-01** (RFC 119, *Answers*).
   What the host gets when it declares under H3 that it cannot supply the
   address is **not yet defined**: a question from a prospective host,
   2026-11-25, and a clause for D7.

## Order

1. Walk `crates/sui-id/src/http/` and finish D7's enumeration; assign each item
   one of D4's three outcomes.
2. Complete this table from that enumeration.
3. Independent design review, attacking the obligations. (The three open
   questions were settled 2026-11-01.)
4. The RFC is amended on the review.
5. Only then is the split's RFC written. It has no number: RFC 120 and RFC
   121, which earlier text here and in the RFC named, were reassigned to
   unrelated security fixes.

**Nothing here is dispatched for implementation.** This RFC ships no code.
