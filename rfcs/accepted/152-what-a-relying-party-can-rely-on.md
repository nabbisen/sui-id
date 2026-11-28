# RFC 152 — What a relying party asks for, sui-id honours or refuses

**Status.** Accepted
**Accepted on.** 2026-11-26
**Approved by.** `@nabbisen`, 2026-11-26: *"Accepted."*
**Security review.** Required. It changes when a user must sign in again, and what an authorization response means.
**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted. The schedule proposed for it (cycle Q, beside RFC 149) was authorized
on the architect's assessment of two app teams' letters (`@nabbisen`, 2026-11-25: *"Authorized."*). That authorizes
the schedule, **not this RFC's acceptance**.
**Closure prerequisites.** Every authorization-request parameter that states a requirement is either honoured or refused
with an error, and none is silently ignored. `max_age` and `prompt=login` force a new sign-in, and the ID token says when it
happened. Userinfo returns profile claims only under the `profile` scope. A signing-key rotation cannot fail a sign-in at a
relying party that caches the JWKS as sui-id tells it to. Each of these is tested, with a mutation. **And the reference
states D8's obligations**, each with what breaks when it is not met. **From the second amendment**: a federated
sign-in never makes `auth_time` newer than a credential it can show, and no app keeps an empty scope list (D9).
**Amended.** 2026-11-26, approved by `@nabbisen`: *"Yes on both."*, on the architect's proposal from a prospective relying
party's letter. **D8 added**: what a relying party must do for sui-id's guarantees to reach its users. It adds a documentation
deliverable and no behaviour.
**Amended.** 2026-11-28, on the architect's consideration of its three open questions and one more
([`open-questions-considered-2026-11-28.md`](../handoffs/152-what-a-relying-party-can-rely-on/open-questions-considered-2026-11-28.md)).
`@nabbisen` chose each recommendation: *"Two-choice page (Recommended)"*, *"Prove it upstream (Recommended)"* and *"End it
by migration (Recommended)"*. The open questions are resolved in D2 and D3, and **D9 is added**: an empty scope list no
longer means *"permit any"*. **The release moves to `0.96.0`**: `0.95.0` was cut after RFC 149 (`@nabbisen`: *"0.95.0 now
(Recommended)"*).
**Tracks.** The OpenID Connect provider. Raised by a relying party's questions, 2026-11-25.
**Touches.** `crates/sui-id/src/http/handlers/oidc.rs` (the authorization request and userinfo); `crates/sui-id-core/src/oidc/`
(the code and token issuance, discovery, JWKS); `crates/sui-id-store` (an authorization code and a refresh token carry the
sign-in time; signing-key publication); `docs/src/reference/oidc-api.md`.
**Independent design review.** [Design review record, 2026-11-26](../handoffs/152-what-a-relying-party-can-rely-on/design-review-2026-11-26.md). By `@nabbisen`, who did not author it.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/152-what-a-relying-party-can-rely-on/README.md`](../handoffs/152-what-a-relying-party-can-rely-on/README.md)

## Summary

A relying party wrote to ask what it may rely on. Measuring the answers at `648e2ae` found four places where **sui-id's
success response means something sui-id did not do**, or where a relying party following sui-id's own advice can fail:

1. **`prompt` and `max_age` are ignored without an error.** `AuthorizeQuery` (`handlers/oidc.rs`) reads a fixed set of
   parameters and drops the rest. A user with a live sui-id session gets a code at once, so a relying party that sent
   `prompt=login` gets a code with no new sign-in, and **no ID token says when the sign-in happened** (there is no `auth_time`).
   OpenID Connect Core requires a provider that receives `max_age` to re-authenticate past it, and to return `auth_time`.
2. **Userinfo returns `preferred_username` and `name` whatever the scope.** Only `email` is gated on its scope.
3. **A rotated signing key is published at the moment it starts signing.** The JWKS is served with
   `Cache-Control: public, max-age=300`. A relying party, or a shared cache in front of it, may hold a JWKS without the new
   `kid` for up to five minutes, and a sign-in in that window fails.
4. **Discovery does not say which claims exist** (`claims_supported`), or that request objects are unsupported. A relying
   party has to guess, and the one that wrote guessed wrong about where `name` comes from.

## Decision

### D1 — a parameter that states a requirement is honoured or refused, never ignored

The authorization request sorts every OpenID Connect parameter into one of two kinds, and the reference documents each:
- **A requirement**: `max_age`, `prompt`, `request`, `request_uri`, `claims`, `id_token_hint`. Each is **honoured (D2, D3)
  or refused with the protocol's own error**, redirected to the relying party:
  - `request` → `request_not_supported`;
  - `request_uri` → `request_uri_not_supported`;
  - `claims` → `invalid_request`, until a later RFC supports it.

  `id_token_hint` is honoured only as a check that it names the signed-in user, and is refused otherwise.
- **A hint**: `login_hint`, `ui_locales`, `display`, and a *voluntary* `acr_values`. These may be ignored, as the
  specification allows, and the reference says which.

**A parameter sui-id does not recognise at all** is ignored, as OAuth requires. The rule above is for the ones whose meaning
it knows.

### D2 — `max_age` and `auth_time`

- **The sign-in time** of a session is the last time its user presented a credential to it: the sign-in itself, or a
  step-up re-authentication (RFC 102). **Resolved 2026-11-28 (open question 2)**, by one rule: *`auth_time` may be older
  than the truth, never newer.* So the sign-in time moves only when sui-id checked a credential itself, or verified that a
  federation provider checked one because sui-id asked:
  - **it moves** on a local sign-in (a password, local or directory, or a passkey, with any second factor), on a step-up
    through L05 (a TOTP code or a passkey), and on a federated sign-in that completes a local second factor;
  - **it does not move** on B7's password re-check (`first_factor_proof`), which records nothing on the session;
  - **a federated sign-in that must be fresh** (`prompt=login`, or `max_age` exceeded) counts only if sui-id sent
    `prompt=login` and `max_age=0` upstream, and the provider's ID token carries an `auth_time` no earlier than the moment
    sui-id sent the request, within the ID token check's skew. Otherwise the sign-in fails with a page that says the
    provider did not confirm a new sign-in, and nothing is issued. Measured: today the upstream request carries neither
    parameter (`crates/sui-id/src/http/handlers/federation.rs:532`), and the provider's `auth_time` is discarded;
  - **any other federated sign-in**: `auth_time` is the provider's `auth_time` when it sends one, never later than sui-id
    accepted the sign-in; when it sends none, the moment sui-id accepted it, and the reference says so (D7).
- `max_age=N` with a session whose sign-in time is more than `N` seconds ago **sends the user through sign-in again before
  a code is issued**. `max_age=0` always does.
- **`auth_time` is in every ID token.** OpenID Connect allows it unconditionally, and one rule is clearer than two. The
  authorization code and the refresh token carry it, so an ID token from a refresh echoes the original `auth_time`, as it
  already echoes `acr` and `amr`. That needs one column on each table, added by a migration.

### D3 — `prompt`

| Value | sui-id does |
|---|---|
| `none` | Issues a code only if the user has a session and nothing would need the user, and answers `login_required`, `consent_required` or `interaction_required` otherwise. `none` with any other value is `invalid_request` |
| `login` | Sends the user through sign-in again, even with a live session, and `auth_time` is the new time |
| `consent` | Shows the consent screen even where the client's policy would skip it |
| `select_account` | **Honoured by a two-choice page** (resolved 2026-11-28, open question 1): *Continue as `<name>`*, without a new sign-in, or *Sign in as someone else*, which is `login`'s path. The page says that switching signs this browser out of `<name>` for every app that uses sui-id in it. With no session, the sign-in page is shown. With `login` as well, *Continue* asks for the credential again, for the same account |
| anything else | `invalid_request` |

Discovery gains `prompt_values_supported` with exactly the values honoured.

### D4 — userinfo returns profile claims under `profile` only

`preferred_username` and `name` are returned when the access token's scope includes `profile`. This is a behaviour change for
a relying party that relied on getting them without asking, so it is named in the CHANGELOG's *Changed*.

### D5 — a signing key is published before it signs

- A rotation creates the new key **published but not yet signing**.
- **The signer is chosen when each token is signed**: the newest key that has been published for at least the JWKS cache
  lifetime (300 seconds, one named constant shared with the `Cache-Control` header). Until then, the previous key keeps
  signing.
- **No scheduler is needed**, and nothing can activate a key that relying parties have not had time to see.
- The admin console says when the new key will start signing.
- Keeping `public` in `Cache-Control` is then harmless. **Removing `public` would not have helped**: a shared cache may store
  a response without `Authorization` whether or not it is marked `public`. (The architect's assessment of 2026-11-25
  proposed removing it; that proposal is withdrawn here.)

### D6 — discovery says what exists

Discovery adds:
- `claims_supported`, generated from the same list the ID token and userinfo serialize, so it cannot drift;
- `prompt_values_supported` (D3);
- `claims_parameter_supported`, `request_parameter_supported` and `request_uri_parameter_supported`, all `false`.

### D7 — the reference states each parameter's handling in one table

`docs/src/reference/oidc-api.md`:
- one table of every authorization-request parameter: honoured, refused (with its error) or ignored as a hint;
- `auth_time`;
- the profile-scope rule;
- the key-publication delay.

The *What sui-id does not do (yet)* list loses the entries this RFC delivers.

### D8 — what a relying party must do, stated where it reads

Added 2026-11-26. The finding, from a prospective relying party:
- sui-id revokes a disabled user's refresh tokens in the same transaction as the disable;
- **but the relying party's own session lifetime is the delay** before the person loses access;
- **nothing sui-id published said so.** One candidate relying party, measured, keeps a seven-day session with no refresh.

Its guarantee is built correctly and would not reach the person it protects.

So `docs/src/reference/oidc-api.md` gains a section, **"What your relying party must do"**. Its first line says what RFC
119's host obligations cannot say: **sui-id cannot enforce any of these**, because the relying party's session is outside
it. Each obligation names what breaks:

| # | A relying party should | What breaks if it does not |
|---|---|---|
| **R1** | Bound how long it trusts a sign-in: refresh at an interval it chooses, or keep sessions short | A user disabled at sui-id keeps access for the rest of the relying party's session. That session's lifetime is the revocation delay |
| **R2** | Store `auth_time` as its own field, never inferred from its own session's issue time | The issue time stops meaning authentication when silent renewal is added, and `max_age` checks answer with the wrong fact, silently |
| **R3** | Treat a re-authentication as true at that moment only, and ask again (`max_age`) when it needs recency later | A long-lived session minted from a `prompt=login` response voids the requirement it asked for |
| **R4** | Key accounts on `(iss, sub)`, never on the email or the username | Accounts merge or split when either changes |
| **R5** | Treat `email_verified = false` as unverified (every user, until RFC 101 ships) | An unverified address is trusted as an identity |
| **R6** | Keep only the newest refresh token | Presenting an old one is treated as theft: that grant's whole token family is revoked, and the user signs in again |
| **R7** | Refresh the JWKS once on an unknown `kid`, bypassing caches | A sign-in fails in the minutes around a signing-key rotation, until D5 lands |

**Tests.** None run in sui-id: the list is documentation. Each row's claim about sui-id is checked against the code when the
section is written, as R6 and R7 were.

### D9 — an empty scope list no longer means *"permit any"*

Added 2026-11-28.
- **Today** `enforce_scope_policy` skips the check for an app whose scope list is empty
  (`crates/sui-id-core/src/oidc/authorize.rs:309`). Such an app can be issued a token carrying any scope string it names,
  defined or not. That is fail-open, and *"empty means everything"* is a meaning nobody guesses.
- **Nothing creates one any more.** Dynamic registration refuses an empty `scope`, and the app form always includes
  `openid`. They remain from before migration 0002 (v0.29.12), and from the old edit form, up to `0.94.0`.
- **A migration ends it** without breaking a working relying party:
  - each empty list becomes `openid`, the standard scopes, and every scope defined when the migration runs;
  - each converted app gets an audit record naming the list it was given;
  - a `CHECK` then refuses an empty list.
- **What changes for a relying party**: only a request for a scope nobody defined. It now gets `invalid_scope`, with the
  message that names the console page.
- **The upgrade note names the query** that finds such apps beforehand.

## What this RFC does not do

- **Front- and back-channel logout.** A relying party learns of a disabled user through its next refresh, which a disable
  revokes in the same transaction. A logout channel is a separate decision.
- **`acr_values` as an essential claim**, through the `claims` parameter, which D1 refuses until a later RFC.
- **Email verification.** `email_verified` is `false` for every user until RFC 101 (Accepted; cycle R, `0.97.0`, since `0.95.0` was cut after RFC 149 on 2026-11-28) ships its flow.
  RFC 101's D2 already makes an address change a pending new address rather than an edit of the verified one.

## Tests, each with a mutation

| Test | Mutation it must catch |
|---|---|
| A signed-in user, `prompt=login`: the sign-in page is shown, and the ID token's `auth_time` is after the request | `prompt` ignored again |
| `max_age=60` with a session signed in 61 seconds ago (a controlled clock) re-authenticates; with one signed in 59 seconds ago, it does not | the comparison inverted, or `>=` for `>` |
| `prompt=none` without a session redirects `login_required` and shows no page | a login page rendered |
| `request=…`, `request_uri=…` and `claims=…` each redirect their D1 error | any of them ignored |
| A refresh's ID token carries the original `auth_time` | `auth_time` re-stamped at refresh |
| Userinfo without `profile` omits `preferred_username` and `name`; with it, includes them | the scope check removed |
| After a rotation, tokens are signed by the previous key until the new one has been published for the cache lifetime, then by the new one (a controlled clock) | the new key signing at once |
| `claims_supported` equals the union of the serialized claim names | a claim added to the ID token but not to the list |
| `prompt=select_account` with a session shows the two choices; *Continue* issues a code with the original `auth_time` | *Continue* re-stamping `auth_time`, or the page skipped |
| A federated sign-in under `prompt=login` whose provider returns no `auth_time`, or one older than the request, is refused and issues nothing | the upstream `auth_time` check removed |
| After the migration, no app has an empty scope list, each converted app has an audit record, and an empty list cannot be stored | the `CHECK` removed, or an empty list skipping the check again |

## Open questions

**All three are resolved, 2026-11-28** (the second amendment, above). They are kept as they were asked.

1. **`prompt=select_account`** (resolved in D3): refused, or treated as `login`? Refusing is the honest answer for a single-account session.
   Treating it as `login` lets the user change account by signing out and in again, which is what a relying party sending it
   wants.
2. **Which step-up paths count as presenting a credential** for D2 (resolved in D2, with federated sign-in).
3. **A relying party's existing behaviour** (resolved: no warning release is needed, because `0.95.0`'s upgrade note announces D4 one release ahead, and tells both the relying party's developer and the administrator what to do): D4 removes claims a relying party may already read without asking for `profile`.
   Should the change ship with one release of warning in the CHANGELOG first? The architect's view is no: there is no
   compatibility promise before 1.0, and the claims were returned against the specification. Naming it in *Changed* is
   enough.
