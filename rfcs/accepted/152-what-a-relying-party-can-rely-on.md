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
states D8's obligations**, each with what breaks when it is not met.
**Amended.** 2026-11-26, approved by `@nabbisen`: *"Yes on both."*, on the architect's proposal from a prospective relying
party's letter. **D8 added**: what a relying party must do for sui-id's guarantees to reach its users. It adds a documentation
deliverable and no behaviour.
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
  step-up re-authentication (RFC 102). The review checks which step-up paths ask for a credential, and only those count.
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
| `select_account` | sui-id has one account per browser session. **Open question 1**: refused as `account_selection_required`, or treated as `login` |
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

## What this RFC does not do

- **Front- and back-channel logout.** A relying party learns of a disabled user through its next refresh, which a disable
  revokes in the same transaction. A logout channel is a separate decision.
- **`acr_values` as an essential claim**, through the `claims` parameter, which D1 refuses until a later RFC.
- **Email verification.** `email_verified` is `false` for every user until RFC 101 (Accepted; cycle Q) ships its flow.
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

## Open questions

1. **`prompt=select_account`**: refused, or treated as `login`? Refusing is the honest answer for a single-account session.
   Treating it as `login` lets the user change account by signing out and in again, which is what a relying party sending it
   wants.
2. **Which step-up paths count as presenting a credential** for D2 (the review measures them).
3. **A relying party's existing behaviour**: D4 removes claims a relying party may already read without asking for `profile`.
   Should the change ship with one release of warning in the CHANGELOG first? The architect's view is no: there is no
   compatibility promise before 1.0, and the claims were returned against the specification. Naming it in *Changed* is
   enough.
