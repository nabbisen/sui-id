# Integrator's guide

This guide is for someone integrating an application against a sui-id
instance that someone else is running. If you are operating sui-id itself,
see [operators.md](operators.md) for the reference and
[deployment.md](deployment.md) for the first-time install walkthrough.

## Discovering the provider

Every sui-id instance publishes an OIDC discovery document at:

```
GET <issuer>/.well-known/openid-configuration
```

The `issuer` is whatever the operator has set in their config. Most OIDC
client libraries can take that URL and configure themselves automatically.

The matching JWKS is at `<issuer>/.well-known/jwks.json` and contains one or
more Ed25519 (`OKP` / `EdDSA`) keys.

## Registering a client

Clients are created by an administrator from the sui-id admin panel,
under `/admin/clients`. The administrator gives:

- A human-readable name.
- One or more redirect URIs. They must be `https://...` URLs, except for
  loopback (`http://localhost:...`) which is permitted for development.
- Whether the client is **confidential** (server-side, gets a client
  secret) or **public** (browser-only or native, no secret).
- An **allowed scopes** list (default `openid profile`). Any scope your
  application asks for at `/authorize` must appear in this list; a
  request that exceeds the policy is rejected with `invalid_scope`.
  An empty list means "permit any scope" — that's the legacy fallback
  for clients registered before the scope-policy feature shipped, but
  new registrations should always declare what they actually need.
- An optional **post-logout redirect URI** list. If provided, only
  these URIs may be used as `post_logout_redirect_uri` at
  `/oauth2/logout`. If empty, sui-id falls back to the authorization
  redirect URI list — this is convenient but not standards-blessed,
  and a deprecation warning will appear in the operator's log.

When the form is submitted, sui-id displays the new client id and — if
confidential — the client secret. **The secret is shown exactly once.** If
you lose it, the administrator must re-issue or replace the client.

The administrator can later edit the name, redirect URIs, allowed
scopes, and logout return URIs from `/admin/clients/{id}/edit`. The
client id, type (confidential vs public), and secret are immutable.

## The flow

sui-id supports exactly one OIDC flow: **Authorization Code with PKCE
(S256)**. Implicit, hybrid, and password-grant flows are not supported.

### 1. Send the user to /authorize

```
GET <issuer>/oauth2/authorize
    ?client_id=<your-client-id>
    &redirect_uri=<one of your registered URIs>
    &response_type=code
    &scope=openid profile email
    &state=<csrf-protected random>
    &nonce=<random per-request>
    &code_challenge=<base64url SHA-256 of your verifier>
    &code_challenge_method=S256
```

If the user is not signed in to sui-id, they will see a login form, then be
redirected back through `/authorize` automatically.

On success, the user is redirected to:

```
<your redirect_uri>?code=<one-time code>&state=<your state>
```

### 2. Exchange the code at /token

```
POST <issuer>/oauth2/token
Content-Type: application/x-www-form-urlencoded

grant_type=authorization_code
&code=<the code>
&redirect_uri=<same as in step 1>
&client_id=<your-client-id>
&client_secret=<your-client-secret>   # only for confidential clients
&code_verifier=<the original verifier you hashed in step 1>
```

Confidential clients may alternatively pass credentials via HTTP Basic
authentication (`Authorization: Basic <base64(client_id:client_secret)>`).

The response is a standard JSON token document:

```json
{
  "access_token": "<JWT signed by sui-id>",
  "token_type": "Bearer",
  "expires_in": 900,
  "refresh_token": "<opaque string>",
  "id_token": "<JWT signed by sui-id, present when scope contains openid>"
}
```

### 3. Use the access token

`access_token` is a JWT. Verify it against sui-id's JWKS:

- `alg`: `EdDSA`
- `kid`: matches a key in the JWKS document
- `iss`: the sui-id `issuer`
- `aud`: your `client_id`
- `exp`: not in the past

You can also call `<issuer>/oauth2/userinfo` with `Authorization: Bearer
<access_token>` to fetch the user's profile.

### 4. Refresh

When the access token expires:

```
POST <issuer>/oauth2/token

grant_type=refresh_token
&refresh_token=<the previous one>
&client_id=<your-client-id>
&client_secret=<your-client-secret>
```

sui-id **rotates** refresh tokens on every use: the response contains a
brand-new `refresh_token`, and the previous one is invalidated. Always
persist the new one.

## ID token claims

ID tokens carry the standard OIDC claim set:

| Claim   | Meaning                                         |
| ------- | ----------------------------------------------- |
| `iss`   | sui-id issuer URL                               |
| `sub`   | Stable, opaque user identifier (UUID)           |
| `aud`   | Your `client_id`                                |
| `iat`   | Issued at, unix seconds                         |
| `exp`   | Expires at, unix seconds                        |
| `nonce` | Echoed from your `/authorize` request, if given |

The userinfo response carries `sub`, `preferred_username`, and `name` (when
the user has a display name set).

## Multi-factor authentication

MFA is an *internal* concern of sui-id, not something the relying
party participates in. The user, on a per-account basis, opts into
TOTP (RFC 6238 authenticator app) or one or more WebAuthn passkeys —
or both — from the sui-id Profile page. After they do, the password
form at `/admin/login` redirects to a second-factor challenge before
issuing a session.

What this means for you:

- The `/authorize` flow is unchanged. From the relying party's
  point of view nothing about the OIDC dance signals whether the
  user authenticated with one factor or two.
- The `acr` ("authentication context class") and `amr`
  ("authentication methods references") claims are **not** currently
  set on issued ID tokens. If you need to know whether MFA was
  used, that is on the roadmap; today you can't tell. For most
  deployments this is fine — the operator decides whether the
  population must use MFA, the application trusts the IdP's
  decision.
- If your application has its own additional step-up requirement
  (e.g. "require MFA only for the admin panel"), implement it on
  your side; sui-id has no `prompt=login` or `max_age` enforcement
  yet.

## Logout

sui-id supports OpenID Connect RP-Initiated Logout 1.0 at
`GET /oauth2/logout`. The relying party sends the user there with:

```
GET /oauth2/logout
    ?id_token_hint={id_token}
    &client_id={client_id}
    &post_logout_redirect_uri=https://your.app/signed-out
    &state={opaque}
```

`id_token_hint` is recommended; sui-id uses it to identify the user
without prompting. `post_logout_redirect_uri` must be either
registered in the client's logout-URI list (preferred), or — for
backwards compatibility — match one of the client's authorization
redirect URIs.

After revoking the user's sui-id session, sui-id redirects back to
the supplied URI (with `state` echoed if provided). If the URI is
not acceptable, sui-id renders a static "Signed out" confirmation
page instead.

## Token introspection (RFC 7662)

Confidential clients can ask `POST /oauth2/introspect` whether a
token they hold is still valid:

```
POST /oauth2/introspect
Authorization: Basic base64(client_id:client_secret)
Content-Type: application/x-www-form-urlencoded

token=eyJhbGc...&token_type_hint=access_token
```

`token_type_hint` is optional but speeds the lookup if you know
which kind of token you're asking about. The two accepted hints are
`access_token` and `refresh_token`.

The response is JSON. For an active token:

```json
{
  "active": true,
  "scope": "openid profile",
  "client_id": "...",
  "username": "alice",
  "token_type": "Bearer",
  "exp": 1735689600,
  "iat": 1735688700,
  "sub": "...",
  "aud": "...",
  "iss": "https://idp.example.com"
}
```

For an inactive (expired, revoked, malformed, or belonging to a
different client) token:

```json
{ "active": false }
```

A few sui-id-specific points:

- **Public clients cannot introspect.** Only confidential clients
  have a secret to authenticate with at this endpoint. Public
  clients should not need introspection — they hold the token
  themselves and can read its `exp` claim directly.
- **A client can only see its own tokens.** Submitting another
  client's token returns `{"active": false}` rather than its
  metadata. This avoids using introspection as an oracle to fish
  for valid tokens.
- The `Authorization: Basic` header is preferred per RFC 6749 §2.3.1
  but `client_id` + `client_secret` form fields also work.

## Token revocation (RFC 7009)

Confidential clients can also revoke a refresh token at
`POST /oauth2/revoke`:

```
POST /oauth2/revoke
Authorization: Basic base64(client_id:client_secret)
Content-Type: application/x-www-form-urlencoded

token=eyJhbGc...&token_type_hint=refresh_token
```

Per RFC 7009 §2.2, the response is **always** `200 OK` with an empty
body — even if the token was already revoked, expired, or never
existed. This is deliberate: the revocation endpoint must not be
usable as an oracle. The only error responses are
`invalid_request` (malformed body), `unsupported_token_type`, or
`invalid_client` (auth failure).

Effects:

- **Refresh token**: marked revoked at the storage layer. The next
  attempt to use it at `/oauth2/token` returns `invalid_grant`.
  Existing access tokens issued from the same authorization grant
  remain valid until they expire — sui-id's access tokens are
  stateless JWTs and cannot be revoked individually except via the
  small deny-list (see below).
- **Access token**: a `jti` deny-list entry is recorded so that
  subsequent `/oauth2/introspect` calls will report it inactive.
  Note that this does *not* prevent the access token from being
  accepted at relying-party APIs that don't introspect — those
  applications validate the JWT signature locally and have no
  signal of revocation. The deny-list entry is garbage-collected
  once the token's original `exp` passes.

For a complete logout, the relying party should call `/oauth2/revoke`
with the user's refresh token *and* call `/oauth2/logout` (RP-
Initiated Logout) to end the user's sui-id session. Doing only one
of the two leaves a path back: the refresh token can mint new
access tokens, or the sui-id session can be used to log straight
back into the application.

## Errors

Errors at `/token` follow RFC 6749 §5.2. Errors at `/authorize` follow RFC
6749 §4.1.2.1. sui-id additionally returns its own JSON envelope at the
admin/management endpoints:

```json
{
  "code": "invalid_state",
  "message": "This server has not been initialized yet.",
  "request_id": "8e6c3d27...",
  "protocol_code": "invalid_grant"
}
```

The `request_id` is the value to mention when filing an operator support
ticket: it correlates to a specific log line on the server side without
exposing the underlying cause to the caller.

## What sui-id does not do (yet)

- Front-channel or back-channel logout. (RP-initiated logout *is*
  supported; see above.)
- Dynamic client registration.
- Custom claims beyond the OIDC standard set.
- `acr` / `amr` claims signalling whether MFA was used during
  authentication.
- `prompt=login`, `prompt=none`, or `max_age` parameter
  enforcement.

If you need any of those today, you may need a different IdP. Several are on
the [roadmap](../ROADMAP.md).

## Further reading

- [`docs/deployment.md`](deployment.md) — chronological install
  walkthrough for an operator deploying sui-id from scratch.
- [`docs/operators.md`](operators.md) — operational reference for
  someone running sui-id day to day.
- [`docs/threat-model.md`](threat-model.md) describes the threats the
  protocol surface defends against and what assumptions you may safely
  make about a properly-configured sui-id deployment.
