# RFC 007 — Multi-tenancy

**Status.** Exploratory
**Tracks.** ROADMAP / Longer term — "Multi-tenancy".
**Touches.** every table that has a `user_id` or `client_id` (i.e.
most of `sui-id-store`), cross-cutting policies in
`sui-id-core` (lockout, rate limits, audit chain), the admin UI
in `sui-id-web`, the OIDC discovery and JWKS endpoints.

This is a deep change. The RFC is sketch-grade and the
maintainer's call is whether multi-tenancy belongs in sui-id at
all — it pulls hard against the "minimum ceremony" stance.

## Summary

Today every user, client, signing key, audit row, etc. shares a
single flat namespace. A *tenant column threaded through the
schema* would partition this into multiple isolated namespaces,
each with its own admins, its own client list, its own audit
chain. This is the standard B2B-IdP shape — Auth0 calls it
tenants, Keycloak calls them realms, Entra calls them tenants.

The shape is uncontroversial; the question is whether sui-id
should grow into it. The answer probably depends on whether a
specific deployment surfaces the need.

## Background — why this is hard, not just tedious

Adding a `tenant_id` column everywhere is the easy part. The
hard parts:

1. **Cross-cutting policy isolation.** Lockout counters,
   per-IP rate limits, refresh-token theft families, audit
   hash chains — all of these become per-tenant data
   structures, not global. The hash chain in particular: if
   sui-id today has one chain, tenant T1's audit row N+1 must
   chain to T1's row N, not to whatever the latest global row
   was.
2. **OIDC routing.** OIDC discovery is at a single
   well-known path. Multiple tenants need either
   per-tenant subdomains or a per-tenant path prefix. Both
   are fine; both interact with redirect-URI validation in
   ways that need explicit design.
3. **Cross-tenant isolation invariants.** A bug that lets a
   client in tenant A authorise a user from tenant B is
   catastrophic. The query layer needs a strict "always
   filter by tenant_id" discipline. Existing repo functions
   take `&Database`; adding a `TenantContext` parameter
   everywhere is mechanically obnoxious but the only way to
   make the invariant compiler-checked.
4. **Single-tenant deployments.** The vast majority of
   sui-id deployments are single-tenant and should not pay
   the schema cost. The migration must keep working for
   them: a default `tenant_id = 'default'` row, threaded
   transparently, with the existing UI behaving as before.

## Requirements (if implemented)

1. A new top-level concept "tenant", identified by a slug
   (`acme`, `beta-corp`, …). The default tenant is `default`
   and every existing row migrates into it.
2. Every existing entity gains a `tenant_id` foreign key.
   Cross-tenant queries are impossible to express through the
   normal repo API.
3. Each tenant has its own admin user(s). A tenant admin can
   only see and manage their tenant's data.
4. There is a *root* role that spans tenants — the operator
   who created the deployment. The root admin can create and
   delete tenants and provision the first tenant admin.
5. OIDC clients are scoped to a tenant. Discovery for
   tenant `T` is at `/<T>/.well-known/openid-configuration`
   (or `https://<T>.host/.well-known/...` if subdomain
   routing is enabled). JWKS likewise.
6. Every audit row is scoped to a tenant. The hash chain is
   per-tenant. The root operator's actions on a tenant are
   recorded both in the tenant's chain (as the originating
   action) and in a global root-audit chain.
7. Per-IP and per-account rate limits are per-tenant. A
   spammer flooding tenant A's `/admin/login` does not
   degrade tenant B's sign-in throughput.
8. The flat namespace persists for a single-tenant
   deployment: nothing in the URL, UI, or admin experience
   shows "tenants" unless `tenant_count > 1`.

## Design (sketch)

### Schema

Migration `0030_tenants.sql` (large, intricate):

```sql
CREATE TABLE tenant (
    id            TEXT PRIMARY KEY,
    slug          TEXT NOT NULL UNIQUE,
    display_name  TEXT NOT NULL,
    created_at    TIMESTAMP NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT 1
);

INSERT INTO tenant (id, slug, display_name, created_at)
    VALUES ('default-tenant-uuid', 'default', 'Default', '<migration-time>');

-- For every existing table with user/client/etc references:
ALTER TABLE users          ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default-tenant-uuid';
ALTER TABLE clients        ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default-tenant-uuid';
ALTER TABLE signing_keys   ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default-tenant-uuid';
ALTER TABLE refresh_tokens ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default-tenant-uuid';
ALTER TABLE audit          ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default-tenant-uuid';
-- … and so on, ~15 tables.

-- Indexes change to lead with tenant_id:
CREATE UNIQUE INDEX idx_users_tenant_username
    ON users (tenant_id, username);
CREATE UNIQUE INDEX idx_clients_tenant_slug
    ON clients (tenant_id, ...) ;
-- … etc.
```

Followed by a *separate* migration that drops the old
single-column UNIQUE constraints on `users.username` etc.,
to be replaced with the composite ones above.

### Repo discipline

Every repo function gains a `tenant_id` parameter. Type-level
enforcement via a `TenantId` newtype that must be threaded
through:

```rust
pub fn get_user(db: &Database, tenant: TenantId, user_id: UserId) -> StoreResult<User>;
```

The compiler enforces that nobody can call `get_user` without
a `TenantId` in hand, and the only ways to get a `TenantId`
are:

- From the request context (resolved at the routing layer
  from URL path or subdomain), OR
- From an explicit `RootContext::all_tenants()` iterator,
  used only by the root admin's listing screens.

There is no `&str` -> `TenantId` conversion that doesn't go
through validation.

### Routing

Two modes, configurable:

- **Path-prefix routing.** `/<tenant_slug>/admin`,
  `/<tenant_slug>/oauth2/authorize`, etc. Discovery at
  `/<tenant_slug>/.well-known/openid-configuration`. Default.
  Easiest to deploy.
- **Subdomain routing.** `<tenant_slug>.host`. Cleaner but
  requires wildcard TLS and DNS. Configuration option.

The single-tenant deployment uses neither — paths are flat
and unprefixed, exactly as today. The tenant context for
single-tenant deployments resolves to the implicit default
tenant.

### Audit chain per tenant

Each tenant's audit chain is independent. The
`audit::append` function takes the tenant ID and looks up
the latest hash for *that* tenant.

The root operator gets a separate `root_audit` table with
its own chain, which records cross-tenant actions: tenant
created, tenant disabled, tenant admin provisioned, etc.

### Migration strategy

The migration is one-way. There is no path back to a
single-flat-namespace schema short of a full export/import.
This is fine — multi-tenancy is a deliberate adoption
decision.

For an existing single-tenant deployment:

1. Migration runs, all existing rows get
   `tenant_id = 'default-tenant-uuid'`.
2. UI continues to behave as single-tenant: no tenant
   selector, no tenant column in lists.
3. The first time the admin creates a *second* tenant, the
   UI flips into multi-tenant mode and exposes the tenant
   navigation.

## Tests (sketch)

- Migration round-trip on a populated single-tenant DB.
- Per-tenant isolation: two tenants with same-named users,
  user A in tenant 1 cannot authenticate against tenant 2's
  authorize endpoint.
- Audit chains are independent: rows added to T1 do not
  break T2's chain hash.
- Per-tenant rate limits: spam tenant 1, observe tenant 2's
  rate limits unchanged.
- Path-prefix routing: 404 on cross-tenant path mixing
  (`/T1/oauth2/authorize?client_id=<T2-client>`).
- Subdomain routing: same as above with subdomain mismatch.
- Default-tenant behaviour for single-tenant deployments
  unchanged.
- Root-operator audit: creating a tenant, the root_audit
  chain extends, the new tenant's chain initialises empty.

## Security considerations

- **Tenant isolation as a hard invariant.** This is the
  whole feature. The compiler-enforced `TenantId`
  parameter is the primary defence; integration tests are
  the secondary.
- **Cross-tenant audit visibility.** Tenant A's admin
  cannot see tenant B's audit. The root operator can see
  everyone's. Documented loudly.
- **Signing key isolation.** Each tenant has its own
  signing keys, its own JWKS, its own JWTs. A token
  signed by tenant A cannot validate against tenant B's
  JWKS. This is what makes per-tenant trust meaningful.
- **Tenant deletion.** Soft-deletes a tenant by default
  (`enabled = false`). Hard-delete is a separate root CLI
  command (`sui-id admin tenant purge --slug X`) that
  cascades through every table. Audit chain for the
  deleted tenant is preserved by default; a separate
  flag deletes it too.
- **Master key.** A single master key per deployment,
  shared across tenants. Per-tenant master keys are
  *not* in scope for this RFC and would be a
  significantly larger change.

## Open questions

- Do we need per-tenant SMTP configuration? Probably yes;
  schema gains `smtp_config.tenant_id`, the existing
  singleton becomes a per-tenant singleton.
- Per-tenant `hibp_mode`? Yes, same shape.
- Per-tenant master keys would isolate cryptographic
  blast radius across tenants but multiply the rotation
  story. Defer.
- Can a sui-id deployment ever stop being multi-tenant
  once it has been? Recommend: no, treat the migration
  as one-way. A tenant export-and-redeploy gives you a
  fresh single-tenant deployment if you really need it.
