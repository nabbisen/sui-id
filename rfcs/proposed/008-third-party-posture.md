# RFC 008 — Third-Party-Posture Bundle

**Status.** Proposed (longer-term, no scheduled delivery)
**Priority.** Low. Single-phase feature bundle; must land coherently in
one release, not piecemeal. Requires explicit owner direction before
scheduling.
**Tracks.** ROADMAP / Longer term — "Outbound-facing third-party
scenarios". Phase tag `v1-third-party-posture`.
**Touches.** New tables (`user_consent`, `scope_definition`,
`client_registration_token`); the `clients` table widened
(`registered_via`, `consent_policy`, `logo_uri`, `homepage_uri`,
`privacy_policy_uri`, `tos_uri`); new endpoints (consent screen, RFC 7591
dynamic registration); new screens (consent, "Active applications" on
`/me/security`); expanded scope policy and a localised scope catalog;
admin client-approval flow; CLI (`issue-registration-token`).

## Summary

sui-id today is designed for the *first-party* deployment model — every
registered OIDC client is an application the same operator runs. To
support the *third-party* posture (independent developers register
clients; end users authorise those clients with their own data), several
capabilities must land together as one coherent phase: a consent screen,
dynamic client registration, per-client scope policy, application
identity (logo/URLs), and a per-user refresh-token revocation surface.

This RFC frames the bundle and argues why it ships as a single release.
Shipping any subset produces a misleading product — most dangerously a
consent UI that users are trained to click through because, in the
first-party world they came from, there was never anything to consent to.

## Motivation

The first-party and third-party postures have different threat models.
First-party: the operator registers a client, hands the secret to the
app's developer (the same person), users never see consent because they
are authorising the operator's own app. Third-party: an unaffiliated
developer registers a client, sends a user to the authorize endpoint, and
the user must decide whether to grant access to *their* data. Mixing the
two without realising it is how a "trust-on-first-use" consent experience
gets shipped that users reflexively approve. The motivation is to make
sui-id genuinely safe as a third-party IdP — which requires the whole
bundle or none of it.

## Background

Today: an admin registers a client, sets `redirect_uris`, the app
authorises and gets tokens; there is no consent screen because there is
nothing to consent to. The third-party posture inverts the trust call —
tokens are issued under the *user's* authority *to the app*, not as a
deployment-internal trust decision. That inversion needs a consent
boundary, an identity for the requesting app, granular scopes the user
can reason about, a registration path for external developers, and a way
for users to revoke later.

## Target code areas

- **`sui-id-store`** — new repos `user_consent`, `scope_definition`,
  `client_registration_token`; `clients` table widening.
- **`sui-id-core`** — consent-decision logic (first-time vs broader-scope
  re-prompt); RFC 7591 registration validation; scope-catalog resolution.
- **`sui-id` handlers** — consent GET/POST, `POST /oauth2/register`,
  per-user revoke under `/me/security`, admin client-approval and
  scope-catalog pages.
- **`sui-id-web`** — consent screen, "Active applications" section,
  client-edit application-identity fields, scope-catalog admin.
- **`sui-id-i18n`** — `scope_desc_<name>` localised scope descriptions
  (compile-time exhaustive, same as all other strings).

## Security properties / invariants

- **P1 (consent screen is the security boundary).** It renders the
  *registered* client name (never anything from the request), a clear
  scope list, and defaults optional scopes to *unchecked*.
- **P2 (broader scope re-prompts).** A request for a broader scope set
  than previously granted always re-prompts, with the new scopes
  highlighted. Same-or-narrower scopes under `first_time_only` skip the
  prompt.
- **P3 (revocation propagates immediately).** Revoking a consent deletes
  the `user_consent` row and revokes all that client's refresh tokens for
  that user at once. In-flight short-lived access-token JWTs expire on
  their own clock (accepted window).
- **P4 (registration is gated).** Dynamic registration requires an
  initial access token (RFC 7591) by default. Open registration is an
  explicit opt-in, IP-rate-limited, admin-notified, and every
  dynamically-registered client starts *disabled* pending admin enable.
- **P5 (registration tokens stored hashed).** `client_registration_token`
  values are hashed, same shape as `password_reset_token`. Single-use per
  registration, multi-registration within `max_uses`, TTL-bounded.
- **P6 (application-identity URLs are validated, not fetched).** The four
  `*_uri` fields must be HTTPS (or `http://localhost` for development) and
  syntactically valid; their contents are never fetched or verified. The
  admin client-edit screen flags a `homepage_uri` domain that matches no
  `redirect_uri`.

## Non-goals

- OIDC Federation / RFC 9396 rich authorization requests (flag for future
  consideration).
- Per-field profile consent granularity beyond the scope-description
  summary (a "see more" expand is optional — see Open questions).
- Shipping any capability of this bundle independently (the whole point is
  coherence).
- Organisation or tenant modelling (orthogonal; see RFC 025).

## Proposed design

### 1. Consent screen

User sees "App X wants access to `<scope-1>`, `<scope-2>`." Approve,
refine (deselect optional scopes), or refuse.

```sql
CREATE TABLE user_consent (
    user_id          TEXT NOT NULL,
    client_id        TEXT NOT NULL,
    granted_scopes   TEXT NOT NULL,         -- space-separated
    granted_at       TEXT NOT NULL,
    last_used_at     TEXT,
    PRIMARY KEY (user_id, client_id),
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES clients (id) ON DELETE CASCADE
);
```

Two per-client policy modes (`consent_policy` column): `always_prompt`
(every authorize hits consent) and `first_time_only` (first authorize
prompts; same-or-narrower scopes thereafter use stored grants; a broader
request re-prompts, P2). Default `first_time_only`.

### 2. Dynamic client registration (RFC 7591)

`POST /oauth2/register` takes a client-metadata JSON document and returns
a fresh `client_id` (+ `client_secret` for confidential clients). Gated by
an initial access token (`Authorization: Bearer <token>`); no token → 401.

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

Optional open-registration mode (no token), behind an explicit admin
setting, IP-rate-limited; default off (P4).

### 3. Per-client scope policy refinement

```sql
CREATE TABLE scope_definition (
    name             TEXT PRIMARY KEY,    -- 'read:profile'
    requires_consent INTEGER NOT NULL,
    is_default       INTEGER NOT NULL DEFAULT 0
);
```

Each scope carries a localised description (`Strings::scope_desc_<name>`,
compile-time exhaustive). The minimum catalog ships `openid`, `profile`,
`email`, `offline_access` with their default consent posture; extending it
is an admin action.

### 4. Application identity

The four `*_uri` columns drive the consent header (logo, name, privacy
policy, terms). Validated HTTPS-or-localhost, never fetched (P6).

### 5. Refresh-token UX

`/me/security` gains an "Active applications" section: each
`(client_id, granted_scopes)` row with name, logo, scopes, last-used, and
a "Revoke access" button that deletes the consent row and revokes all the
client's refresh tokens for that user (P3). This extends the existing
session-revocation shape on the same page.

## Data model impact

Three new tables (`user_consent`, `scope_definition`,
`client_registration_token`) and six new columns on `clients`. Migrations
backfill existing clients to `registered_via='admin'`,
`consent_policy='first_time_only'`, NULL identity URLs. Existing
first-party clients keep working unchanged.

## API impact

New endpoints: `POST /oauth2/register` (RFC 7591); the consent screen
(`GET|POST /oauth2/consent`); per-user revoke under `/me/security`; admin
client-approval and scope-catalog pages. The authorize flow gains a
consent step governed by `consent_policy`. Existing first-party authorize
flows with `first_time_only` and pre-granted scopes are unaffected after
the first prompt.

## Testing strategy

A new e2e theme per capability:

- `consent.rs` — first-time prompt, second-time skip, broader-scope
  re-prompt (P2), revoke from `/me/security` (P3).
- `dynamic_registration.rs` — happy path with a valid initial access
  token, 401 without (P4), token expiry, `max_uses` exhaustion (P5).
- `scope_catalog.rs` — add/remove scopes via admin; consent rendering
  against a custom scope set.
- `application_identity.rs` — logo/URL display on consent; HTTPS
  validation at registration (P6).
- `third_party_revoke.rs` — revoke flow; post-revocation behaviour of
  refresh and access tokens.

## Migration strategy

Additive migrations create the three tables and widen `clients` with safe
defaults. No data loss. A deployment that never enables third-party
features sees `first_time_only` clients that, being first-party, were
already implicitly consented — so the consent screen does not surprise
existing first-party users (their grants are seeded as already-consented
during migration, or the operator opts a client into `always_prompt`
deliberately). Open registration stays off until explicitly enabled.

## Rollout plan

**Single release.** Internally the implementer may sequence the five
capabilities, but none ships to users until all five are present and
tested together, behind the `v1-third-party-posture` phase tag. Shipping a
subset is explicitly disallowed (see Summary / Motivation). No version
designation without owner direction and soak.

## Risks and mitigations

- *Risk:* users click through consent (trained by the first-party world).
  *Mitigation:* P1 — registered name, clear scopes, optional scopes
  unchecked; the whole bundle reframes the interaction.
- *Risk:* open registration grants ambient authority. *Mitigation:* P4 —
  default off; if on, rate-limited, admin-notified, clients start
  disabled.
- *Risk:* logo-URL phishing. *Mitigation:* P6 — render with an
  "App logo provided by the application" label; flag homepage/redirect
  domain mismatch in admin.
- *Risk:* scope creep on re-authorize. *Mitigation:* P2 — broader scope
  always re-prompts with new scopes highlighted.
- *Risk:* registration-token leakage. *Mitigation:* P5 — hashed at rest,
  TTL, `max_uses`.

## Acceptance criteria

- The consent screen renders the registered client name and a clear scope
  list, optional scopes unchecked.
- A broader-scope re-authorize re-prompts; same-or-narrower under
  `first_time_only` does not.
- Revoking a consent immediately revokes that client's refresh tokens for
  the user.
- Dynamic registration requires a valid initial access token by default;
  open registration is opt-in and starts clients disabled.
- Identity URLs are validated HTTPS-or-localhost and never fetched.
- All five capabilities ship in one release; none independently.
- 0 warnings; full suite green; all CI gates hold.

## Open questions

- A mandatory admin "client approval" step between dynamic registration
  and usability? Recommend **yes** when open registration is enabled,
  **no** for token-gated registration; make it a config knob.
- RFC 9396 rich authorization requests? **Out of scope**; flag for future.
- Should consent show *which* profile fields are read, not just "your
  basic profile"? Recommend a short scope-description summary with an
  optional "see more" expand if the deployment cares.
