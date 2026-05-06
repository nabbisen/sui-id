# RFC 008 — Third-party-posture bundle

**Status.** Exploratory
**Tracks.** ROADMAP / Longer term — "Outbound-facing-third-party
scenarios".
**Touches.** new tables (`user_consent`, `client_metadata`,
`client_registration_token`), new endpoints (RFC 7591 dynamic
client registration), new screens (consent, "Active
applications" on `/me/security`), expanded scope policy,
existing `clients` table widened.

## Summary

sui-id today is designed for the *first-party* deployment model
— every registered OIDC client is an application the same
operator runs. To support the *other* posture (third-party
developers register clients, end users authorise those clients
with their own data), several capabilities have to land
together as a single coherent phase. This RFC frames the bundle.

The ROADMAP entry for this work is explicit that splitting the
bundle across releases produces a worse story than not shipping
it at all (a half-built consent UI is more confusing than no
consent UI). Treat this RFC as one phase tagged
`v1-third-party-posture`, shipping in one release once it's
all designed and tested.

## Background

The first-party posture today: an admin registers a client,
sets `redirect_uris`, hands the client_id/secret to the app's
developer (who is the same person), the app authorises and
gets tokens. Users never see a consent screen; there's nothing
to consent to — they're authorising the operator's own app.

The third-party posture: a developer not affiliated with the
sui-id deployment registers a client (ideally self-service),
their app sends a user to sui-id's authorise endpoint, the
user sees "App X wants access to scopes Y and Z" and decides.
Tokens are issued under the user's authority *to the app*,
not as a deployment-internal trust call.

These postures have different threat models. Mixing them
without realising it is bad — it's how you ship a
"trust-on-first-use" consent experience that users
click through because they've been trained that there's
nothing to consent to. Hence one-bundle.

## Capabilities (each must land in the bundle)

### 1. Consent screen

User sees `App X wants access to <scope-1>, <scope-2>`. Approve,
refine (deselect optional scopes), or refuse.

Schema:

```sql
CREATE TABLE user_consent (
    user_id          TEXT NOT NULL,
    client_id        TEXT NOT NULL,
    granted_scopes   TEXT NOT NULL,         -- space-separated
    granted_at       TIMESTAMP NOT NULL,
    last_used_at     TIMESTAMP,
    PRIMARY KEY (user_id, client_id),
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES clients (id) ON DELETE CASCADE
);
```

Two policy modes per client (column on `clients`, see below):

- `consent_policy = 'always_prompt'` — every authorise hits
  the consent screen.
- `consent_policy = 'first_time_only'` — first authorise hits
  the screen, subsequent authorises with the same-or-narrower
  scopes use the stored `granted_scopes`. A *broader* scope
  request always re-prompts.

Default: `first_time_only` for new clients.

The `/me/security` page gains an "Active applications"
section listing each `(client_id, granted_scopes)` row with a
"Revoke" button. Revocation deletes the row and revokes all
that client's outstanding refresh tokens for that user.

### 2. Dynamic client registration (RFC 7591)

A POST endpoint at `/oauth2/register` that takes a JSON
client metadata document and returns a freshly-minted
`client_id` + `client_secret` (or just `client_id` for public
clients).

Gating: an *initial access token* model. The deployment admin
generates a registration token (`sui-id admin
issue-registration-token --max-uses 5 --expires-in 1d`); the
developer presents it as `Authorization: Bearer <token>`. No
token = 401. This is the standard RFC 7591 shape.

Tokens are single-use-per-registration but multi-registration
within their `max_uses`. They live in
`client_registration_token` with a hashed value column and
TTL.

Optional second mode: open registration (no token required),
behind an explicit admin setting and rate-limited per IP.
Recommend default off.

The schema gain on `clients`:

```sql
ALTER TABLE clients ADD COLUMN registered_via TEXT NOT NULL DEFAULT 'admin'
    CHECK (registered_via IN ('admin','dynamic'));
ALTER TABLE clients ADD COLUMN consent_policy TEXT NOT NULL DEFAULT 'first_time_only'
    CHECK (consent_policy IN ('always_prompt','first_time_only'));
ALTER TABLE clients ADD COLUMN logo_uri TEXT;
ALTER TABLE clients ADD COLUMN homepage_uri TEXT;
ALTER TABLE clients ADD COLUMN privacy_policy_uri TEXT;
ALTER TABLE clients ADD COLUMN tos_uri TEXT;
```

### 3. Per-client scope policy refinement

Today `allowed_scopes` is a flat space-separated list. For the
consent screen to be meaningful, scopes need:

- **Sub-resource scopes.** `read:profile` vs `write:profile`,
  `read:email` standalone, etc. The deployment defines a
  scope catalog.
- **Human descriptions.** Each scope has a localised
  short description (`"Read your basic profile"`,
  `"Send email on your behalf"`) that the consent screen
  renders. The catalog lives in `sui-id-i18n` —
  `Strings::scope_desc_<name>` — same compile-time
  exhaustiveness story.

Scope catalog schema:

```sql
CREATE TABLE scope_definition (
    name             TEXT PRIMARY KEY,    -- 'read:profile'
    requires_consent BOOLEAN NOT NULL,
    is_default       BOOLEAN NOT NULL DEFAULT 0
);
```

The minimum catalog ships with `openid`, `profile`, `email`,
`offline_access` and their default consent posture. Extending
the catalog is an admin action.

### 4. Application identity

The four `*_uri` columns above. The consent screen renders:

```
<App logo>
<App name> wants access to your account.
<Privacy policy> · <Terms of service>

This app will be able to:
  ☑ Read your basic profile
  ☑ Read your email address
  ☐ Send email on your behalf

[ Refuse ]                              [ Allow ]
```

The four URLs are validated at registration time: must be
HTTPS (or http://localhost for development), must be
syntactically valid, but their *contents* are not fetched or
verified. The admin tooling shows them in the client edit
screen for review.

### 5. Refresh-token UX

Long-lived refresh tokens against third-party clients need a
per-user revocation surface. The "Active applications" section
on `/me/security` (item 1 above) is the place for this.

Each row shows:

- Client name + logo
- Scopes granted
- "Last used" timestamp
- "Revoke access" button → deletes the consent row, revokes
  all refresh tokens, the next time the app tries to use
  any token it gets a fresh authorise prompt (with consent
  re-required).

This already exists for sessions on `/me/security`; we extend
the same shape for clients.

## Why this is one bundle, not five releases

Shipping any subset produces a misleading product:

- Consent screen alone: confusing because the same admin
  registered the clients, so the screen feels like
  ceremonial paperwork, training users to click through.
- Dynamic registration alone: third-party developers can
  now register clients but users have no consent step, so
  any registered client immediately gets ambient access
  authority.
- Per-client scope policy alone: more granular scopes that
  no UI surfaces.
- Application identity alone: logo/homepage fields with no
  consent screen to render them on.
- Refresh-token UX alone: useful for first-party clients
  too but doesn't unlock third-party usage on its own.

The whole bundle changes sui-id's posture in a coherent way.
Either ship it or don't.

## Tests (sketch)

For each capability, the existing e2e harness gets a new
`tests/e2e/<theme>.rs`:

- `consent.rs` — first-time prompt, second-time skip, broader
  scope re-prompt, revoke from `/me/security`.
- `dynamic_registration.rs` — happy path with valid initial
  access token, 401 without, expiry, max-uses exhaustion.
- `scope_catalog.rs` — adding/removing scopes via admin,
  consent-screen rendering against custom scope set.
- `application_identity.rs` — logo/URL display on consent,
  HTTPS validation at registration.
- `third_party_revoke.rs` — revoke flow, post-revocation
  behaviour of refresh and access tokens.

## Security considerations

- **Consent screen UX is the security boundary.** It must
  render the *registered* client name (not anything from
  the request). It must render a clear scope list. It must
  default the optional checkboxes to *unchecked*.
- **Open registration danger.** Open registration without
  an initial access token is convenient and dangerous.
  Default off; if enabled, IP rate-limited and admin-
  notified on every registration. Each registered client
  starts disabled and requires an admin to enable before
  any authorise call works.
- **Logo URL phishing.** A third-party-controlled image at
  `logo_uri` could be visually misleading. Mitigations:
  the image is rendered with a clear "App logo provided
  by the application" label; the admin client-edit screen
  flags clients whose `homepage_uri` domain doesn't match
  any `redirect_uri`.
- **Scope creep on re-authorise.** A request for a broader
  scope set than previously granted *must* re-prompt the
  user, with the *new* scopes highlighted.
- **Revocation propagation.** Revoking a consent must
  immediately revoke all that client's refresh tokens for
  that user; in-flight access tokens (which are short-lived
  JWTs) expire on their own clock — we accept that
  short-lived window.
- **Initial access token storage.** Hashed in
  `client_registration_token`, not plain. Same shape as
  `password_reset_token`.

## Open questions

- Should we ship a "client approval" admin step between
  dynamic registration and the client being usable?
  Recommend yes for any deployment with open registration
  enabled, no for token-gated registration. Make it a
  config knob.
- Do we ever support OIDC Federation in the sense of RFC
  9396 (rich authorization requests) or beyond? Out of
  scope for this RFC; flag for future consideration.
- Does the consent screen need to show *which* fields of
  the user's profile would be read, not just "your basic
  profile"? Recommend a short summary in the scope
  description, with a "see more" expand if the deployment
  cares.
