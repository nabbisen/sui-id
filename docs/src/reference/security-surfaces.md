# Security surfaces reference

This page states two facts about what sui-id exposes, for an operator
assessing their own exposure or an architect designing against this system:
**which routes answer without a signed-in session**, and **which surfaces
show a secret exactly once**. It does not describe *how* the code keeps
either promise — which extractor gates a route, which header layer protects
a response — only *what* the system guarantees. `router.rs` and the code
behind each surface are the how; this page is the what.

Both tables below are **checked, not merely cross-referenced**: the cited
test parses `crates/sui-id/src/http/router.rs` directly, at every run, and
fails if a route is added to or removed from the set it derives without this
page changing to match. A row added here that the router doesn't produce, or
a route the router now produces that isn't a row here, fails that test — the
same way it already failed before this page existed, for anyone who forgot a
reason.

## Routes that answer without an authenticated caller

A route belongs here because it is public by nature, because it
authenticates something other than a browser session (a client, a bearer
token, a one-time token, a pending sign-in), or because it does its own
session check inline. Every other route in sui-id requires a signed-in
session. Roughly grouped below in that order — public by nature, first-run
setup, OAuth/OIDC protocol endpoints, sign-in and account recovery,
optional operational endpoints — matching how `r120_routes.rs` lists them.

Checked by `r120_routes::the_routes_that_answer_without_an_actor_are_exactly_the_expected_set`
(`crates/sui-id/tests/e2e/r120_routes.rs`), which derives this set from
`router.rs` and a second test, `every_route_classified_as_requiring_an_actor_turns_an_anonymous_caller_away`,
which confirms every route **not** on this list actually turns an anonymous
caller away rather than trusting the classification alone.

| Method | Path | Why it answers without a caller |
|---|---|---|
| `GET` | `/` | landing; redirects by initialization state |
| `GET` | `/healthz` | liveness probe; leaks nothing (RFC 016) |
| `GET` | `/.well-known/openid-configuration` | OIDC discovery |
| `GET` | `/.well-known/jwks.json` | public signing keys |
| `GET` | `/static/{*path}` | embedded static assets |
| `GET` | `/setup` | welcome; redirects once initialized |
| `GET` | `/setup/admin` | form; redirects once initialized |
| `POST` | `/setup/admin` | gated by the setup token, and refused once initialized |
| `GET` | `/setup/done` | informational only |
| `GET` | `/oauth2/authorize` | resolves the session inline and sends an anonymous user to sign in |
| `GET` | `/oauth2/logout` | RP-initiated logout: an id_token_hint or the session cookie, inline |
| `POST` | `/oauth2/token` | client authentication and a one-time code or refresh token |
| `POST` | `/oauth2/register` | RFC 7591: an initial-access token |
| `POST` | `/oauth2/introspect` | client authentication |
| `POST` | `/oauth2/revoke` | client authentication |
| `GET` | `/oauth2/userinfo` | bearer access token |
| `POST` | `/oauth2/userinfo` | bearer access token |
| `GET` | `/admin/login` | sign-in form |
| `POST` | `/admin/login` | credentials; lockout and rate limit |
| `GET` | `/admin/login/mfa` | second factor; a pending-sign-in cookie |
| `POST` | `/admin/login/mfa` | second factor; a pending-sign-in cookie |
| `POST` | `/admin/login/webauthn/start` | passkey sign-in; a pending ceremony |
| `POST` | `/admin/login/webauthn/complete` | passkey sign-in; a pending ceremony |
| `POST` | `/admin/logout` | ends the caller's own session; CSRF |
| `GET` | `/admin/profile` | permanent redirect to /me/security |
| `GET` | `/admin/settings` | constant redirect to the first settings tab; reads nothing |
| `GET` | `/me/security` | constant redirect to the overview; reads nothing |
| `GET` | `/forgot-password` | recovery form |
| `POST` | `/forgot-password` | uniform response; rate limit |
| `GET` | `/reset-password` | a one-time reset token |
| `POST` | `/reset-password` | a one-time reset token |
| `GET` | `/auth/federated/{slug}/start` | upstream sign-in start |
| `GET` | `/auth/federated/callback` | upstream sign-in callback; a signed state cookie |
| `GET` | `/metrics` | a bearer token or an administrator session, inline; mounted only when enabled |

## Surfaces that show a secret once

Six surfaces show a value once — a client secret, a TOTP secret and QR code,
or a set of recovery codes — and never persist or redisplay it in plaintext
afterward (RFC 122). Each reaches the reader directly in the body of the
response that produced it: never a redirect, never a query string, never any
other URL component (RFC 122 D1). Every route in the table below also carries
`Cache-Control: no-store` at the router layer — `router.rs`'s own
`SetResponseHeaderLayer::overriding(CACHE_CONTROL, "no-store")`, attached to
the named route rather than left to a handler to remember — so a
back-forward cache or an intermediate proxy cannot retain the response
either. `/reset-password` carries the same header for the same reason,
protecting a one-time password-reset token rather than a shown secret; it is
included because it uses the identical mechanism, not because it is one of
the six.

Checked by `r122_routes::the_routes_carrying_the_no_store_layer_are_exactly_the_expected_set`
(`crates/sui-id/tests/e2e/r122_routes.rs`), which parses `router.rs` for
exactly this header layer and asserts the set **in both directions**: no
route carries it without appearing here, and no row here names a route that
has stopped carrying it.

| Route | What it shows, once |
|---|---|
| `/reset-password` | RFC 103: a one-time reset token held in a resubmitted form field |
| `/admin/clients` | RFC 122: POST shows a freshly generated client secret once |
| `/admin/clients/{id}/rotate-secret` | RFC 122: renders the rotated client secret directly, once |
| `/me/security/mfa/enroll/start` | RFC 122: shows the TOTP secret and QR code once |
| `/me/security/mfa/enroll/confirm` | RFC 122: shows the fresh recovery codes once |
| `/me/security/mfa/recovery-codes/regenerate` | RFC 122: shows the regenerated recovery codes once |
| `/oauth2/register` | RFC 122: RFC 7591 response carries a generated client_secret |

**A seventh surface, deliberately outside this table.**
`POST /admin/users/{id}/recovery-link` also shows a secret once — an
administrator-issued account-recovery link — and also carries
`Cache-Control: no-store`, but sets it in the handler itself rather than at
the router layer, predating RFC 122 and outside what `r122_routes.rs`
parses. Its no-store header is checked by its own test,
`r103_s4_issuance_response_is_not_cacheable_sends_no_referrer_and_is_not_a_redirect`
(`crates/sui-id/tests/e2e/r103_stage4.rs`), not by the test above. Stating
that boundary here, rather than adding a row this page's own test cannot
verify, is the same choice RFC 122 made when it left this route out of
`r122_routes.rs`'s hand-list for the identical reason.

## On the parse

Both tables above are ordinary markdown, written to be read; the tests cited
under each locate the table by its heading and parse the pipe-delimited rows
that follow, stripping the backticks each `Method`/`Path`/`Route` cell wears
for readability. This is fiddlier than parsing a purpose-built data format
would be, and deliberately so — reshaping either table into something easier
to machine-parse would serve the test instead of the reader these tables
exist for, which is not the trade this RFC asked for. If a future edit to
either table's prose (not its rows) breaks the parse, that is this design's
known limitation, not a hidden one.
