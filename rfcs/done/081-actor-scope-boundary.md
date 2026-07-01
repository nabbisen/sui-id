# RFC 081 — Actor Scope Boundary and Scoped Repository Signatures

**Status.** Implemented (v0.67.0)
**Tracks.** Strategy theme 4 ("tenant boundary"), translated for
a single-tenant codebase (audit gap G9, G5 partially). Category B.
**Touches.** New `sui-id-core/src/actor.rs`,
`sui-id-core/src/admin/*`, `sui-id-core/src/me_security.rs`,
mutation functions in `sui-id-store/src/repos/*`, handler
extractors in `sui-id/src/handlers/`.

## Summary

sui-id has no tenants; its real isolation boundaries are **role
scope** (Admin / Auditor / User), **self vs other** (a user may
mutate only their own resources), and **client binding** (a
client may observe only its own tokens). This RFC makes those
boundaries explicit in type signatures: an `Actor` context type
constructed only by the authentication layer, marker-carrying
capability types (`AdminActor`, `ReadOnlyAdminActor`, `SelfActor`),
and privileged repository/domain mutations re-signed to require
the appropriate actor type — so a privileged call without proof
of privilege becomes a compile error, and "query forgot the
scope filter" becomes structurally hard to write.

## Motivation

Strategy §5.4's intent is that scoped resources cannot be touched
without scope context and that system-level administration is
type-separated from lower scopes. In v0.63.1 enforcement lives in
Axum extractors plus per-page `can_write: bool` flags; domain
functions like `admin::users::disable(db, user_id)` are callable
from any code path with no proof an authorization check happened.
The auditor role (RFC 071) raised the cost of getting this wrong:
a future handler that takes `CurrentAdminOrAuditor` but calls a
mutation compiles fine today. The audit also confirmed the
multi-tenant theme itself is deferred (proposed RFC 025), so this
RFC deliberately creates the seam RFC 025 would later widen
(`Actor` would gain a tenant field; signatures already thread it).

## Background

Extractors: `CurrentAdmin` (admin only), `CurrentAdminOrAuditor`
(read surfaces, `can_write` rendering flag), self-service session
resolution in `/me/*`. Roles live in `sui_id_store::models::Role`.
Last-admin safeguard exists in admin/users domain logic.

## Target code areas

1. New `sui-id-core/src/actor.rs`:

   ```rust
   /// Proof of an authenticated principal. Constructible only by
   /// the session-resolution path (private ctor; the handlers
   /// crate gets it from the extractor, which calls a single
   /// `pub(crate)`-gated factory in core).
   pub struct Actor { user_id: UserId, role: Role, session: SessionId }

   pub struct AdminActor(Actor);          // role == Admin, write-capable
   pub struct ReadOnlyAdminActor(Actor);  // Admin or Auditor, read-only
   pub struct SelfActor(Actor);           // any role, self-scoped ops

   impl Actor {
       pub fn into_admin(self) -> Result<AdminActor, Actor>;
       pub fn into_read_admin(self) -> Result<ReadOnlyAdminActor, Actor>;
       pub fn into_self(self) -> SelfActor;
   }
   ```

   `AdminActor` exposes `read_only(&self) -> &ReadOnlyAdminActor`
   -style downgrade (one direction only; no upgrade path).

2. Domain mutations require the capability type:
   `admin::users::disable(db, &AdminActor, target: UserId)`,
   `admin::clients::rotate_secret(db, &AdminActor, …)`,
   `me_security::change_password(db, &SelfActor, …)` — and
   self-scoped functions derive the target from the actor
   (`actor.user_id()`), never from a caller-supplied user id, so
   "operate on someone else via the self-service path" is
   unrepresentable.
3. Read-only admin surfaces take `&ReadOnlyAdminActor`.
4. Extractors are reworked to produce `Actor` and convert; the
   `can_write` flag for rendering is derived
   (`actor.role() == Role::Admin`) instead of free-floating.
5. Store-layer mutations invoked from admin/self domains take the
   actor's `UserId` for audit attribution (feeding RFC 085).

## Security properties / invariants

- **P1 (deny by construction).** No privileged domain mutation is
  callable without an `AdminActor`; no `AdminActor` exists except
  via role-checked conversion from an authenticated `Actor`.
- **P2 (read/write separation).** `ReadOnlyAdminActor` reaches no
  mutation signature; the auditor role cannot mutate even if a
  handler is mis-wired.
- **P3 (self-scope).** Self-service mutations target only
  `actor.user_id()`; cross-user targeting via `/me/*` is not
  expressible.
- **P4 (no forgery).** `Actor` has no public constructor and no
  `Deserialize`.
- **P5 (membership change).** Role change or session revocation
  invalidates future `Actor` construction immediately (actors are
  per-request values, never cached across requests — pinned by
  doc + test).

## Non-goals

- No tenants, no `TenantId`, no `TenantScoped<T>` — explicitly
  deferred with RFC 025; this RFC only keeps the seam compatible.
- No policy engine; no permission granularity beyond the three
  roles (strategy §4 non-objectives).
- No change to the login/MFA flows that *establish* sessions.

## Proposed design

(Core shape above.) The key discipline: capability types live in
core, conversions are the only gate, and handlers shrink to
"extract → convert → call". Where a handler needs both read and
write paths, it holds `AdminActor` and downgrades for reads.

## Data model impact

None.

## API impact

Internal signatures only; routes and wire behaviour unchanged.
Rendering behaviour unchanged (`can_write` derivation identical).

## Testing strategy

- `compile_fail` doctests: calling `admin::users::disable` with
  `ReadOnlyAdminActor` / `Actor` / raw ids.
- Unit: conversions reject wrong roles (`User → into_admin` errs;
  `Auditor → into_read_admin` ok, `into_admin` errs).
- Integration (binary crate, CI): auditor session hitting every
  admin mutation route still receives the deny response —
  unchanged behaviour, now double-enforced.
- Property test (with RFC 082): for arbitrary (role, operation)
  pairs, the conversion table equals the documented role matrix.

## Migration strategy

Mechanical signature propagation in one sweep; no data
migration; behaviourally invisible.

## Rollout plan

Suggested v0.66.0, paired with RFC 082 (the decision core
consumes `Actor`). Sequenced after RFC 078 (shared id types).

## Risks and mitigations

- *Risk:* boilerplate in handlers. Mitigation: conversions are
  one line; extractor does the common case.
- *Risk:* a future "system maintenance" code path (CLI) has no
  session. Mitigation: a distinct `CliActor` constructor gated to
  the binary crate's CLI module, audited as `actor=cli` —
  keeping the rule "every mutation names its principal".

## Acceptance criteria

- All admin mutations and self-service mutations require the
  respective capability types; grep shows no privileged domain
  fn taking only `db + ids`.
- Compile-fail and conversion tests pass; integration denials
  unchanged; 0 warnings; baseline green.

## Open questions

- Should `SelfActor` also gate *reads* of own sensitive data
  (sessions list, consent list)? Leaning yes for uniformity;
  decide during implementation by diff size.
