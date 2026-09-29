# RFC 120 — Consent and the setup wizard must prove who is asking

**Status.** Proposed
**Lifecycle note, stated rather than discovered.** The fix landed **while this
RFC is Proposed**, which RFC 000 otherwise prohibits. `@nabbisen` authorized the
fix itself on 2026-09-26 under the feature freeze's urgent-security-fix
exception, and leaving a confirmed authentication defect in a public tree to
complete paperwork would have been the worse choice. **This is a deliberate
deviation, recorded here at the moment it was made** — which is the difference
between it and RFC 105, where the same thing happened and nobody noticed. It
needs `@nabbisen`'s ratification, and the RFC does not move to `accepted/`
until he gives it.
**Independent design review of *this* RFC: none.** The implementation role found
and reported both defects and built the fix; the architect confirmed both
defects and reviewed the fix. Nobody independent reviewed *this design*. It is
recorded as unreviewed judgment rather than labelled a completed review. The
design is "identity comes from the session", which is not novel; the judgment
that carries risk is D3's choice of a signed cookie over server-side state, and
that choice is argued in the review package.
**Security review.** Required
**Independent design review.** Found by the RFC 119 design review, 2026-09-26, by the implementation role — which authored neither RFC 119 nor this one. Both defects were confirmed independently by the architect, line by line, before this RFC was written.
**Design prerequisites.** None. This RFC is an urgent security fix and takes precedence over every scheduled item under the feature freeze's first exception.
**Implementation prerequisites.** None.
**Closure prerequisites.** No request can establish, or act as, an identity it has not authenticated: the consent flow derives the subject and the authentication methods from the session alone, and every state-changing route reachable after first-run initialization requires an authorized actor, CSRF, a rate limit and an audit row. Each has a test that **fails on the code before this RFC** and passes after.
**Tracks.** Authentication integrity. Found by the RFC 119 design review, 2026-09-26.
**Touches.** `crates/sui-id/src/http/handlers/oidc.rs`, `crates/sui-id/src/http/handlers/setup.rs`, `crates/sui-id/src/http/router.rs`, and their tests.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/120-consent-and-setup-prove-the-caller/README.md`](../handoffs/120-consent-and-setup-prove-the-caller/README.md)

## Summary

Two routes act on an identity, or change server-wide security posture, without
establishing who is asking. Both were confirmed by reading the code that runs.
This RFC states the invariant each one violates and what must be true instead.
It deliberately contains **no reproduction and no attack description**: the
repository is public, and this RFC lands in the same commit as the fix.

## Decision

**D1 — The subject of a consent decision comes from the session, and from
nothing else.** `consent_post` currently reads `user_id` from a cookie it also
set, with no session lookup in the function. After this RFC the handler resolves
the session through the same path every other authenticated route uses, and the
subject is that session's user. A request without a valid session is refused,
not redirected to a guess.

**D2 — The authentication methods of a consent decision come from the session
row.** They are currently taken from the same cookie. They flow into the issued
token's claims, so a client that can choose them can describe its own
authentication to a relying party. The session row is the only source.

**D3 — A cookie that carries request parameters carries only request
parameters, and is integrity-protected and bound to its session.** Identity
never travels in it. Whatever remains in it is signed and scoped, so that a
value the server did not write is not accepted as one it did.

**D4 — The deny path validates its redirect target and encodes what it
appends.** It currently redirects to a cookie-supplied URI and concatenates
`state` unescaped. The redirect target is checked against the client's
registered URIs by the same routine the approve path uses, and appended values
are percent-encoded.

**D5 — A route that changes server-wide security posture requires an authorized
actor, CSRF, a rate limit and an audit row — and its first-run exemption is
expressed as an exemption.** `/setup/lang` and `/setup/hibp` invert their guard:
they redirect when the system is *not* initialized and fall through when it is,
which is every live instance. The correct shape states the rule (authorized
admin) and names the narrow first-run exception, so that an inverted comparison
cannot silently open the route. Disabling breach-password checking is a security
posture change and is audited like one.

**D6 — Every route reachable without authentication is enumerated, once, and
the list is checked.** These two escaped because nothing enumerates them. The
enumeration is this RFC's durable output, not just its fix: a test asserts the
set of routes that answer without an authenticated actor, so adding one is a
visible, reviewed change rather than an accident.

**D7 — Each fix carries a test that fails before it.** Neither defect was
executed when found — both were read. A fix whose test would also have passed
on the old code has not been shown to close anything.

## Scope, and what this RFC does not do

The RFC 119 review returned **88 enumerated behaviours**, of which the architect
confirmed these two as live and exploitable and **corrected three others whose
severity was overstated**. Seven further findings are plausible and unverified —
among them a revoked access token that is rejected only at `/userinfo`, one-time
secrets served without `no-store` including a rotated client secret carried in a
redirect query string, non-uniform forgot-password timing, and an audit viewer
that treats a verification error as success. **They are not in this RFC.** They
are triaged in the handoff and each becomes its own RFC or a rejected finding,
because bundling unverified findings with a confirmed fix delays the fix.

## Disclosure

**Authorized by `@nabbisen`, 2026-09-26:** fix quietly, then consider whether an
advisory is necessary, since there is no production use yet. He asked whether an
advisory is needed at all.

**The architect's recommendation: no security advisory; one factual
`CHANGELOG.md` entry in the release that carries this fix.** Measured
2026-09-26: the repository has **no GitHub Releases at all** (only tags), **one
fork, last pushed 2026-07-01**, and `ROADMAP.md` records that sui-id is not in
production and no third-party deployment is known. An advisory's real audience
is automated dependency scanning, which requires a package-registry
publication; that could not be verified from the working environment (the
registry API returned 403 from here) and is the **one fact that would change
this recommendation** — if sui-id is published, an advisory is cheap and should
be filed.

**Something must still be said, and a CHANGELOG line is enough.** This project's
claim on anyone's attention is that it handles authentication carefully. Fixing
an authentication defect and never recording it leaves a later adopter unable to
tell that releases up to and including 0.78.0 carried it, which is the one thing
disclosure exists to convey. The entry states the affected versions and what was
wrong in a sentence, with no reproduction.

This is a decision about *this* pair of defects. It is not a policy, and
`.github/SECURITY.md` is unchanged: a report from outside still gets the private
advisory route it promises.
