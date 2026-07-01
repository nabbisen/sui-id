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

## Dynamic client registration (RFC 7591)

Third-party applications can self-register via `POST /oauth2/register` using
an initial-access token issued by an administrator.

### Obtaining an initial-access token

An administrator runs:

```sh
sui-id admin issue-registration-token --config sui-id.toml \
  --max-uses 1 --note "MyApp onboarding"
```

The raw token is printed exactly once. It is SHA-256-hashed before
storage — if you lose it you must issue a new one.

### Registering

```http
POST /oauth2/register
Authorization: Bearer <initial-access-token>
Content-Type: application/json

{
  "client_name": "My Application",
  "redirect_uris": ["https://app.example.com/callback"],
  "token_endpoint_auth_method": "client_secret_post",
  "scope": "openid email profile",
  "logo_uri": "https://app.example.com/logo.png",
  "client_uri": "https://app.example.com"
}
```

Response (`201 Created`):

```json
{
  "client_id": "01JXXXXXXXXXXXXXXXXXX",
  "client_secret": "<shown once>",
  "client_name": "My Application",
  "redirect_uris": ["https://app.example.com/callback"],
  "grant_types": ["authorization_code", "refresh_token"],
  "token_endpoint_auth_method": "client_secret_post",
  "scope": "openid email profile"
}
```

> **Important.** Dynamically registered clients start **disabled**.
> An administrator must enable the client in the admin panel before it can
> obtain tokens. This is a deliberate gate — operators review third-party
> registrations before granting access.

Supported fields: `redirect_uris` (required), `client_name` (required),
`scope`, `grant_types`, `token_endpoint_auth_method` (`"client_secret_post"`
or `"none"` for public clients), `logo_uri`, `client_uri`, `policy_uri`,
`tos_uri`, `post_logout_redirect_uris`.
All `*_uri` fields must be HTTPS (or `http://localhost` for development).

Errors follow the RFC 7591 `{"error": ..., "error_description": ...}` shape.

---

## Federated sign-in (RFC 004)

When upstream OIDC providers are configured, sui-id adds "Sign in with X"
buttons to the login page.

The flow is fully server-side from the RP perspective:

1. User clicks the button → redirected to `GET /auth/federated/{slug}/start`
2. sui-id fetches the upstream's discovery document, generates PKCE S256 +
   nonce, seals state in a signed cookie, and redirects to the upstream
   `authorization_endpoint`.
3. After the user authenticates upstream, the browser returns to
   `GET /auth/federated/callback`.
4. sui-id exchanges the code at the upstream `token_endpoint` (PKCE),
   validates the nonce, maps `(provider_id, upstream_sub)` → local user
   (provisioning or link-only depending on `provision_mode`), enforces
   local MFA if enrolled, and issues a sui-id session.

The upstream access token is used only during step 4 and is never persisted.
See `[[federation_provider]]` in the configuration reference for setup.

---

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
| `acr`   | Authentication Context Class Reference (see below) |
| `amr`   | Authentication Methods References (see below)   |

The userinfo response carries `sub`, `preferred_username`, and `name` (when
the user has a display name set).

### `acr` — assurance level

sui-id reports the level-of-assurance of the originating sign-in as
a numeric ISO/IEC 29115 LoA string in the ID token's `acr` claim.
Three levels are produced:

| `acr` | What it means                                                  |
| ----- | -------------------------------------------------------------- |
| `"1"` | Single-factor sign-in. Password only.                          |
| `"2"` | Multi-factor with a software second factor (TOTP, recovery code). |
| `"3"` | Multi-factor with a phishing-resistant hardware-bound key (WebAuthn). |

Numeric strings are the form Keycloak and most other off-the-shelf
IdPs produce; longer URI variants (NIST AAL,
`http://idmanagement.gov/ns/assurance/aal/2`; eIDAS LoA) target
specific national contexts and are not what a general-purpose IdP
should emit.

A relying party that needs a minimum assurance level should compare
the numeric value as a string-or-integer (`"2"` and `"3"` are both
acceptable when the requirement is "at least 2"). sui-id does not
yet support the `acr_values` request parameter that lets a client
*demand* a particular ACR up front; if you need that, raise an
issue.

### `amr` — which factors were used

Alongside `acr`, the `amr` claim is an array of RFC 8176 method
references describing the actual factors used:

| Token   | Meaning                                                |
| ------- | ------------------------------------------------------ |
| `pwd`   | Password.                                              |
| `otp`   | One-time code (TOTP authenticator app or recovery code). |
| `hwk`   | Hardware-bound key proof (WebAuthn).                   |
| `mfa`   | Umbrella signal: two or more distinct factor types were used. |

Examples of what your RP will see:

| Sign-in path                | `acr` | `amr`                       |
| --------------------------- | ----- | --------------------------- |
| Password only               | `"1"` | `["pwd"]`                   |
| Password + TOTP             | `"2"` | `["pwd", "otp", "mfa"]`     |
| Password + recovery code    | `"2"` | `["pwd", "otp", "mfa"]`     |
| Password + WebAuthn passkey | `"3"` | `["pwd", "hwk", "mfa"]`     |
| Federated sign-in only      | `"1"` | `["fed"]`                   |
| Federated + local TOTP      | `"2"` | `["fed", "otp", "mfa"]`     |
| Federated + local WebAuthn  | `"3"` | `["fed", "hwk", "mfa"]`     |

`acr` and `amr` describe the *originating* sign-in. They do not
change as the user uses your application: a session that started
as password-only does not become MFA later just because the user
enrols TOTP afterwards. ID tokens issued via the refresh-token
flow echo the `acr` and `amr` of the original authorization,
verbatim.

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
- Custom claims beyond the OIDC standard set.
- The `acr_values` request parameter on `/oauth2/authorize`: sui-id
  *issues* `acr` and `amr` claims (see "ID token claims") but does
  not yet honour an RP's `acr_values` ask for a minimum assurance
  level up front. Filter on the issued `acr` at the relying party
  for now.
- `prompt=login`, `prompt=none`, or `max_age` parameter
  enforcement.
- Full JWKS signature verification of upstream ID tokens in the
  federation flow (currently trusts the TLS-authenticated
  `token_endpoint`; JWKS validation is a planned hardening step).

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
