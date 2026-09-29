# The findings this RFC answers

**Source.** The independent design review of [RFC 119](../../proposed/119-embedding-contract.md),
2026-09-26, by the implementation role — which authored neither RFC 119 nor
RFC 120. Its method was to read every line of `crates/sui-id/src/http/` for
security-relevant behaviour, which is how it met these two.
**Confirmed by.** The architect, independently, line by line, before RFC 120 was
written.
**Status.** **Both are fixed** — `cd4d137`, 2026-09-29. This record exists
because RFC 000 requires an Accepted RFC's independent design review to be
durable, repository-relative evidence rather than a claim.

**Why this file is an extract and not the review.** The review returned **88
enumerated behaviours**, of which several are **still open** and each will get
its own RFC. Publishing them before they are fixed is what
[`.github/SECURITY.md`](../../../.github/SECURITY.md) exists to prevent, so the
full review stays outside this repository until its remaining findings are
closed. What is here is the part RFC 120 answers, and that part is fixed.

## Finding 1 — the consent answer derived identity from a value the caller could write

`POST /oauth2/consent` took the subject of the decision, **and the
authentication methods recorded into the resulting authorization code**, from
`sui_id_consent` — a cookie the server set as plain JSON, unsigned, and not
bound to any session. The handler performed no session lookup. Its CSRF check
compared a cookie against a form field, both supplied by the same request.

**Why the methods matter as much as the subject:** they travel into the issued
token's claims, so a caller choosing them described its own authentication to a
relying party.

**Fixed by** RFC 120 D1–D4: the handler takes the session extractor, the subject
and the methods come from that session, and what the cookie still carries is
integrity-protected, bound to that session, and parsed only after it verifies.

## Finding 2 — two setup steps had an inverted guard

`POST /setup/lang` and `POST /setup/hibp` redirected when the system was **not**
initialized and proceeded when it was — that is, on every running instance —
with no authenticated caller, no CSRF, no rate limit and no audit row. One of
them sets `hibp_mode`, so the breach-password check could be disabled
server-wide, silently.

**Fixed by** RFC 120 D5: both steps sit behind the administrator extractor as a
group and in each handler, take the rate limit and CSRF, and write through a
command that changes the value and appends the audit row in one transaction.
The initialization comparison was deleted rather than corrected — the first
administrator is signed in when they are created, so a positively-stated
requirement needs no first-run exemption and the inverted form has nowhere to
live.

## What made both reachable, and what now catches that class

Nothing enumerated the routes that answer without an authenticated caller. RFC
120 D6 adds a test that derives the set from the router — 107 (method, path)
pairs, 72 requiring an actor, 35 not, each with a stated reason — so adding a
public route is a reviewed change. **On the commit before the fix that test
fails and names exactly the five entries these two findings covered.**

## On the reviewer's conduct

It disclosed that it had started a local development instance and begun a script
that would have exercised the first finding against it, stopped because that is
an exploit rather than a review, and stated that it could not be certain how far
the script had run. Stopping there was right, and disclosing the uncertainty
rather than rounding it down was righter. It also meant both findings rested on
**reading** until RFC 120's tests executed them.
