# RFC 025 — Multi-Tenant Expansion Path: Detailed Design

**Status.** Proposed (longer-term, no scheduled delivery)
**Priority.** Low. Detailed-design RFC for an expansion that is
realistically in scope but is not in a release queue. Its purpose is not
to schedule the work but to ensure that *if* the work happens, the design
is settled enough that an implementer is not building it from scratch, and
that present-day decisions (RFC 022's single-realm scope statement,
current schema choices) are made with full knowledge of the path forward.
A schedule requires explicit owner direction.
**Tracks.** v0.29.5 data-model review-2 §3, §4, §6 (case B). Supersedes
[RFC 007 (Multi-tenancy)](../archive/007-multi-tenancy.md), which had the
topic but not the detail.
**Touches.** Substantial. Schema changes propagate to nearly every table;
routing changes affect the issuer URL space; admin authorisation gains a
dimension (global admin vs tenant admin); audit logging gains tenant
scope; setup flow gains a "create first tenant" step.

## Summary

sui-id is single-realm by design and by RFC 022. There is a real,
articulated demand profile for a multi-tenant shape — small/medium SaaS
deployments that want an isolated IdP per customer without running N
processes, where Keycloak is too much machinery and Auth0 is too much SaaS
dependency.

This RFC describes what that expansion looks like inside sui-id's design
philosophy. It is a *detailed design*, not a schedule. Its job is to keep
the design on paper so that present-day decisions can account for the
eventual shape, so that RFC 022 can reference a real expansion path, and
so that an implementer reads one RFC instead of designing from scratch. If
the owner later decides not to pursue multi-tenancy, this RFC moves to
`archive/` with status `Withdrawn`.

## Motivation

A half-detailed multi-tenancy RFC invites premature compromise — a future
PR adds `tenant_id TEXT NULL DEFAULT 'default'` to `users` and calls it
tenancy, which is worse than no tenancy because it breaks invariants
without delivering isolation. Carrying enough detail (schema, routing,
admin authorisation, migration) lets a reader see the full shape and tell
when a partial change is moving toward it or away from it. The motivation
is therefore as much *defensive* (prevent piecemeal tenancy) as it is
forward-looking.

## Background: what's there today

The single-realm shape (RFC 022):

```
users(id, username, email, ...)              -- flat
clients(id, name, redirect_uris, ...)        -- flat
sessions(user_id, ...)
refresh_tokens(user_id, client_id, ...)
audit_log(actor, target, ...)                -- flat
server_settings(id = 'singleton', ...)       -- one config
smtp_config(id = 'singleton', ...)           -- one config
```

One issuer URL (the deployment URL), one JWKS, one admin role.

## Target code areas

- **Schema** — a `tenants` table; `tenant_id` on tenant-bearing tables
  (`users`, `clients`, `audit_log`); singleton config tables become
  per-tenant (`tenant_settings`) plus a new `global_settings` singleton;
  `signing_keys.tenant_id`; admin-role columns
  (`is_global_admin`, `is_tenant_admin`) replacing `is_admin`.
- **Routing** — slug-extraction middleware; tenant-prefixed URL space;
  `_global` reserved scope for global-admin routes.
- **`sui-id-core`** — tenant-scoped authorize/token/userinfo; per-tenant
  signing-key resolution; global-admin "assume tenant" flow.
- **`sui-id` handlers** — tenant admin pages under `/{slug}/admin/*`;
  global admin pages under `/_global/admin/*`; tenant CRUD.
- **Setup** — two-stage bootstrap (deployment, then first tenant).
- **Migration** — the single-realm → multi-tenant one-shot upgrade.

## Security properties / invariants

The fundamental concern is **cross-tenant data access**.

- **P1 (slug is strictly validated).** The routing middleware rejects any
  slug not matching `^[a-z0-9](-?[a-z0-9])*$`. A traversal slug like
  `../other` 404s before reaching tenant logic.
- **P2 (token confusion prevented).** A token issued for tenant A sent to
  a tenant B endpoint is rejected — per-tenant JWKS (design §5 option A)
  plus per-endpoint `iss`-claim verification.
- **P3 (cookie/CSRF tenant scoping).** Session cookies are tenant-scoped
  by path (`Path=/{slug}/`); the cookie name does not encode the tenant.
  CSRF tokens are session-bound and therefore tenant-bound.
- **P4 (global-admin assumption is audited).** A global admin has no
  tenant-data access without explicitly *assuming* a tenant; the
  assumption is logged, and the global-admin account is high-value
  (recommend mandatory MFA via global settings).
- **P5 (suspended tenant is privacy-preserving).** A suspended tenant
  returns 503 uniformly (indistinguishable from a real outage); a
  nonexistent slug returns 404 — neither leaks tenant existence by
  differential response.
- **P6 (tenant-creation race is DB-enforced).** Slug uniqueness is the
  unique index on `tenants.slug`; simultaneous creations on the same slug —
  one wins, one errors — with no reliance on application-layer
  serialisation.

## Non-goals

Explicitly deferred to subsequent RFCs even if multi-tenant ships:

- **Organisation hierarchies** (departments/business units within a
  tenant) — a separate model; tenants are flat here.
- **Group / role / permission systems** — the only roles are
  `is_tenant_admin` and `is_global_admin`; per-resource permissions are
  out of scope.
- **Custom claims / arbitrary user attributes** — userinfo stays minimal;
  per-tenant claim mapping is a follow-up.
- **Tenant marketplace / self-service tenant signup** — tenant creation is
  admin-only.
- **Cross-tenant federation** — a user in tenant A cannot log into a
  client in tenant B; inter-tenant federation would be its own design.

## Proposed design

### §1 The `tenants` table

```sql
CREATE TABLE tenants (
    id TEXT PRIMARY KEY,             -- ulid or uuid
    slug TEXT NOT NULL UNIQUE,       -- url-safe, e.g. "acme"
    name TEXT NOT NULL,
    status TEXT NOT NULL
        CHECK (status IN ('active', 'suspended', 'deleted')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

`slug` matches `^[a-z0-9](-?[a-z0-9])*$`, length 2..64, immutable after
creation (changing it would break already-issued issuer URLs). A reserved
slug `_global` represents "not bound to a tenant" (§4).

### §2 `tenant_id` propagation

**Single-source-of-truth principle.** A row that already joins to a
tenant-bearing table via FK does *not* also carry its own `tenant_id` —
this avoids tenant-id drift. **Tenant-bearing tables** (carry their own
`tenant_id`): `users`, `clients`, `audit_log`. Everything else
(`sessions`, `refresh_tokens`, `user_totp`, `user_webauthn_credentials`,
`password_reset_tokens`, `revoked_access_tokens`, consents) inherits
through FK. `audit_log` carries its own because audit rows need tenant
scope at write time even when actor/target no longer exist.

Singleton config becomes per-tenant:

```sql
CREATE TABLE tenant_settings (
    tenant_id TEXT PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    default_lang TEXT,
    hibp_mode TEXT NOT NULL,
    idle_session_timeout_secs INTEGER NOT NULL,
    max_concurrent_sessions INTEGER
    -- ...
);
```

A new `global_settings` singleton holds deployment-wide settings (cookie
prefix, listen address, feature flags, global-admin bootstrap state).

### §3 Login-identifier scoping

`username` and `email` are unique *within a tenant*: `(tenant_id,
username)` and `(tenant_id, email_normalized)` are the unique keys. Two
tenants can both have "alice". The OIDC `sub` is the user's row id
(opaque, globally unique); RPs never see `username`. The login form is
*always* tenant-scoped (`/{slug}/login`) — no tenant-selector dropdown,
which would enable tenant enumeration.

### §4 Admin scoping

Two roles, neither sharing a column:

- **Global admin** (`is_global_admin=1`, `tenant_id=_global`) — creates /
  suspends / deletes tenants, manages global settings. No direct
  tenant-data access without *assuming* a tenant.
- **Tenant admin** (`is_tenant_admin=1`, `tenant_id=<real tenant>`) —
  manages users / clients / settings within their tenant only.

A CHECK constraint forbids being both. A global admin *assumes* a tenant
via a session with `assumed_tenant_id`; the assumption is audited (P4).
`/{slug}/login` authenticates a tenant user; `/_global/login`
authenticates a global admin (reachable only by direct URL, bootstrapped
in setup).

### §5 Issuer / discovery / JWKS per tenant

Issuer `https://{host}/{slug}/`; discovery
`/{slug}/.well-known/openid-configuration`; JWKS
`/{slug}/.well-known/jwks.json`. **Per-tenant signing keys (recommended
option A)** over shared keys: isolation is the point of tenancy, RFC 021's
single-active invariant becomes per-tenant
(`UNIQUE (tenant_id, is_active) WHERE is_active = 1`), and rotation runs
per-tenant. A signing-key compromise then affects only one tenant.

### §6 Routing layer

Every URL gains a tenant prefix except truly global ones:

```
/{slug}/.well-known/openid-configuration   /{slug}/oauth2/authorize
/{slug}/oauth2/token                       /{slug}/oauth2/userinfo
/{slug}/admin/login   (tenant admin)       /_global/admin/login (global)
/{slug}/admin/users                        /_global/admin/tenants
/{slug}/me/security                        /setup (one-shot global)
```

Slug-extraction middleware validates against `tenants` (P1). Unknown slug
→ 404; suspended → 503 (P5).

### §7 Setup flow

Two stages: **bootstrap** (once per deployment — creates the `_global`
reserved tenant, the first global admin, `global_settings`, the master
key, global signing keys) and **tenant creation** (a global admin creates
the first real tenant via `/_global/admin/tenants`, which generates that
tenant's signing keys and first admin). The wizard is for the
*deployment*; tenants come after, matching RFC 012's one-shot-bootstrap
contract.

### §8 Migration (single-realm → multi-tenant)

A one-shot upgrade that creates one default tenant containing everything:
add `tenants`, insert the default tenant, add and backfill `tenant_id`
columns, copy singleton config into `tenant_settings`, promote the
existing admin (`is_tenant_admin = is_admin`, then drop `is_admin`), and
keep existing URLs working via redirects.

**The issuer-URL transition is the hard part** (RPs cache `iss`). Three
options: hard-cut (re-register; brutal); both-issuer transition (sign with
the new issuer, accept old issuers on verify endpoints for one release —
recommended); no-URL-change for the default tenant (backwards-compatible
at the cost of a routing special-case). Recommend **both-issuer, one-
version transition**, documented in the release notes.

### §9 Audit-log scoping

`audit_log` gets `tenant_id`. A tenant admin sees only their tenant's
rows; a global admin sees all with a tenant filter. The hash chain is
**per-tenant** (strong isolation — a tenant admin cannot infer another
tenant's audit volume by side channel), plus a separate `_global` chain
for global-admin and tenant-lifecycle events.

### §10 Rate limiting and lockout

Per-tenant: the rate-limit key derivation includes `tenant_id`. A lockout
of "alice" in one tenant does not lock out "alice" in another.

## Data model impact

Substantial. New `tenants` table; `tenant_id` on `users`, `clients`,
`audit_log`, `signing_keys`; singleton config replaced by
`tenant_settings` + `global_settings`; admin-role columns replaced. The
single-source-of-truth principle (§2) keeps FK-reachable tenancy off most
tables. This is a major-version migration, not a 0.x.y patch — one-shot at
a major boundary, no hard data loss, soft changes (issuer URLs, RP token
refresh) documented in upgrade notes.

## API impact

Every OIDC and admin endpoint gains a tenant prefix; new global-admin
endpoints under `/_global/`; per-tenant discovery/JWKS. The issuer URL in
issued tokens changes (handled by the both-issuer transition, §8). Setup
becomes two-stage.

## Testing strategy

- **Per-tenant isolation** — e2e: tenant A admin cannot read tenant B
  users.
- **Routing** — 404 on bogus slug (P1), 503 on suspended tenant (P5), 200
  on active.
- **Per-tenant signing-key isolation** — a tenant A token does not verify
  against tenant B JWKS (P2).
- **Login-identifier scoping** — same username legal in two tenants;
  cross-tenant login fails with `invalid_grant`.
- **Migration regression** — an upgraded single-realm DB produces correct
  multi-tenant invariants.
- **Global-admin assumption** — assumption is logged, scope limited (P4).

## Migration strategy

One-shot, at a major-version boundary, behind a documented upgrade
workflow with backup-before-upgrade discipline. The both-issuer transition
window (§8) gives RPs one release to re-issue tokens via refresh. No hard
data loss; soft changes are documented. Forward-only; no automated
downgrade.

## Rollout plan

This is a single major-version feature, not an incremental series. It is
not scheduled. A schedule forms only when (a) a real deployment surfaces a
need that "one process per tenant" cannot meet, or (b) the owner judges
the design stable enough in `proposed/` to begin. Until then the RFC's
role is to inform other RFCs and to prevent piecemeal tenancy. No version
designation without owner direction and soak.

## Risks and mitigations

- *Risk:* cross-tenant data access (the core concern). *Mitigation:*
  P1–P3 — strict slug validation, per-tenant JWKS + `iss` verification,
  path-scoped cookies/CSRF.
- *Risk:* global-admin over-reach. *Mitigation:* P4 — no tenant access
  without audited assumption; mandatory MFA recommended.
- *Risk:* tenant-existence leakage. *Mitigation:* P5 — uniform 503 for
  suspended, 404 for unknown.
- *Risk:* piecemeal tenancy sneaking in. *Mitigation:* this RFC's detail
  lets reviewers reject partial changes that break invariants without
  delivering isolation.
- *Risk:* issuer-URL transition breaks RPs. *Mitigation:* §8 both-issuer
  one-version window, documented.

## Acceptance criteria

- A tenant admin can never read another tenant's data through any path.
- Tokens issued for one tenant do not verify or introspect against
  another.
- Unknown slug → 404; suspended tenant → 503; both privacy-preserving.
- Global-admin tenant access requires an audited assumption.
- The single-realm → multi-tenant migration produces correct invariants
  with no hard data loss.
- Per-tenant signing keys, audit chains, and rate-limit/lockout scoping
  hold.
- 0 warnings; full suite green; all CI gates hold.

## Open questions

1. **Slug case-sensitivity** — lowercase per §1; confirm.
2. **`_global` literal vs an uncollidable path segment** (e.g.
   `/$global`) — reserved-literal is simpler and matches this design;
   decide at implementation.
3. **Per-tenant SMTP** — `tenant_settings` should include SMTP so each
   tenant sends from its own domain; confirmed in scope.
4. **Single-tenant deployments** — a one-tenant deployment should feel as
   light as single-realm; §8's no-slug-prefix-for-default option handles
   this if explicitly supported, else a reverse-proxy rewrite hides the
   slug.
5. **Future organisation-within-tenant** — out of scope, but the FK
   propagation and `tenant_settings` shape must not preclude adding an
   `organization_id` later; by construction they do not.
