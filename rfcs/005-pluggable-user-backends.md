# RFC 005 — Pluggable user backends (LDAP shim)

**Status.** Exploratory
**Tracks.** ROADMAP / Longer term — "Pluggable user backends".
**Touches.** new trait module `sui-id-core::user_source`, new
crate or feature flag for the LDAP implementation, schema
addition (`users.source` discriminator).

## Summary

Some operators run an existing directory (Active Directory,
OpenLDAP) and want sui-id to authenticate against it instead of
sui-id's local user table. The current storage layer assumes
sui-id owns the user table outright; this RFC proposes a
read-only LDAP user-source plug-in shape that lets the local
table fall back to the directory for users it doesn't know.

This is exploratory because LDAP-as-an-auth-source has well-known
sharp edges (referrals, group nesting, paged search semantics,
DN-vs-uid identity ambiguity), and the design is best driven by
a concrete deployment's requirements. The shape proposed here is
the *minimum* — pure authentication, no group sync, no SCIM.

## Background

sui-id's value proposition is "the simple thing for small
deployments." The moment we add a user backend, we add a config
surface, an operational surface (what happens when the LDAP
server is unreachable), and a security surface (DN injection,
unbound search). All of these need to stay deliberately bounded
to keep that value proposition.

Concrete shape this RFC defends against:

- The LDAP backend is *read-only*. We don't write back, ever.
- The LDAP backend supplies *authentication* and a stable
  identity, nothing else. No group membership sync. No SCIM.
- Users authenticated via LDAP have a *shadow* row in `users`
  for the purposes of sessions, MFA, audit linkage. The shadow
  is created on first successful LDAP bind and never holds a
  password.

## Design (sketch)

### Trait

```rust
// sui-id-core/src/user_source.rs

pub trait UserSource: Send + Sync {
    /// Returns `Ok(Some(record))` if the source authenticates
    /// the credentials, `Ok(None)` if the username is unknown to
    /// this source, and `Err(_)` only for transport-layer issues
    /// (e.g. directory unreachable). Wrong-password is
    /// `Ok(None)`, deliberately conflated with unknown-user, so
    /// that callers cannot distinguish them via timing.
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

Migration `0021_users_source.sql`:

```sql
ALTER TABLE users ADD COLUMN source TEXT NOT NULL DEFAULT 'local'
    CHECK (source IN ('local','ldap'));
ALTER TABLE users ADD COLUMN external_stable_id TEXT;

CREATE UNIQUE INDEX idx_users_external
    ON users (source, external_stable_id)
    WHERE external_stable_id IS NOT NULL;
```

A user with `source='ldap'` has no row in `credentials` (the
existing nullable foreign key already supports this); their
password is *never* stored locally, even hashed.

### Auth handler change

`/admin/login` and `/oauth2/authorize` resolve a username
through a *cascade* of sources:

1. Local: existing `credentials::verify` path.
2. Configured `UserSource` instances, in declaration order.

The first source that returns `Ok(Some(_))` wins. A source
returning `Err(_)` (transport failure) is logged and the
cascade continues — fail-soft, so a flaky LDAP server doesn't
take out local sign-in for the local admin.

A successful LDAP authentication for a username sui-id has
not seen before triggers shadow-user creation: insert a row
into `users` with `source='ldap'`, `external_stable_id =
record.stable_id`, no `credentials` row.

A second sign-in for the same `external_stable_id` updates
the shadow's display fields if the upstream changed them.

### MFA over an LDAP-sourced user

MFA is a sui-id concern, not LDAP's. An LDAP user can enrol
TOTP and passkeys against their shadow row exactly like a
local user. The challenge screen doesn't change.

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

The `bind_password_env` indirection is non-negotiable: secrets
do not go in the config file plaintext.

### Failure modes

- **Directory unreachable.** Cascade continues to next source;
  failed source emits `auth.user_source.transport_failure`.
- **Directory bind fails (sui-id's own service-account
  credentials wrong).** Same as above. Operator-visible alert
  via the audit log.
- **User exists in LDAP but objectGUID disagrees with shadow.**
  Treat as a stale shadow: rebind under the new stable_id,
  flag the old shadow as "potential conflict" for admin
  attention. Do not auto-merge.

## Tests (sketch)

- In-memory LDAP server fixture (existing crates like
  `simple-ldap-server-test` or a hand-rolled async stub).
- Cascade order test: local user wins over LDAP user with the
  same username.
- Shadow creation: first LDAP sign-in creates a `users` row
  with `source='ldap'`.
- Re-sign-in: shadow display fields update if upstream changes.
- MFA over LDAP: TOTP enrol, sign in via LDAP, MFA challenge
  appears, succeeds.
- Transport failure: LDAP unreachable, local users still sign
  in fine.
- Audit: `auth.user_source.matched`,
  `auth.user_source.transport_failure` events recorded.

## Security considerations

- **DN injection.** The `user_search_filter` admits exactly
  one substitution `{username}`, and the substitution
  escapes LDAP filter metacharacters per RFC 4515. No
  DN-templating.
- **TLS required.** `ldap://` (cleartext) is rejected at
  config load with a clear error. `ldaps://` or `ldap://` +
  STARTTLS only.
- **Service account least-privilege.** Documentation
  emphasises that the `bind_dn` user needs read-only access
  to the user-search subtree; nothing more.
- **Anonymous bind.** Disallowed at config load.
- **Password timing.** `authenticate` returns `Ok(None)` for
  both "unknown user" and "wrong password". Implementations
  must take care to spend roughly equal time on both
  branches; the LDAP shim should issue a search-then-bind
  flow with a fixed-cost search even when the search misses.
  This matches the existing local-auth dummy-Argon2id
  posture.
- **Shadow row removal.** When an admin deletes an LDAP
  shadow, the next sign-in by that LDAP user re-creates a
  *new* shadow with new MFA state. Document this loudly —
  it's the correct behaviour but surprising.

## Open questions

- Is the cascade order configurable, or always local-first?
  Recommend local-first, hardcoded. The local admin is the
  sui-id-itself escape hatch and must be addressable even
  if every external source is misconfigured.
- Do we ever need *write* support? Recommend no, and treat
  any deployment that needs it as out of scope. SCIM is a
  much bigger conversation.
- How does the LDAP source interact with HIBP? Recommend:
  HIBP is not run on LDAP-sourced passwords (we don't see
  the password long enough to hash and submit, and even if
  we did, refusing to authenticate against an LDAP-managed
  password isn't sui-id's call to make).
