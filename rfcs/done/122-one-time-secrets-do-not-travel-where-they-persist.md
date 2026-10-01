# RFC 122 — A one-time secret does not travel where it persists

**Status.** Implemented (v0.79.0)
**Closure reviewed on.** 2026-10-02
**Closure approved by.** `@nabbisen` (accountable owner), 2026-10-02: "Batch 2 is approved." The closure review was performed by **the architect, which wrote this RFC**, and is therefore **not** independent of it — `@nabbisen` is the approver, which is what RFC 000 requires when no independent role exists. The implementation role measured the review's claims separately, and corrected two of them; that corroboration is recorded beside the review and is not approval.
**Closure evidence.** [Closure review batch 2, 2026-10-02](../handoffs/120-consent-and-setup-prove-the-caller/closure-review-batch-2-2026-10-02.md), with [independent verification](../handoffs/120-consent-and-setup-prove-the-caller/closure-verification-batch-2-2026-10-02.md)
**Accepted on.** 2026-09-30
**Approved by.** `@nabbisen`, 2026-09-30: "RFC 122 is accepted." Accepted on the
**amended** text: its design review corrected three things this RFC claimed and
found a sixth surface it had missed. An ordinary lifecycle act — nothing here is
exploitable, so nothing justified landing ahead of acceptance.
**Security review.** Required
**Independent design review.** [Design review 2026-09-30](../handoffs/122-one-time-secrets-do-not-travel-where-they-persist/design-review-2026-09-30.md) by the implementation role. Verdict **accept with the changes named**, all three of which correct what this RFC *claimed* rather than what it asks to be built. It also found a **sixth surface this RFC had missed**, with a weaker trust boundary than any it had measured. This RFC was amended on that review the same day.
**Design prerequisites.** None.
**Implementation prerequisites.** None.
**Closure prerequisites.** No secret sui-id shows once is carried in a URL, and no response that carries one is storable: every surface that issues a client secret at creation, a rotated client secret, **a dynamically registered client's secret**, a TOTP secret or QR, or recovery codes is reached by a method that does not put it in a URL, and carries `Cache-Control: no-store`. Each has a test, and a test also holds the guarded set of routes to a stated list in both directions.
**Tracks.** Secret handling. Found by the RFC 119 design review, 2026-09-26; confirmed by the implementation role in the RFC 120 triage.
**Touches.** `crates/sui-id/src/http/handlers/admin/clients.rs`, `crates/sui-id/src/http/handlers/me_security/mfa.rs`, `crates/sui-id/src/http/handlers/dynamic_register.rs`, `crates/sui-id/src/http/router.rs`, `crates/sui-id-web/`, and the tests.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/122-one-time-secrets-do-not-travel-where-they-persist/README.md`](../handoffs/122-one-time-secrets-do-not-travel-where-they-persist/README.md)

## Summary

sui-id shows some secrets exactly once: a client secret at creation, a rotated
client secret, a TOTP secret and its QR code, and recovery codes. Two transports
are wrong for that.

**A rotated client secret is carried in a redirect query string** — so it
reaches the `Location` header, the browser's history, the referrer of anything
the page loads, and any log or proxy that records a URL. The comment above it
says the history entry "is replaced by the subsequent navigation", which is not
what a redirect does. It also means the edit page displays whatever that
parameter contains as "the new secret", so the page can be made to show a value
the server never issued.

**Several of these responses carry no `Cache-Control: no-store`**, so a
back-forward cache keeps them.

**A sixth surface, found by the design review and missed by this RFC:**
`POST /oauth2/register` (RFC 7591 dynamic client registration) returns a
`client_secret` in its JSON body with no `no-store`, and it is the **only** one
of the six not behind a full administrator session — it is mounted under the
token routes and authenticated by a bearer registration token. That is a weaker
boundary than every surface this RFC measured, and a machine-to-machine endpoint
is more likely than an `/admin` page to sit behind a gateway or proxy that logs
or caches responses. **It sharpens this RFC's own closing argument rather than
merely illustrating it.**

## Decision

**D1 — A secret is never a URL component.** Not in a path, not in a query
string, not in a fragment. Where a value must survive one navigation, it travels
by a method that does not put it in a URL, or it does not survive the
navigation.

**D2 — A page renders a secret it obtained, never one it was handed.** The edit
page currently trusts a query parameter. After this RFC, a surface that shows a
secret got it from the operation that produced it.

**D3 — Every response that carries one of these secrets is unstorable.**
`Cache-Control: no-store`, applied as a **router layer on the named routes** —
the pattern `/reset-password` already uses. A response builder or a typed
wrapper would still require a handler to reach for it, which is the failure this
RFC exists to fix with better ergonomics. The layer does not remove the need for
someone to remember; it **relocates the remembering into one file**, so a
reviewer asking "does every secret-bearing route carry this" reads `router.rs`
once instead of auditing handler bodies for a call that may not be there.

**A referrer policy is redundant here, not required, and this RFC said otherwise.**
The first draft bundled the two protections in one sentence as though they served
the same purpose on every surface. They do not: `no-store` protects the response
**body**; a referrer policy protects the page's **own URL** from travelling in a
`Referer` header — which matters only when the secret is *in* that URL. **D1
removes the secret from every URL, so after D1 there is nothing left for a
referrer policy to protect on any of these six surfaces**, and the global
`strict-origin-when-cross-origin` default already applies. The recovery-link
page's `no-referrer` is its author's second layer of caution, not a requirement
D3's own reasoning implies for the others.

**D4 — The list is checked in both directions, and its one weakness is stated
rather than implied.** The secrets sui-id shows once are enumerated, and a test
parses `router.rs` — the technique RFC 120's route test already uses — to assert
that **every route on the list carries the `no-store` layer, and that no route
off the list carries it**. The second direction is what makes a quiet addition or
removal a diff someone must approve.

**This is not RFC 120's derivation, and this RFC does not claim it is.** RFC
120's enumeration works because "does this handler take an actor extractor" is a
property of the signature, checkable without understanding the handler. "Does
this handler show a one-time secret" has no such marker, and inventing one — a
newtype return, an attribute macro — is more machinery than this RFC's own
"modest exposure" framing justifies. **So the membership of the list is
hand-maintained and nothing mechanically proves it complete**: a genuinely new
secret-bearing surface can still be added without anyone adding it. That is the
residual, and it is written here rather than left for a reader to assume
otherwise.

## What this RFC does not claim

The exposure is modest and this RFC does not pretend otherwise: these are POST
responses or short-lived query-carrying GETs, on an administrative interface, on
a service with no known deployment. It is worth fixing because the cost is small
and because a secret in a URL is the kind of thing that is cheap now and
expensive after a log aggregator is introduced.

**Two severities are stated so they are not inflated by proximity to each other.**
The edit page rendering an unvalidated query parameter as "the new secret" is
**not** an XSS vector — the render path is Leptos text interpolation, which
escapes, with no raw-HTML sink anywhere in it — and nothing that parameter
contains is written, checked or granted access. It is a display forgery: an
attacker who knows a client's id can send an administrator a link that shows a
false "new secret" on the genuine page under the administrator's own session.
Social engineering against the chrome, **medium at most**, and it is fixed as a
consequence of D1 removing the transport, not because it is independently
dangerous.

**And "render directly" is not a durability improvement.** Five of the six
surfaces already render their secret straight from the POST response; only the
rotated secret redirects. Making it match its sibling is copying a pattern this
same file already contains — but a directly-rendered response is not re-derivable
by a reload either, so an operator who navigates away has lost the value exactly
as they would today. D1's requirement is narrower than durability: the value must
never *become a URL*. This RFC asks for that and no more.
