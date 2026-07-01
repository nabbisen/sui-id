# RFC 005 — Pluggable User Backends (Read-Only LDAP User-Source)

**Status.** Implemented (v0.76.1)
**Priority.** Low. Feature-expansion RFC. Best driven by a concrete
deployment's requirements; requires explicit owner direction before
scheduling.
**Tracks.** ROADMAP / Longer term — "Pluggable user backends".
**Touches.** New trait module `sui-id-core::user_source`; an LDAP
implementation behind a feature flag (or a sibling crate); schema
addition (`users.source` discriminator, `users.external_stable_id`);
auth-handler cascade in `sui-id`; config (`[[user_source]]` blocks with
env-indirected bind secret).

## Summary

Some operators run an existing directory (Active Directory, OpenLDAP) and
want sui-id to authenticate against it instead of sui-id's local user
table. The current storage layer assumes sui-id owns the user table
outright; this RFC proposes a *read-only* LDAP user-source plug-in shape
that lets the local table fall back to the directory for users it does not
know.

The scope is deliberately the *minimum*: pure authentication and a stable
identity. No group membership sync, no SCIM, no write-back. LDAP users get
a local *shadow* row in `users` (for sessions, MFA, audit linkage) created
on first successful bind and never holding a password.

## Motivation

sui-id's value proposition is "the simple thing for small deployments."
An operator with an existing directory should not have to choose between
sui-id and their directory; a bounded read-only auth-source lets sui-id
sit in front of the directory without owning credentials. The bounded
scope is the point — every capability beyond authentication (groups,
write-back, SCIM) adds config, operational, and security surface that
would erode the simplicity that makes sui-id worth choosing.

## Background

LDAP-as-an-auth-source has well-known sharp edges: referrals, group
nesting, paged-search semantics, DN-vs-uid identity ambiguity. The design
here sidesteps all of them by doing the least possible:

- The LDAP backend is *read-only*. sui-id never writes to the directory.
- It supplies *authentication* and a stable identity, nothing else.
- LDAP-authenticated users have a *shadow* row in `users` for sessions,
  MFA, and audit linkage. The shadow is created on first successful bind
  and never holds a password (the existing nullable `credentials` FK
  already supports a password-less user).

## Target code areas

- **`sui-id-core/src/user_source.rs`** (new) — the `UserSource` trait and
  `ExternalUserRecord` type; the cascade resolver that tries local first,
  then configured sources in order.
- **LDAP implementation** — behind a `ldap` feature flag (or a sibling
  crate `sui-id-ldap`), using a vetted async LDAP client; RFC 4515 filter
  escaping; search-then-bind with fixed-cost search.
- **`sui-id-store`** — migration adding `users.source` and
  `users.external_stable_id`; shadow-row creation/update repo methods.
- **`sui-id` auth handlers** — `/admin/login` and `/oauth2/authorize`
  resolve usernames through the cascade.
- **config** — `[[user_source]]` blocks; bind secret via
  `bind_password_env` indirection.

## Security properties / invariants

- **P1 (DN injection prevented).** The `user_search_filter` admits exactly
  one substitution `{username}`, escaped per RFC 4515. No DN templating.
- **P2 (TLS required).** Cleartext `ldap://` is rejected at config load.
  Only `ldaps://` or `ldap://` + STARTTLS is accepted. Anonymous bind is
  disallowed at config load.
- **P3 (timing equivalence).** `authenticate` returns `Ok(None)` for both
  "unknown user" and "wrong password," and the implementation spends
  roughly equal time on both branches (search-then-bind with a fixed-cost
  search even on a miss). This matches the existing local-auth
  dummy-Argon2id posture.
- **P4 (local admin is always reachable).** The cascade is local-first and
  hardcoded. A flaky or misconfigured directory never locks out the local
  admin — a transport error from a source is logged and the cascade
  continues (fail-soft).
- **P5 (no password ever stored for LDAP users).** A `source='ldap'` user
  has no `credentials` row. sui-id sees the password only transiently
  during the bind and never persists it, hashed or otherwise.
- **P6 (least-privilege service account).** Documentation requires the
  `bind_dn` account to have read-only access to the user-search subtree
  and nothing more.

## Non-goals

- Write-back of any kind (password change, attribute update).
- Group / role / membership sync.
- SCIM.
- Configurable cascade order (local-first is hardcoded — see P4).
- Running HIBP on LDAP-sourced passwords (sui-id does not hold the
  password long enough, and refusing an LDAP-managed password is not
  sui-id's call — see Open questions).

## Proposed design

### Trait

```rust
// sui-id-core/src/user_source.rs

pub trait UserSource: Send + Sync {
    /// `Ok(Some(record))` if the source authenticates the credentials,
    /// `Ok(None)` if the username is unknown to this source OR the
    /// password is wrong (deliberately conflated to defeat timing
    /// distinction), and `Err(_)` only for transport-layer issues
    /// (directory unreachable).
    fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<ExternalUserRecord>, UserSourceError>;
}

pub struct ExternalUserRecord {
    pub stable_id:        String,           // DN or objectGUID, never re-used
    pub display_username: String,           // for the local shadow row
    pub email:            Option<String>,
    pub display_name:     Option<String>,
}
```

### Schema

Migration `00NN_users_source.sql`:

```sql
ALTER TABLE users ADD COLUMN source TEXT NOT NULL DEFAULT 'local'
    CHECK (source IN ('local','ldap'));
ALTER TABLE users ADD COLUMN external_stable_id TEXT;

CREATE UNIQUE INDEX idx_users_external
    ON users (source, external_stable_id)
    WHERE external_stable_id IS NOT NULL;
```

### Auth cascade

`/admin/login` and `/oauth2/authorize` resolve a username through:

1. **Local** — the existing `credentials::verify` path.
2. **Configured `UserSource` instances**, in declaration order.

The first source returning `Ok(Some(_))` wins. A source returning
`Err(_)` (transport failure) is logged and the cascade continues
(fail-soft, P4). A first successful LDAP authentication for an unknown
username triggers shadow-user creation; a subsequent sign-in for the same
`external_stable_id` updates the shadow's display fields if the upstream
changed them.

### MFA over an LDAP-sourced user

MFA is a sui-id concern, not LDAP's. An LDAP user enrols TOTP and passkeys
against their shadow row exactly like a local user; the challenge screen
is unchanged.

### Configuration

```toml
[[user_source]]
kind = "ldap"
slug = "company-ldap"
url  = "ldaps://ldap.example.com:636"
bind_dn = "cn=sui-id-svc,ou=services,dc=example,dc=com"
bind_password_env = "SUIID_LDAP_BIND_PW"          # never inline
user_search_base = "ou=people,dc=example,dc=com"
user_search_filter = "(uid={username})"           # {username} is the sole substitution
stable_id_attribute = "objectGUID"                # or "entryUUID" or DN
display_name_attribute = "cn"
email_attribute = "mail"
connect_timeout_secs = 5
search_timeout_secs = 10
```

`bind_password_env` indirection is non-negotiable: secrets never go in the
config file as plaintext.

### Failure modes

- **Directory unreachable** → cascade continues; emit
  `auth.user_source.transport_failure`.
- **Service-account bind fails** → same as above; operator-visible via
  the audit log.
- **`stable_id` disagrees with an existing shadow** → treat as a stale
  shadow: rebind under the new `stable_id`, flag the old shadow as
  "potential conflict" for admin attention. Never auto-merge.

## Data model impact

Two new columns on `users` (`source`, `external_stable_id`) plus a partial
unique index. No new tables. One migration. Existing rows default to
`source='local'`.

## API impact

No new routes. The behaviour of `/admin/login` and `/oauth2/authorize`
changes internally (the cascade), but the request/response contract is
unchanged. New audit events `auth.user_source.matched` and
`auth.user_source.transport_failure`.

## Testing strategy

- An in-memory LDAP fixture (a vetted test server crate or a hand-rolled
  async stub).
- Cascade order: a local user wins over an LDAP user with the same
  username.
- Shadow creation: first LDAP sign-in creates a `users` row with
  `source='ldap'`.
- Re-sign-in: shadow display fields update when the upstream changes.
- MFA over LDAP: TOTP enrol, sign in via LDAP, challenge appears,
  succeeds.
- Transport failure: directory unreachable, local users still sign in
  (P4).
- Timing: search-miss and wrong-password branches spend comparable time
  (P3).
- Filter escaping: a username containing LDAP filter metacharacters does
  not alter the search filter (P1).
- TLS guard: `ldap://` cleartext config is rejected at load (P2).

## Migration strategy

One additive migration. Existing users default to `source='local'`; no
behavioural change for deployments that configure no `[[user_source]]`.
LDAP is opt-in by configuration after upgrade. Removing the configuration
stops new LDAP sign-ins; existing shadow rows remain (they are ordinary
`users` rows) until an admin deletes them.

## Rollout plan

Two increments: (1) the `UserSource` trait, the cascade, and the schema —
with a trivial in-memory test source, no LDAP yet; (2) the LDAP
implementation behind a feature flag. Default builds and the default
deployment story are unchanged at every step. No version designation
without owner direction and soak.

## Risks and mitigations

- *Risk:* DN/filter injection. *Mitigation:* P1 — single escaped
  substitution, no DN templating.
- *Risk:* cleartext credential exposure. *Mitigation:* P2 — TLS required,
  anonymous bind disallowed.
- *Risk:* a flaky directory locks out the local admin. *Mitigation:* P4 —
  local-first, fail-soft cascade.
- *Risk:* surprising shadow-row removal behaviour (deleting a shadow
  resets MFA state on next sign-in). *Mitigation:* document loudly; it is
  correct but counter-intuitive.

## Acceptance criteria

- Local users always resolve before any external source.
- An LDAP-authenticated unknown username creates a password-less shadow
  row.
- LDAP users can enrol and use local MFA.
- A directory outage does not affect local sign-in.
- Wrong-password and unknown-user are timing-indistinguishable.
- Cleartext LDAP config is rejected at load.
- 0 warnings; full suite green; all CI gates hold.

## Open questions

- Cascade order configurable, or always local-first? Recommend
  **local-first, hardcoded** — the local admin is the escape hatch.
- Write support ever? Recommend **no** — any deployment needing it is out
  of scope; SCIM is a much larger conversation.
- HIBP over LDAP passwords? Recommend **no** — sui-id does not hold the
  password long enough, and refusing an LDAP-managed password is not
  sui-id's decision.
- Shadow-row removal UX — should the admin UI warn that deletion resets
  MFA state? Recommend yes; defer exact copy to implementation.
