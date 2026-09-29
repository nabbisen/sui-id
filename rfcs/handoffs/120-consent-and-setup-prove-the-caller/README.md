# Consent and the setup wizard must prove who is asking

**RFC.** RFC 120, **Proposed**. Authorized as an urgent security fix by
`@nabbisen`, 2026-09-26: fix quietly, then consider disclosure.
**Implementer.** Mid-capability model — the role that found both defects.
**Baseline.** `e1a251d` or later.

## Why this handoff was staged outside git until now

`@nabbisen` ruled that implementation handoffs live under `rfcs/handoffs/` —
"ever, now and in the future". This one now does. It was staged outside git
while the fix was being built, for one reason: **the repository is public**, and
a tracked handoff describing an unfixed authentication defect publishes it. It
moved here in the same commit as the fix, which is the first moment the
description is safe. That was a timing decision about a public repository, not
an exception to where handoffs live.

## What to build

Read RFC 120's D1–D7. They are the specification. Two routes, in this order.

### 1. The consent POST (`handlers/oidc.rs`)

The handler must resolve the session the way every other authenticated route
does, take the subject **and** the authentication methods from that session, and
refuse a request that has none. Whatever the flow still needs to carry between
the consent screen and the POST must be integrity-protected and bound to that
session, and must not carry identity at all. The deny path validates its
redirect target against the client's registered URIs, using the same routine the
approve path already uses, and percent-encodes what it appends.

**Decide and state, with your reasoning:** whether the remaining parameters
belong in a signed cookie, in a server-side row keyed by an opaque id, or are
re-derived from the request. A server-side row is the shape that cannot be
forged at all; a signed cookie is less change. **Say which you chose and why** —
this is the design decision of the stage, and the architect will review the
choice, not just the code.

### 2. The setup wizard's post-initialization routes (`handlers/setup.rs`)

Both handlers must require an authorized administrator, enforce CSRF, take a
rate limit, and write an audit row; the first-run case is the **named
exception**, expressed so that an inverted comparison cannot open the route.
`hibp_mode` is a security-posture setting: its change is audited with the old
and new value.

Prefer moving these routes under whatever router grouping already carries the
admin requirement, rather than adding a check inside each handler, so the next
handler added there inherits it.

### 3. The enumeration (D6)

A test that asserts **the set of routes answering without an authenticated
actor**. Build it from the router, not from a hand-written list, so it cannot
drift. Adding a public route then requires changing the expected set, which is
visible in review. Name in the package every route the test finds, so the
architect and `@nabbisen` can read the list and disagree with it.

## Evidence

- **A test per defect that fails on `e1a251d` and passes after.** Show both
  results. This is D7 and it is not optional: neither defect was executed when
  found, so a test that does not demonstrate the hole demonstrates nothing.
- The mutation discipline of the previous stages: break each new guard in turn
  and name the test that catches it.
- fmt, both clippy scopes, `cargo test --workspace` count before and after,
  MSRV, and every doc gate.
- **No exploit text, no reproduction script, and no attack narrative in any file
  that will be committed.** State invariants and what the tests assert. The
  review package itself may describe what you did — it stays in `.git-exclude/`.

## Triage list, not in this RFC

From the RFC 119 review, unverified by the architect. Assess each, and for each
say **confirmed / not a defect / needs its own RFC**, with `file:line`. Do not
fix them here.

1. The revoked-access-token deny-list is consulted only at `/userinfo`, so a
   revoked token may still be accepted elsewhere.
2. One-time secrets served without `no-store` — client secret, rotated secret,
   TOTP secret and QR, recovery codes — and a **rotated client secret carried in
   a redirect query string**.
3. Forgot-password timing is not uniform: a real account sends mail inline, an
   unknown address returns early, so the stated enumeration promise does not
   hold.
4. `/oauth2/introspect`, `/oauth2/revoke`, `webauthn_auth_start` and dynamic
   registration are unthrottled, and client-secret verification runs Argon2 on
   every call.
5. The audit viewer treats a chain-verification **error** as "chain OK".
6. The federation link/takeover guard fails open on a lookup error.
7. Factor enrollment's proof (fresh step-up or current password) is enforced
   only in the handler, not by the enrollment functions.

Also record, without fixing: role change has no step-up or confirm screen
(it **does** require an authenticated admin and CSRF — the review's phrasing was
wrong); and `/authorize`'s session resolution skips the idle timeout and the
disabled/deleted check, compensated today by the code exchange's user-active
re-check.

## What to return

A review-request package under `.git-exclude/review-requests/`, in the usual
form, plus: the before/after test results for D7, your D1–D4 design choice with
its reasoning, the route enumeration, and the triage verdicts.
