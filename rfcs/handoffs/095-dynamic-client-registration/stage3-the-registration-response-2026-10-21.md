# M3 stage 3 — the registration response

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen`. Stage 2 landed `7cc1488`, Level B 27/0.

**Dispatched.** 2026-10-21 JST, by the architect.
**Plan.** [`m3-stage-plan-2026-10-20.md`](m3-stage-plan-2026-10-20.md) §C.
**Normative source.** [`metadata-validation.md`](metadata-validation.md),
§*Response/error assertions*. Five bullets; each is an item below.
**Stage 2's review.** `.git-exclude/reviewed/rfc-095-stage2-field-rules-2026-10-20.md`
— read §"The finding" before starting. One of this stage's items comes from it.
**Baseline.** Read the tip with `git log -1`.

## 0 — a correction to my own stage plan, before you work from it

The plan said *"neither `Cache-Control: no-store` nor `Pragma` appears in
`handlers/dynamic_register.rs` today."* Literally true, and misleading.
**`Cache-Control: no-store` is already set** — at the router layer,
`router.rs:33-41`, a `SetResponseHeaderLayer::overriding` on
`/oauth2/register` that covers every response from the route, success and
error alike. I measured the handler and reported the handler; the route is
what matters. **`Pragma: no-cache` is genuinely absent** — the only `PRAGMA`
in the HTTP layer is `handlers/oidc.rs:528`.

So item 5 below is smaller than the plan implied, and you should not go
looking for work that is done.

## 1 — the response is missing two fields RFC 7591 requires

`RegistrationResponse` (`dynamic_register.rs:76-94`) carries `client_id`,
`client_secret`, and the metadata echo. **It has neither
`client_id_issued_at` nor `client_secret_expires_at`** — measured, both absent
from the entire workspace.

RFC 7591 §3.2.1 makes `client_secret_expires_at` **REQUIRED** whenever a
`client_secret` is issued, and the matrix asks for *"required issued
identifiers/secret timestamps"* in the same breath as the echo. A client
library that reads `client_secret_expires_at` to decide when to re-register
gets nothing from us today.

- Add both. `client_id_issued_at` is seconds since the epoch;
  `client_secret_expires_at` is `0` when the secret does not expire, which
  RFC 7591 defines explicitly — **`0` means never, not "expired"**, and a
  reader of the code should not have to look that up, so say it in a comment.
- `client_secret_expires_at` appears only when a secret was issued — the
  `none` auth method issues none.
- **Check whether the store already records an issued-at** before adding a
  column. If the client row has a creation timestamp, use it; a second source
  of truth for the same fact is worse than a missing field.

## 2 — the echo: after defaults, canonically ordered

*"Success echoes every registered supported value after defaults and
canonical ordering."*

Two separate requirements and the second is the easy one to skip:

- **After defaults.** What is echoed is what was *stored*, including the
  values stage 2 defaults in — `client_secret_basic`,
  `["authorization_code"]`, `["code"]`. A caller who omits
  `token_endpoint_auth_method` must be able to learn from the response what
  they actually got.
- **Canonically ordered.** `redirect_uris`, `grant_types`, `response_types`
  and the `scope` token list each need one defined order, applied on the way
  out, not whatever order the request happened to use. **Choose the
  canonicalisation and say what it is** — sorted, or request-order-preserved,
  or storage-order — and apply the same rule in the echo as in storage, or
  the two disagree and a re-registration diff becomes noise.

## 3 — unknown metadata is absent from the echo

*"Ignored unknown metadata is absent."* A member this deployment does not
support, sent in a registration that otherwise succeeds, must not appear in
the response. Not echoed, not nulled, not reflected under a different name.

Stage 2 moved `response_types` from `KNOWN_UNSUPPORTED_MEMBERS` to
`SUPPORTED_MEMBERS`; check what remains in each list and make the test
enumerate the unsupported set rather than picking one example from it.

## 4 — the invalid-token states are one shape

*"Invalid-token states have identical status, `WWW-Authenticate`, body, and
cache headers."*

**This is the security item in this stage.** A registration endpoint
protected by a bearer token leaks token state through any difference between
"no token", "malformed token", "unknown token", "expired token" and
"exhausted token" — each distinguishable response is an oracle for probing
which of those a value is.

Make them byte-identical on all four axes. **Test it as an equality between
responses, not as five separate assertions against a literal** — five
assertions that each happen to match today can drift apart one at a time;
an equality between two captured responses cannot.

*Stage 2's `an_exhausted_token_is_refused` already exercises one of these
states. Check whether its current assertion would survive this change, and
whether it still tests what its name says once all five look alike.*

## 5 — the two cache headers

*"Every success/error has `Cache-Control: no-store` and `Pragma: no-cache`."*

`Cache-Control` is handled at the router (see §0). **Add `Pragma: no-cache`
the same way, at the route, not in the handler** — the handler has several
exit paths and a header added per-path is a header that will be missed on the
next one added. `handlers/oidc.rs:525-528` is the existing in-tree example of
the pair being set together.

Assert both on a success **and** on at least one error, since the whole point
is that no response from this route is cacheable.

## 6 — public errors say nothing they should not

*"Public errors contain only fixed ASCII templates and field/index
identifiers."*

Audit `reg_error` and every caller: the body may name *which* field and
*which* index failed, from a fixed set of templates. It may not echo the
offending value, the catalog contents, or anything derived from the request
beyond a field name and an index.

Stage 2's `RedirectRejection::Malformed` carries `uri: String` and a reason,
with a doc comment saying callers choose how much to expose. **This stage is
that choice.** Decide it, apply it, and say what you decided — the enum
deliberately left the question open and it cannot stay open past the response
surface.

## 7 — the integrator note stage 2's review deferred here

`scope` is **required** at registration. That is a deliberate departure from
RFC 7591 §2, where it is OPTIONAL, and
[`metadata-validation.md`](metadata-validation.md) now records the rule and
the reason.

**It is not yet written anywhere an integrator reads.**
`docs/src/reference/oidc-api.md` §*Dynamic client registration (RFC 7591)*
(`:52`, with `### Registering` at `:69`) is where it belongs. State that
`scope` is required, that it must include `openid`, and that this differs
from RFC 7591's optionality — a client library that omits it will be refused,
and the error should not be the first place they learn that.

While you are in that section, check its request/response examples against
what this stage makes true: if they show a response, it now has two more
fields.

## 8 — what to return

The usual package, plus:

- **The canonicalisation you chose** for the ordered fields, and whether
  storage and echo use the same rule.
- **What you decided public errors may contain**, and how
  `RedirectRejection`'s open question is now closed.
- **Whether the store already had an issued-at** before you added anything.
- Mutation evidence for §4 only: make one of the five invalid-token states
  differ on one axis and show the equality test failing. One mutation, not
  five — the equality assertion either holds across all of them or it does
  not.
- The full matrix; Level B scope. Report it local.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the last commit
  of the round, baselined on the tip at that moment.

**A `CHANGELOG` entry: yes.** Two new response fields and a changed error
surface are integrator-visible. `### Added` and `### Changed`; **not**
`### Security` — §4 closes an oracle, but on an endpoint whose token states
were already uniform in status if not in body, so call it what it is in the
entry and let the severity question come to me if you think it is more.

## 9 — what this stage does not do

Not the authentication/grant matrix (stage 4 — independent design review
first), not browser presentation (stage 5), not the legacy-candidate report
(stage 6). It does not close M3.

Leave the working tree uncommitted. One open dispatch at a time; this is it.
