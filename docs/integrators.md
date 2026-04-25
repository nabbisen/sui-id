# Integrator's guide

This guide is for someone integrating an application against a sui-id
instance that someone else is running. If you are operating sui-id itself,
see [operators.md](operators.md).

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

When the form is submitted, sui-id displays the new client id and — if
confidential — the client secret. **The secret is shown exactly once.** If
you lose it, the administrator must re-issue or replace the client.

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

- Logout / RP-initiated logout.
- Front-channel or back-channel logout.
- Dynamic client registration.
- Custom claims beyond the OIDC standard set.
- Token introspection (RFC 7662).
- Token revocation (RFC 7009).

If you need any of those today, you may need a different IdP. Several are on
the [roadmap](../ROADMAP.md).
