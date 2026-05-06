# RFC 004 — Federation as upstream OIDC client

**Status.** Exploratory
**Tracks.** ROADMAP / Longer term — "Federation".
**Touches.** new module `sui-id-core::federation`, new schema
(`federation_provider`, `federation_link`), `sui-id-web` (login
screen "Sign in with …" buttons), `sui-id` (callback handler).

## Summary

Add the *other* side of the OIDC equation: sui-id as a relying
party against an upstream IdP (Google, Microsoft Entra, GitHub via
its OIDC adapter, an internal Keycloak, …). On a successful
upstream sign-in, the federated identity is mapped to a sui-id
user — either an existing one (link mode) or a freshly created
one (provision-on-first-login mode, opt-in).

This is a "longer term, less certain" item because the policy
question — when do you let the upstream replace the local
password? — has more reasonable answers than design ones, and
the answer changes the schema. This RFC sketches a shape; expect
a follow-up pass before implementation.

## Background

sui-id today owns the user table outright. A user has a username,
optionally an email, a password hash, MFA factors, and that's the
full picture. Federation introduces a new authentication path that
doesn't go through `credentials::verify` at all: an upstream IdP
asserts identity, sui-id trusts the assertion, the local user
record may not even have a password.

The interesting part isn't the OIDC mechanics — those are well
understood and the existing `sui-id-core::oidc` machinery for
serving the IdP role is mostly the inverse of what's needed for
the RP role. The interesting part is the trust boundary: which
upstreams are configured, how a federated identity maps onto a
local user, and what happens when the upstream and local
identities disagree.

## Design (sketch)

### Schema

```sql
CREATE TABLE federation_provider (
    id                   TEXT PRIMARY KEY,        -- internal UUID
    slug                 TEXT NOT NULL UNIQUE,    -- 'google', 'company-keycloak'
    display_name         TEXT NOT NULL,           -- shown on login button
    issuer               TEXT NOT NULL,           -- well-known discovery base
    client_id            TEXT NOT NULL,
    client_secret_enc    BLOB NOT NULL,           -- AAD-bound
    scopes               TEXT NOT NULL,           -- space-separated, e.g. 'openid email'
    provision_mode       TEXT NOT NULL            -- 'link_only' | 'provision_on_first_login'
                         CHECK (provision_mode IN ('link_only','provision_on_first_login')),
    enabled              BOOLEAN NOT NULL DEFAULT 0,
    created_at           TIMESTAMP NOT NULL,
    updated_at           TIMESTAMP NOT NULL
);

CREATE TABLE federation_link (
    user_id        TEXT NOT NULL,
    provider_id    TEXT NOT NULL,
    upstream_sub   TEXT NOT NULL,             -- the `sub` claim from upstream
    upstream_email TEXT,                      -- last-seen email; metadata only
    linked_at      TIMESTAMP NOT NULL,
    last_seen_at   TIMESTAMP NOT NULL,
    PRIMARY KEY (user_id, provider_id),
    UNIQUE (provider_id, upstream_sub),
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (provider_id) REFERENCES federation_provider (id) ON DELETE CASCADE
);
```

### Flow

Login screen lists enabled providers as buttons. Clicking
"Sign in with X":

1. sui-id generates state + nonce + PKCE verifier, stashes them
   in a short-lived signed cookie.
2. Redirects to the upstream's `authorization_endpoint` with the
   stashed values.
3. Upstream redirects back to `/auth/federated/callback?code=…`.
4. sui-id validates state/nonce, swaps code for tokens at the
   upstream's `token_endpoint`, fetches userinfo if needed.
5. Looks up `federation_link` by `(provider_id, upstream_sub)`.
   - Hit: load the local user, mint a sui-id session.
   - Miss + `provision_mode = 'provision_on_first_login'`:
     create a local user (no password), insert a
     `federation_link`, sign in.
   - Miss + `provision_mode = 'link_only'`: redirect to
     `/auth/federated/link` where the user signs in with their
     local credentials and approves the link.

### Provision-on-first-login policy

The risky path. Two safeguards:

- **Email verification trust.** Only honour upstream-provided
  emails for new-account creation if the upstream marks them
  verified (the standard `email_verified: true` claim). Without
  it, provision goes to a held state requiring admin approval.
- **Username derivation.** Default: derive from the upstream's
  `preferred_username` claim, fall back to local-part of email,
  conflict-resolve with a numeric suffix. Never accept an
  arbitrary username from the upstream.

### MFA after federation

Federated sign-in skips local password but does *not* skip local
MFA. If the user has local TOTP or a passkey enrolled, the
post-federation flow drops them on the existing MFA challenge
screen. The `amr` claim issued by sui-id reflects this:
`["fed:google", "totp"]` is a valid combination.

The upstream's own MFA assertion (if signalled in the upstream's
ID token's `amr`) is *not* trusted as a substitute for local MFA.
Two reasons: it's hard to verify intent (the upstream's
definition of "MFA" varies) and it lets a compromised upstream
bypass local controls.

## Tests (sketch)

- Mock upstream OIDC server for integration tests. The existing
  `sui-id-core::oidc` machinery should be reusable here as an
  in-process upstream double.
- State / nonce / PKCE round-trip tests.
- `link_only` flow: callback hits `/auth/federated/link`, local
  password+MFA confirm completes the link.
- `provision_on_first_login` happy path with verified email.
- Same path with unverified email: held-state branch.
- Federated user with local MFA: challenge enforced post-callback.
- Provider disabled mid-flight: callback rejected with a
  generic error.
- `auth.federation.link.created`, `auth.federation.signin.success`,
  `auth.federation.signin.upstream_failure` audit events.

## Security considerations

- **Open redirect via state cookie.** State is HMAC'd with the
  master key, single-use, expires in 10 minutes.
- **Nonce reuse.** Single-use, deleted after the first
  successful callback. Replays rejected.
- **Upstream email taken-over.** A federated identity that
  asserts an email matching an existing local user with a
  different upstream link is treated as an attempted account
  takeover: callback is denied, audit event recorded, the
  legitimate user receives a notification email if SMTP is
  configured.
- **Upstream `sub` immutability.** The mapping key is
  `(provider_id, upstream_sub)`, *not* email. Email changes
  upstream-side don't break links; an attacker can't take
  over by changing email at the upstream side.
- **Token leakage.** The upstream's access token is used once
  (if at all, for userinfo) and discarded. We don't store
  upstream tokens.
- **Provider rotation.** Editing a provider's `client_secret`
  re-encrypts only that row; existing links continue to work.
  Disabling a provider blocks new sign-ins but does not
  unlink — an admin who wants to fully revoke would also
  delete the rows from `federation_link`.

## Open questions

- Provision-on-first-login default: *off* per provider, opt-in
  per provider via `provision_mode` column.
- "Sign in with X" button placement on the login screen vs.
  on the setup wizard (federated provisioning of the first
  admin?). Recommend: not on setup wizard. Setup is offline-
  first; federation is a runtime choice.
- A user with a federation link but a forgotten local
  password — can they reset locally? Recommend: yes, the
  forgot-password path doesn't care about federation links.
  Federation is a sign-in path; password reset is a separate
  capability.
- Should federation entries be visible on `/me/security` for
  the user to revoke their own links? Recommend: yes, in a
  later iteration. Out of scope for this RFC.
