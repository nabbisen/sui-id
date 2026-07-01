# RFC 004 — Federation as Upstream OIDC Relying Party

**Status.** Implemented (v0.76.4)
**Priority.** Low. Feature-expansion RFC. Requires explicit owner
direction and real-environment soak of the current core before scheduling.
**Tracks.** ROADMAP / Longer term — "Federation".
**Touches.** New module `sui-id-core::federation`; new schema
(`federation_provider`, `federation_link`); `sui-id-web` (login-screen
"Sign in with …" buttons, link/approval screens); `sui-id` (callback
handler, provider admin pages); i18n (provider button labels, link-flow
copy); config (provider secrets via env indirection).

## Summary

Add the *relying-party* (RP) side of the OIDC equation to sui-id: the
ability to authenticate a user against an upstream identity provider
(Google, Microsoft Entra, GitHub's OIDC adapter, an internal Keycloak)
and map that federated identity onto a sui-id user — either an existing
one (link mode) or a freshly provisioned one (provision-on-first-login,
opt-in per provider).

sui-id today is exclusively an OIDC *provider* (the IdP role). Federation
adds an authentication path that does not go through `credentials::verify`
at all: an upstream IdP asserts identity, sui-id trusts the assertion
within a bounded trust model, and the local user record may carry no
password.

This RFC presents a settled design shape. The policy question that keeps
it longer-term — *when may an upstream replace the local password?* — is
resolved here (provision is opt-in per provider, gated on verified email,
never auto-merges by email) so an implementer is not designing from
scratch.

## Motivation

Operators who already run an upstream IdP, or who want "Sign in with
Google" for convenience, currently cannot use sui-id without duplicating
identity. The demand is concrete and recurring. The OIDC RP mechanics are
the inverse of machinery sui-id already has for the IdP role, so the
incremental code is bounded; the value is letting sui-id participate in
an existing identity ecosystem rather than owning every credential.

The reason this is not already built: the trust-boundary design (account
linking, provision policy, MFA interaction) has more reasonable answers
than a single obvious one, and getting it wrong ships an account-takeover
vector. This RFC fixes the answers.

## Background

sui-id today owns the user table outright. A user has a username,
optionally an email, a password hash, MFA factors, and that is the full
picture. The existing `sui-id-core::oidc` machinery serves the IdP role:
authorize, token, userinfo, JWKS, discovery.

Federation introduces an inbound path: sui-id is the RP against an
upstream. The interesting part is not the protocol — that is well
understood — but the trust boundary: which upstreams are configured, how
a federated identity maps onto a local user, and what happens when the
upstream and local identities disagree (the account-takeover surface).

## Target code areas

- **`sui-id-core/src/federation.rs`** (new) — provider configuration
  loading, the authorize-redirect builder (state/nonce/PKCE), the
  callback handler core (token exchange, ID-token validation, link
  resolution), and the provision policy.
- **`sui-id-store`** — new repos `federation_provider` and
  `federation_link`; provider secret sealing reuses `crypto::seal`.
- **`sui-id/src/handlers/`** — `GET /auth/federated/{slug}/start`,
  `GET /auth/federated/callback`, `GET|POST /auth/federated/link`;
  admin provider CRUD under `/admin/federation/*`.
- **`sui-id-web`** — login-screen provider buttons (driven by enabled
  providers), the link-approval screen, the admin provider pages.
- **i18n** — provider button label template, link-flow copy, error copy.

## Security properties / invariants

- **P1 (upstream `sub` is the mapping key, never email).** A federated
  identity maps to a local user by `(provider_id, upstream_sub)`. Email is
  metadata only. An attacker who changes email at the upstream cannot
  take over a local account.
- **P2 (no auto-merge by email).** A federated identity asserting an
  email that matches an existing local user with no link to this provider
  is treated as an attempted takeover: the callback is denied, an audit
  event is recorded, and (if SMTP is configured) the legitimate user is
  notified. Linking requires explicit local authentication.
- **P3 (provision gated on verified email).** Provision-on-first-login
  honours an upstream email for new-account creation only if the upstream
  marks it `email_verified: true`. Otherwise the new account enters a
  held state requiring admin approval.
- **P4 (local MFA is not bypassed).** Federated sign-in skips the local
  password but never the local MFA challenge. The upstream's own MFA
  assertion (`amr`) is not trusted as a substitute, because the upstream's
  definition of "MFA" varies and a compromised upstream must not bypass
  local controls.
- **P5 (state/nonce/PKCE single-use).** State is HMAC'd with the master
  key, single-use, 10-minute TTL. Nonce is single-use, deleted after the
  first successful callback. PKCE verifier is per-flow.
- **P6 (no upstream token storage).** The upstream access token is used
  once (for userinfo, if needed) and discarded. sui-id never persists
  upstream tokens.
- **P7 (username never trusted from upstream).** The local username is
  derived (from `preferred_username` or email local-part) and
  conflict-resolved with a numeric suffix; an arbitrary upstream-supplied
  username is never accepted verbatim.

## Non-goals

- Group / role / attribute sync from the upstream (federation supplies
  authentication and a stable identity, nothing more).
- SCIM provisioning.
- Federated provisioning of the *first* admin (setup is offline-first;
  federation is a runtime choice — see Open questions).
- Trusting the upstream's MFA assertion as local MFA (explicitly P4).
- Storing or refreshing upstream tokens for ongoing API access.

## Proposed design

### Schema

```sql
CREATE TABLE federation_provider (
    id                   TEXT PRIMARY KEY,        -- internal UUID
    slug                 TEXT NOT NULL UNIQUE,    -- 'google', 'company-keycloak'
    display_name         TEXT NOT NULL,           -- shown on login button
    issuer               TEXT NOT NULL,           -- well-known discovery base
    client_id            TEXT NOT NULL,
    client_secret_enc    BLOB NOT NULL,           -- AAD-bound seal()
    scopes               TEXT NOT NULL,           -- space-separated, e.g. 'openid email'
    provision_mode       TEXT NOT NULL            -- 'link_only' | 'provision_on_first_login'
                         CHECK (provision_mode IN ('link_only','provision_on_first_login')),
    enabled              INTEGER NOT NULL DEFAULT 0,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

CREATE TABLE federation_link (
    user_id        TEXT NOT NULL,
    provider_id    TEXT NOT NULL,
    upstream_sub   TEXT NOT NULL,             -- the `sub` claim from upstream
    upstream_email TEXT,                      -- last-seen email; metadata only
    linked_at      TEXT NOT NULL,
    last_seen_at   TEXT NOT NULL,
    PRIMARY KEY (user_id, provider_id),
    UNIQUE (provider_id, upstream_sub),
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (provider_id) REFERENCES federation_provider (id) ON DELETE CASCADE
);
```

### Flow

The login screen lists enabled providers as buttons. "Sign in with X":

1. sui-id generates state + nonce + PKCE verifier, stashes them in a
   short-lived signed cookie (master-key HMAC, 10-minute TTL).
2. Redirects to the upstream's `authorization_endpoint`.
3. Upstream redirects back to `/auth/federated/callback?code=…`.
4. sui-id validates state/nonce, exchanges the code for tokens at the
   upstream `token_endpoint`, validates the ID token, fetches userinfo if
   needed.
5. Resolves `federation_link` by `(provider_id, upstream_sub)`:
   - **Hit** → load the local user, run the local MFA gate, mint a sui-id
     session.
   - **Miss + `provision_on_first_login`** → create a local user (no
     password) when `email_verified`, insert a `federation_link`, run MFA
     gate, sign in. Unverified email → held state, admin approval.
   - **Miss + `link_only`** → redirect to `/auth/federated/link`, where
     the user authenticates locally and approves the link.

### `amr` reflection

A federated sign-in followed by local TOTP produces an issued `amr` of
`["fed:google", "totp"]`. The federation factor is recorded; local MFA is
additive, not replaced.

## Data model impact

Two new tables (`federation_provider`, `federation_link`). No change to
existing tables: a federated user is an ordinary `users` row with no
`credentials` row (the existing nullable FK already permits this). Two
new migrations.

## API impact

New routes: `GET /auth/federated/{slug}/start`,
`GET /auth/federated/callback`, `GET|POST /auth/federated/link`, and admin
provider CRUD under `/admin/federation/*`. No change to existing IdP-role
endpoints. The issued-token `amr` claim gains `fed:{slug}` values.

## Testing strategy

- A mock upstream OIDC server (the existing `sui-id-core::oidc` machinery
  reused as an in-process upstream double).
- State / nonce / PKCE round-trip tests.
- `link_only` flow: callback → `/auth/federated/link` → local
  password + MFA confirm completes the link.
- `provision_on_first_login` happy path with verified email.
- Same path with unverified email → held-state branch.
- Federated user with local MFA: challenge enforced post-callback (P4).
- Account-takeover guard: federated identity asserting an existing
  user's email with no link → denied + audit (P2).
- Provider disabled mid-flight: callback rejected with a generic error.
- Audit events `auth.federation.link.created`,
  `auth.federation.signin.success`,
  `auth.federation.signin.upstream_failure`,
  `auth.federation.takeover_blocked`.

## Migration strategy

Two additive migrations create the new tables. No backfill — existing
users are unaffected and remain local. Enabling federation is an admin
action after upgrade. Disabling a provider blocks new sign-ins but does
not unlink existing users; full revocation also deletes the relevant
`federation_link` rows.

## Rollout plan

Implementable in three increments, each independently shippable:
(1) provider schema + admin CRUD + secret sealing; (2) the
authorize/callback flow with `link_only` only; (3) `provision_on_first_login`
plus the held-state admin-approval path. Self-service link management on
`/me/security` is a later iteration (Open questions). No version
designation without owner direction and soak.

## Risks and mitigations

- *Risk:* account takeover via email collision. *Mitigation:* P1/P2 —
  mapping is by upstream `sub`; email collisions are denied and audited.
- *Risk:* a compromised upstream bypasses local MFA. *Mitigation:* P4 —
  local MFA always enforced; upstream `amr` not trusted.
- *Risk:* open-redirect or replay via the state cookie. *Mitigation:* P5
  — HMAC'd, single-use, short TTL.
- *Risk:* provider secret leakage. *Mitigation:* `client_secret_enc` is
  AAD-bound `seal()`; rotation re-encrypts only that row.

## Acceptance criteria

- A federated identity maps to a local user by upstream `sub`, never by
  email.
- An email collision with an unlinked local user is denied and audited.
- Provision creates an account only on verified upstream email; otherwise
  held for admin approval.
- Local MFA is enforced on every federated sign-in where the user has MFA
  enrolled.
- State/nonce/PKCE are single-use; replays are rejected.
- No upstream tokens are persisted.
- 0 warnings; full suite green; all CI gates hold.

## Open questions

- "Sign in with X" on the setup wizard? Recommend **no** — setup is
  offline-first; federation is a runtime choice.
- A federated user with a forgotten local password — can they reset
  locally? Recommend **yes** — the forgot-password path is independent of
  federation links.
- Self-service link management on `/me/security` (user revokes their own
  links)? Recommend **yes, later** — out of scope for this RFC.
- Per-provider sign-in button ordering and grouping on the login screen —
  defer to implementation.
