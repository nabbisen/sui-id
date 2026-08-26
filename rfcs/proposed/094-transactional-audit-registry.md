# RFC 094 — Transactional Audit Completeness and Typed Event Registry

**Status.** Proposed
**Security review.** Required
**Lifecycle history.** Base design accepted 2026-07-17 after [independent review](../reviews/094-design-review-2026-07-17.md); material amendment returned to Proposed in commit `43085e38219e5eb1bfe11cc698b18f1fa5f5e4d7`; complete amended RFC accepted by `@nabbisen` on 2026-07-21 after [independent review](../reviews/094-federation-command-amendment-review-2026-07-21.md); **returned to Proposed on 2026-07-28** for the scope amendment described below, per RFC 000's return-for-review rule for material changes to scope, prerequisites, and acceptance criteria. The 2026-07-21 acceptance is preserved in history and is superseded, not withdrawn.
**Amendment summary (2026-08-26).** Correction round after the external review: `ReadConn`'s read-only guarantee gains a required M2a assertion that `rusqlite`'s `functions`, `vtab` and `load_extension` features stay disabled, since statement-level read-only status does not constrain side-effecting application functions or virtual tables — that surface is currently not compiled in, and nothing checked it. F01–F06 gain an explicit phase statement: they belong to no conversion wave, are implemented by RFC 096-B1 against the M2a runner foundation, and their prerequisite is that foundation rather than the session-security wave. Both carry an explicit confirmation-required note.
**Amendment summary (2026-08-12).** `ReadConn`'s per-statement `sqlite3_stmt_readonly` interrogation restored as a **required** M2a control after independent review finding B-094-1 established that static denial alone admits `UPDATE … RETURNING` and the other DML `RETURNING` forms; only the versioned read-only PRAGMA allowlist remains deferred. M2a acceptance criteria gained the corresponding negative fixtures. The 2026-07-28 deferral's stated grounds — a "different property", and invasiveness "touching every read path" — were both incorrect.
**Amendment summary (2026-07-28).** Master-key rotation crash recovery removed to RFC 100; `ReadConn` narrowed to the typed wrapper plus static denial, deferring per-statement runtime interrogation; the `syn` AST boundary gate sequenced into the M2b authority switch; conversion phased into M2a and M2b with C15 pinned to M2a; acceptance criteria split per stage; interim documentation honesty made normative. Requested by `@nabbisen` on 2026-07-28 on the recommendation of the requirements architect.
**Implementation owner.** `codex-developer` (OpenAI Codex), confirmed by `@nabbisen`; implementation remains gated below
**Design prerequisites.** RFC 093 Accepted (its gate contract must be Accepted before this RFC is accepted); the attached durable-write inventory and threat delta require independent design approval. The key-recovery state machine is no longer part of this RFC and is reviewed under RFC 100.
**Implementation prerequisites.** M1a complete — RFC 093's Rust gate lanes G01–G09 pass on one clean commit; this RFC Accepted in its amended form; the Class-A inventory and threat delta have independent approval. M1b is not a prerequisite.
**Closure prerequisites.** Per-stage. **M2a:** every converted Class-A path uses the approved transaction seam, injected append failures roll back mutation for every converted row, C15 is atomic, the structural gate passes over converted commands, and raw database access is confined to `sui-id-store` by the dependency graph — `rusqlite` depended on by no other crate, gate-asserted (owner decision, 2026-08-26). **M2b:** every remaining Class-A row converted, no production Class-A best-effort append anywhere, AST boundary gate landed, and independent adversarial closure review accepts durable evidence.
**Tracks.** ROADMAP M2a — Transactional security records (foundation and priority conversion); M2b — remaining conversion and authority switch.
**Touches.** `crates/sui-id-core/src/audit_guard.rs`, `events.rs`, privileged domain services, `crates/sui-id-store/src/db.rs`, `repos/audit.rs`, mutation repositories, audit documentation and CI.
**Handoff.** [`../handoffs/094-transactional-audit/README.md`](../handoffs/094-transactional-audit/README.md)
**Command inventory.** [`../handoffs/094-transactional-audit/command-inventory.md`](../handoffs/094-transactional-audit/command-inventory.md)
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Independent security and closure reviewer.** Role independence per RFC 018 —
the reviewer must not have authored, implemented, or previously approved this
RFC; vendor is not a criterion. Design review routes to the implementation
role, for implementability and for specification gaps it would hit while
building. Judgments that role cannot adjudicate are named explicitly in the
review request and route to a second capable reviewer when one is available,
otherwise to `@nabbisen`, recorded as unreviewed design judgment. Closure
review routes to the specifying role and is evidenced by executing the closure
prerequisites.

> **Returned to Proposed on 2026-07-28.** This RFC was Accepted on 2026-07-21
> including the C17/C18/C23/F01–F06 federation command additions, which remain
> unchanged and independently reviewed. The 2026-07-28 scope amendment
> summarized above is material under RFC 000, so the RFC returns to Proposed and
> requires fresh independent design review and re-acceptance before any
> implementation. Nothing in the previously reviewed federation-command content
> is reopened.

## Summary

Make every inventory-classified Class-A security/operator mutation either
commit with exactly one typed audit record or roll back with it. Protocol,
operational, internal, and bootstrap writes receive separate sealed
capabilities and explicit reviewed rationales. A central event registry becomes
the source of event names, required fields, and atomicity class; a Class-A
transaction runner becomes the only production construction path for
successful audited mutations; and a structural gate proves that the frozen
inventory, registry, call sites, and failure-injection tests agree.

This RFC corrects the present false assurance: the existing matrix calls many
operations Class A while production helpers can still append asynchronously
and ignore failure. Literal string parity is retained only as an M1 diagnostic
and is replaced as authority by typed structure in M2.

## Background

RFC 085 introduced `audit::append_within_tx`, `Audited<T>`, and a coverage
matrix. However, `audit_guard::audit_and` currently performs the mutation and
then ignores failure from the best-effort async append. `events::emit` follows
the same must-attempt shape. A type named `Audited<T>` therefore does not prove
transactional persistence. The documentation also assigns Class B to several
mutations and Class A to a federation denial that performs no durable mutation.

The audit hash chain protects records that exist. It cannot prove that a
record was emitted or that its state change did not commit alone.

## Requirements

1. One typed registry defines each event identifier, class, actor/target
   policy, result vocabulary, and note schema. Production code does not choose
   Class-A event names from arbitrary strings.
2. A Class-A success is constructible only inside one SQLite transaction that
   performs the domain mutation and `append_within_tx` before commit.
3. Any mutation error, event-construction error, audit-chain read/write error,
   serialization error, or injected append failure rolls back both effects.
4. Each successful logical Class-A command emits exactly one success record.
   Retries that do not win a guarded mutation emit no false success record.
5. Denials, detections, and attempts that must remain effective when logging
   fails use Class B. Append failure is reported through structured error
   telemetry without exposing secrets or changing the denial.
6. The durable-write inventory is exhaustive for shipped state mutations at
   M2. Every write is compiler-registered as Class A, protocol state,
   operational maintenance, internal primitive, or bootstrap migration. A new
   unclassified mutation cannot obtain a write transaction or pass CI.
7. RFC 094 converts the shipped dynamic-registration command to the Class-A
   runner at M2: guarded use consumption, client creation, registration-source
   stamp, and `client.dynamic_register` append share one transaction. RFC 095
   retains full validate-first metadata parity plus adversarial concurrency and
   retry completion; there is no M2 best-effort exception.
8. The audit-chain claims in documentation match the actual transaction and
   failure behavior.
9. No external anchoring/notarization service is introduced.

## Security invariants

- **A1 — atomic pair.** For each Class-A command, committed mutation count and
  committed success-event count change together: both one or both zero.
- **A2 — one event.** One successful logical command yields exactly one event
  with its registry identifier; wrapper nesting cannot duplicate it.
- **A3 — typed identity.** A Class-A path cannot supply an unregistered string
  as its event kind.
- **A4 — field completeness.** Registry-required actor, target, result, and
  structured attributes are present before append; secrets are not accepted by
  field types.
- **A5 — loser honesty.** A guarded/concurrent loser cannot emit success.
- **A6 — Class-B fail-safe behavior.** Failure to log a denial/detection never
  turns it into success and is itself observable through structured telemetry.
- **A7 — chain continuity.** Chain-head read and append occur on the same
  transaction handle as the mutation; rollback leaves no orphan chain link.
- **A8 — structural closure.** The authoritative gate derives coverage from
  typed descriptors and registered command bindings, not source-text presence.

## Frozen classification and exact inventory

The proposed exact Stage-0 inventory is attached at
[`command-inventory.md`](../handoffs/094-transactional-audit/command-inventory.md).
It classifies the current production durable-write universe one logical command
at a time and names owner, typed event or exclusion rationale, mutation surface,
and stable test ID. The reviewed implementation manifest
`ci/write-commands.toml` is a machine-readable expansion of that document down
to individual Rust function and SQL write-site identifiers.

The following is a summary only; the attached inventory is normative:

| Family | Commands |
|---|---|
| User administration | create, disable/enable/delete, role/password/email change, MFA/passkey administration, unlock, lockout transition, LDAP shadow upsert |
| Client administration | create, all metadata/policy/URI/scope changes, disable/enable/delete, secret rotation, registration authorization |
| Signing keys | rotate/activate, delete/retire where allowed |
| Server/security settings | default language, HIBP mode, idle timeout, concurrent-session limit, metrics-token change, SMTP/security configuration apply |
| Pending sensitive settings | create, apply/consume, cancel when it changes durable intent |
| Credential, consent, and session security | self/reset password, TOTP/passkey lifecycle, consent grant/revoke, force logout and session revocation |
| Token and registration security | refresh-family/admin revocation, registration-token issue/revoke, baseline atomic dynamic registration |
| Federation and authorization configuration | provider create/enable/delete, link create/delete, scope-definition create/delete |

`client.dynamic_register` is Class A and converts in M2. RFC 095 later tightens
pre-transaction validation, complete metadata persistence, and race/retry
semantics on the same transaction seam; it does not repair an M2 audit gap.

Class B includes authentication attempts/results, rejected authorization,
takeover blocks without a durable mutation, upstream/LDAP transport failures,
other non-mutating denial/observation events. Refresh-reuse family revocation
and its theft-detection event form one Class-A command. If a nominal
Class-B flow also mutates durable security state (for example lockout counters),
that mutation is split into or reclassified as a Class-A command.

Before acceptance, the reviewer compares the attached inventory to all
production write entry points. No “miscellaneous” row is permitted. Any
discrepancy amends the attachment before approval rather than becoming an
implementation-time allowlist.

Acceptance explicitly records the owner and independent reviewer's risk
decision for every P/O/I/X exclusion. Protocol/operational telemetry is not the
tamper-evident audit chain and cannot be cited as equivalent assurance. An
exclusion remains valid only for its named command/surface/rationale; expanding
its security effect requires reclassification and design review.

## Typed registry design

Add a dedicated module (exact crate placement may follow dependency hygiene)
whose public surface is equivalent to:

```rust
pub enum AuditClass { Atomic, MustAttempt }

pub enum AuditEventKind {
    UserCreate,
    UserDisable,
    // one variant per stable event identifier
}

pub struct EventDescriptor {
    pub kind: AuditEventKind,
    pub name: &'static str,
    pub class: AuditClass,
    pub actor: ActorRequirement,
    pub target: TargetRequirement,
    pub attributes: &'static [AttributeSpec],
}

pub trait CommandSpec: sealed::Sealed + Sized {
    type Event: SealedCommandEvent<Self>;
    fn descriptor(event: &Self::Event) -> &'static EventDescriptor;
}

pub trait SealedCommandEvent<C: CommandSpec>: sealed::Sealed {
    fn target(&self) -> Option<AuditTarget>;
    fn result(&self) -> AuditResult;
    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError>;
}
```

Concrete event payload structs carry typed IDs and bounded/sanitized
attributes. They do not accept raw tokens, passwords, client secrets, key
material, cookies, authorization codes, or arbitrary debug values. Event names
are serialized only through `EventDescriptor`; aliases and vocabulary changes
are explicit registry migrations.

The registry exports a deterministic descriptor list for documentation and CI.
The human-readable audit reference is generated from, or mechanically checked
against, this list. Duplicate names, duplicate command bindings, missing
descriptors, and class mismatches fail unit tests and the structural gate.

## Class-A transaction seam

The store exposes a synchronous transaction context because rusqlite
transactions cannot safely cross an async suspension point. The exact naming
may change, but the ownership constraints may not:

```rust
pub struct ClassATx<'tx, C: CommandSpec> {
    tx: &'tx rusqlite::Transaction<'tx>,
    command: &'tx AuthorizedCommandContext<C>,
}

impl Database {
    pub async fn class_a<C, T, E, F>(
        &self,
        context: AuthorizedCommandContext<C>,
        build: F,
    ) -> Result<Audited<T>, E>
    where
        C: CommandSpec,
        E: From<StoreError>,
        F: FnOnce(&mut ClassATx<'_, C>) -> Result<(T, C::Event), E>;
}
```

The precise lifetime spelling may change to suit the existing database worker,
but the generic event type and private capability must remain statically
enforced. The non-negotiable sequence is:

1. open one immediate write transaction;
2. execute mutation through `*_within_tx` repository functions using the
   private transaction capability;
3. construct and validate the typed event from actual mutation output;
4. append and hash the event using that same transaction;
5. commit;
6. only then construct `Audited<T>` and return success.

Each command declaration generates one sealed command-specific event enum as
`C::Event`. Its variants contain target/result/attribute payload only.
`C::descriptor` is generated as an exhaustive match from each variant to one
registry descriptor in that command's closed allowed-kind set; callers cannot
supply an `AuditEventKind`. Adding an enum variant without a descriptor mapping
fails compilation, while duplicate or cross-command descriptor mappings fail
the structural registry test.

`AuthorizedCommandContext<C>` has private fields and no public constructor. It
is created only by consuming a successful authorization decision for command
type `C`, or by a sealed CLI/system authority adapter for commands whose
descriptor permits that principal. It binds the scoped actor, type-level
command ID, request/correlation ID, and clock instant. `C::Event`
contains only command-specific target, result variant, and bounded attributes;
it has no actor, command-ID, timestamp, or correlation fields. The runner and
registry build those authority fields from the context after verifying the
descriptor for `C`. A payload for one command type cannot satisfy another
command's bound.

Compile-negative fixtures attempt to construct a context directly, add an
actor field to an event variant, use an event enum for the wrong `C`, provide
an arbitrary event kind, leave a variant unmapped, and invoke a system-principal
adapter for a user-only command. Runtime structural fixtures prove every
variant maps to exactly one descriptor allowed for that command and no two
variants accidentally share an exclusive descriptor.

`Audited<T>` has no public constructor. `AuditReceipt` is created after commit,
not merely after an append function was called. Class-A repository mutation
functions are `pub(crate)` or capability-gated so production callers cannot
bypass the runner with a bare database connection.

Operations spanning database state and filesystem key material use an explicit
state machine. The database mutation/audit pair remains atomic; pre-commit file
preparation uses a temporary restricted file, and post-commit activation is
idempotent/recoverable. The registry event states the committed database phase,
not an unverified claim that every external side effect succeeded.

## Class-B emission

Replace silent `let _ = audit::append(...)` behavior with a named
`emit_must_attempt` API. It returns/records an outcome and always emits
structured error telemetry on failure. Callers may preserve the primary denial
or detection response, but tests can inject failure and assert that the failure
signal occurred. Raw append remains internal to the audit subsystem.

## Mechanically complete durable-write universe

Inventory/registry agreement alone is insufficient because both could omit the
same bare write. RFC 094 therefore makes raw database write authority private:

1. `Database` no longer exposes `rusqlite::Connection` or a general
   `with_conn`/`with_tx` closure to production repository/domain modules.
2. Read paths receive a non-`Deref` `ReadConn` wrapper. It never returns
   `rusqlite::Connection`, `Transaction`, `Statement`, raw pointers, or any
   object with execute access. Its only SQL entry points are typed
   `query_row`, `query_map_collect`, and `query_optional` operations that map
   rows internally and return owned values.
3. A sealed `declare_write_command!` macro is the only production constructor
   for `WriteTx<CommandPolicy>`. Its declaration requires stable command ID,
   class, owner, and event/test binding. It generates the manifest descriptor
   consumed by the structural gate.
4. `WriteTx<AtomicAudit>` can commit only through the Class-A runner after
   typed append. `WriteTx<Protocol>`, `<Operational>`, and `<Bootstrap>` have
   distinct runners and cannot construct `Audited<T>`.
5. Within-transaction mutation primitives receive the sealed capability, not a
   raw `rusqlite::Transaction`; raw SQL access is confined to the private
   command-executor module. Migration SQL is confined to the registered
   bootstrap runner before service readiness.
6. `ReadConn` denies dangerous forms **by construction**. It exposes only
   `query_row`, `query_map_collect`, and `query_optional`, and it statically
   rejects `PRAGMA`, `ATTACH`, `DETACH`, multi-statement SQL, trailing
   executable text, batch execution, load-extension, backup, restore,
   serialize/deserialize replacement, transaction control, and FFI/raw-handle
   access before preparation.

   **Static denial is not sufficient on its own and never was.** It rejects
   categories of statement, not data-modifying SQL: `INSERT`, `UPDATE`, `DELETE`
   and `REPLACE` are absent from the list above, and their `… RETURNING` forms
   are row-returning, so `query_row("UPDATE … RETURNING …")` is accepted by every
   check named above while changing durable state outside `WriteTx`, the command
   manifest, the transaction runner and typed event construction. That is not a
   different property from this RFC's subject — it is precisely the mutation/audit
   gap this RFC exists to close.

   **Therefore, per-statement runtime interrogation is required, not deferred.**
   Every statement prepared by `ReadConn` is interrogated with
   `sqlite3_stmt_readonly` after preparation and before any step, and a
   non-read-only statement is rejected without executing. rusqlite exposes this
   as `Statement::readonly()` (verified present in `rusqlite 0.40.1`, the pinned
   workspace version, delegating directly to `ffi::sqlite3_stmt_readonly`). The
   check lives inside `ReadConn`'s own entry points; it does not touch call
   sites.

   The two controls are complementary and both are mandatory: static denial
   rejects whole categories before preparation, and the runtime interrogation
   rejects data-modifying statements that no category test can recognise from the
   SQL string alone.

   Only the **versioned read-only PRAGMA allowlist** remains deferred to a
   successor hardening RFC. `PRAGMA` is already statically denied, so deferring
   the allowlist narrows nothing that the two controls above do not already
   cover.

   **The read-only guarantee has a second dependency, which must be asserted.**
   `sqlite3_stmt_readonly` reports whether a statement makes direct changes to
   the database. It does **not** constrain a side-effecting application-defined
   SQL function, nor a virtual table with external or durable side effects,
   reached from an otherwise read-only statement.

   That surface is currently **not compiled into this workspace**. Verified at
   `0183fc5`: no `create_scalar_function`, `create_aggregate_function`,
   `create_module`, `create_collation` or `load_extension` call exists anywhere
   in `crates/`, and the resolved `rusqlite` feature set across the whole
   dependency graph is exactly `bundled, cache, chrono, default,
   ffi-sqlite-wasm-rs, hashlink, modern_sqlite`. (`ffi-sqlite-wasm-rs` belongs to
   rusqlite's own `default` set and gates an optional WASM-target dependency; it
   enables none of the surfaces below. It was missing from this list until
   2026-08-26, when the Part A review re-ran the check and found seven features
   where this RFC claimed six.) The `functions`, `vtab`, `load_extension`,
   `window`, `series` and `array` features are all opt-in and all off, so
   `create_scalar_function` and its relatives do not exist in the binary.

   **So the boundary holds today by virtue of a Cargo feature line that nothing
   checks.** One added feature silently removes it. M2a must therefore assert
   that `rusqlite`'s `functions`, `vtab` and `load_extension` features remain
   disabled in the resolved graph, and fail if any is enabled.

   An approved-function allowlist was considered and rejected: it would be
   allowlisting an empty set, and it would imply the surface exists. Asserting
   the features stay off is both stronger and cheaper. Should a future RFC need
   one of those features, it must state the requirement and supply the allowlist
   then — that is the point at which the question becomes real.

   *Added 2026-08-12 correction round, after the external correction review
   observed that statement-level read-only status is not the complete semantic
   boundary. The observation is correct; the reachability finding is this
   project's own.*

   **Amendment history.** The 2026-07-28 amendment deferred the runtime
   interrogation on the stated grounds that it guarded "a different property" and
   was "the invasive half of the change, touching every read path in the
   workspace." Both grounds were wrong: the property is the same one, and the
   check is confined to `ReadConn`'s implementation. Corrected 2026-08-12
   following independent review finding B-094-1. The type-level wrapper is what
   makes "raw write authority is private" true rather than decorative, and that
   sentence is only true with the runtime check in place.
7. A `syn`-based repository check parses all production Rust modules and rejects
   raw `rusqlite::Connection`/`Transaction`/`Statement`, statement execution,
   `with_conn`/`with_tx`, execute/batch/transaction calls, writable PRAGMA,
   ATTACH/DETACH, backup/restore, extension loading, raw-handle/FFI access, and
   aliases/helper indirection to forbidden types outside the private executor,
   migration runner, and test fixtures. The workspace's `unsafe_code = forbid`
   is an independent barrier to direct SQLite FFI. The small allowed-module
   list is exact in
   `ci/write-authority.toml`; changing it requires independent review.

The compiler capability is primary; the AST check protects the module boundary
and detects a newly introduced bare write absent from both inventory and
registry. Its mandatory negative fixture adds a new repository function that
calls `Connection::execute("UPDATE ...")` without any declaration; both
`cargo check` and `cargo xtask audit-structure --negative-fixtures` must reject
it. Additional fixtures are grouped by **which control must catch them**. The two
controls do not overlap, and a fixture asserted against the wrong one passes for
a reason the specification misattributes.

- **Rejected at runtime by `ReadConn`** — a prepared `UPDATE`; the
  `INSERT`/`UPDATE`/`DELETE`/`REPLACE` forms and their `RETURNING` variants;
  writable PRAGMA (`PRAGMA user_version = 1`, `PRAGMA journal_mode = WAL`); and
  DDL (`CREATE`/`DROP`/`ALTER TABLE`). `sqlite3_stmt_readonly` reports `false`
  for every one of these, so the runtime interrogation must reject them before
  execution.
- **Rejected only by the static category denial list** — `ATTACH`, `DETACH`, and
  transaction control (`BEGIN`, `COMMIT`, `ROLLBACK`, `SAVEPOINT`). SQLite
  reports these **`readonly() == true`**. The runtime interrogation *cannot*
  reject them, so a fixture asserting that it does specifies a test that cannot
  pass by the route it names. These fixtures must assert rejection by the static
  list specifically.
- **Rejected by the compiler** — the backup API, a returned raw statement, and an
  indirect helper accepting a raw statement/connection through `ReadConn`. These
  must fail to compile.

A read-only `SELECT` control must still succeed.

*Measured 2026-08-26 across all 21 statement forms and independently reproduced
on two SQLite builds (rusqlite 0.40.1 bundled, and system 3.53.4) — see
[`../reviews/094-095-096-correction-review-2026-08-26.md`](../reviews/094-095-096-correction-review-2026-08-26.md)
§A1. This paragraph previously required `ATTACH` to "fail to compile or be
rejected as non-read-only before execution"; the second branch is impossible,
and had the static list caught it the fixture would have gone green while this
RFC credited a mechanism that played no part.*

## Conditional Class-A outcomes

No transaction switches capability after observing state. Commands whose
outcome is decided by a guarded read/write are classified Class A for the whole
invocation and emit exactly one registered event for every committed outcome:

- login-failure recording emits `auth.login.failure` when the counter advances
  below threshold or `auth.lockout` when the same transaction crosses the
  threshold;
- refresh rotation emits `auth.refresh.rotated` for the winning normal
  rotation or `auth.refresh.theft_detected` when reuse triggers family
  revocation.

The sealed command declaration lists a closed result enum and generates the
command-associated event sum for each branch. The mutation primitive returns
the result enum from rows-affected/current state; exhaustive matching produces
one `C::Event` variant. The Class-A runner has no “commit without event”
variant, and an unhandled future enum variant is a compile error. Failure/no-op
outcomes that do not commit return `Err` before append and roll back.

Commands that are genuinely distinct operator intents receive distinct IDs
even when they share one repository setter: client enable versus disable, and
registration-token issue versus revoke. Parameterized state-result commands
that remain one intent (user creation with/without HIBP warning, federation
provider enable/disable, federation-link upsert create/update) register every
closed event branch and one rollback test per branch in the inventory.

### Accepted RFC 096 federation command amendment

RFC 096 adds one new Class-A operator intent and a closed family of federation
login commands. It does not overload C17 or compose nested Class-A commands.

**C17 enable amendment** eliminates a standalone durable preflight writer.
Production preflight returns a private non-cloneable/non-debuggable/
non-serializable `ValidatedProviderPreflight` only after complete RFC 096
transport, discovery, and JWKS validation. It binds provider ID/version, exact
disabled-state activation generation, complete durable-policy digest, trusted
observation time, and bounded metadata/
key-set fingerprints. Failure, cancellation, or restart creates/preserves no
authorization evidence.

The C17 enable Class-A transaction consumes the capability and, after
`BEGIN IMMEDIATE`, captures `guard_at`; requires
`0 <= guard_at - observed_at < 600s`; rechecks exact disabled/non-deleted
provider/version/policy digest/activation generation; checked-increments that
generation; stores evidence bound to the new generation; enables; and appends
`federation.provider.enabled`. Exactly 600 seconds is stale. Disable requires a
currently enabled row and, in one C17 transaction, checked-increments the same
generation, clears evidence, disables, invalidates login attempts,
federated-MFA continuations, and ceremonies, and appends its event. An
already-disabled request does not commit or consume a generation.

`activation_generation` is a durable canonical-decimal checked `u64` separate
from policy `config_version`; new/migrated rows start at zero. Malformed,
noncanonical, missing, or maximum state fails closed. Distinct concurrent
preflights may exist, but the exact generation guard and enable increment
permit one winner and invalidate all sibling capabilities. Stale, replayed,
wrong-provider/policy/generation, deleted, or already-enabled input rolls back
without an event. C23 increments both version and generation. C18 increments
generation and invalidates evidence/flows before delete; a deleted/missing row
cannot match. Every increment, state/evidence/flow change, event, and audit
append shares the owning transaction, so overflow/failure/cancellation rolls
back all effects.

C17 retains its authorized administrator actor and internal provider target.
The enable event adds version, old/new activation generation, and bounded
fingerprint identifiers from the
sealed capability, never metadata/JWKS bodies or endpoints. The disable event
adds old/new activation generation and bounded invalidated
login-attempt/MFA/ceremony counts. C18 records old/new generation. Observation
time is
stored as typed evidence but arbitrary caller time cannot enter the event.

**C23 federation-provider trust-policy replacement** atomically validates the
expected current version and generation, checked-increments both
`config_version` and `activation_generation`, replaces the
complete typed trust policy, sets the provider disabled/review-required,
invalidates every pending/exchanging login attempt, federated-MFA continuation,
and bound ceremony for the old version, and
appends `federation.provider.policy_updated`. Missing/malformed/overflowed
counter, predicate loss, attempt invalidation failure, event failure, or audit
failure rolls back all database effects. Cache eviction occurs only after
commit. Old cache entries remain non-authoritative because every lookup binds
the durable provider version, activation generation, and enabled state;
eviction failure may deny but
cannot re-authorize the old version.

C23 runs after migrations and audit-chain readiness but before the affected
provider can be routed or preflighted. Its sealed system actor is
`StartupConfiguration`; the target is internal provider ID. Event attributes
are old/new version and activation generation, sorted changed-field enum names,
bounded invalidated
login-attempt/MFA/ceremony counts,
and resulting `disabled/requires_review` state. They exclude issuer/origin/
client identifiers, secret material, and configuration values. Its guarded
predicate requires exact provider ID, exact old version and activation
generation, non-deleted row, and the fully validated proposed policy; exactly
one provider row must change.

Federated authentication then uses exact commands F01–F06 from the attached
inventory:

- F01 is the Protocol-class existing-link/no-MFA promotion transaction;
- F02 is the Protocol-class existing-link/local-MFA-pending transaction;
- F03 is the Protocol-class local-MFA completion transaction;
- F04 is the Class-A first-provision transaction and emits exactly
  `auth.federation.provisioned`; and
- F05 is the Protocol-class terminal attempt failure/denial transition; and
- F06 is the Protocol-class federated WebAuthn ceremony creation/replacement.

### Phase assignment for F01–F06 — stated, not left to inference

*Added 2026-08-12 after the external correction review found that this RFC
classified F01–F06 but never said when they are built, and that RFC 096-B1's
"requires RFC 094 M2a" therefore rested on an inference nothing supported.*

**F01–F06 are not part of an RFC 094 conversion wave.** They appear in neither
M2a's priority waves (user administration; credential, consent and session
security; token and registration security; signing keys) nor M2b's remaining
waves (server and security settings; pending sensitive settings; federation
configuration; client administration metadata). That omission was an oversight,
not a deliberate silence.

**They are implemented by RFC 096-B1**, against the seam this RFC delivers, and
RFC 096 owns the federation login path. What this RFC owes them is the seam, not
the implementation:

| Needed by F01–F06 | Delivered by |
|---|---|
| `WriteTx<Protocol>` runner — F01, F02, F03, F05, F06 | **M2a foundation** |
| `WriteTx<AtomicAudit>` Class-A runner — F04 | **M2a foundation** |
| Sealed `declare_write_command!`, `ReadConn`, command manifest | **M2a foundation** |

All three are M2a **foundation** items, delivered before the priority conversion
waves. So RFC 096-B1's prerequisite is satisfiable at M2a — but the reason is the
runner foundation, **not** membership of the session-security conversion wave,
which is what RFC 096 previously claimed and what the reviewer correctly
rejected.

`federation configuration` in M2b remains the **provider administration**
commands C17/C18/C23, which is a separate concern from federation login and is
RFC 096-B2's dependency.

**Confirmation still required.** This states the assignment the artifacts imply;
it has not been independently confirmed. The specific question for review: is the
`WriteTx<Protocol>` runner genuinely complete at M2a foundation, or does any part
of it land with the M2b authority switch? If the latter, F01/F02/F03/F05/F06 move
and RFC 096-B1 with them.

F01–F03/F05/F06 keep the previously reviewed base-design U30/U32/U33/U24
protocol classification and
Class-B login-result policy. They nevertheless use one `WriteTx<Protocol>` per
listed compound transition so a consumed attempt or pending ceremony cannot be
separated from the corresponding session. Their post-commit
`auth.federation.signin.success` or fixed failure/denial event is must-attempt,
not represented as atomically audited. F04 is different: it creates durable
identity authority and therefore uses one Class-A transaction and one event.
It calls private mutation primitives rather than nesting U01, C19, or U30.

Newly provisioned users have no pre-existing local MFA factor, so F04 has no
MFA-pending branch. An upstream identity never auto-merges into a local user;
only an already existing `(provider_id, sub)` link can enter F02/F03.

`federation_link.last_seen_at` and last-seen upstream email become internal
observation primitives callable only inside F01, F03, or F04. They are not C19
link-policy updates and cannot create/reassign a link. Failure rolls back the
owning compound transaction. Invalid MFA and non-mutating denials produce no
login success; F05 owns only the guarded attempt transition and a separate
must-attempt observation. F06 owns only method-ceremony protocol state bound to
F02/F03.

The generated manifest must register the new command IDs, every subordinate
write site, closed event bindings, and cross-command negative fixtures.
F01/F02/F04 accept only an RFC 096 verified-identity/claimed-attempt capability;
F03 accepts only the corresponding one-time federated-MFA capability. Type and
runtime predicates reject using an F02 continuation with password MFA, nesting
C19/U01/U30 beneath F04, or completing/creating a session from a failed,
wrong-provider, wrong-user, expired, or already-consumed row.

F01/F02/F04 recheck that the provider is enabled at the attempt's exact
version and activation generation. F01/F02 additionally require the same active link and active user;
F04 rechecks absence of the provider/sub link, verified unique email, username
uniqueness, and non-admin result inside its transaction. F03 rechecks an
unexpired continuation, enabled exact-version-and-activation-generation provider, unchanged active link,
active user, and applicable enabled local factor. The row has a five-minute
exclusive expiry, durable `failure_count` constrained 0–5, and terminal status.
Its closed results are `Promoted`, `RejectedStillPending`,
`AttemptsExhausted`, and `Invalidated`. Each bound wrong method proof atomically
increments once; counts 1–4 remain pending and exactly 5 exhausts/destroys the
continuation plus bound ceremonies. A correct proof promotes only at counts
0–4. Authority drift/expiry invalidates. F03 never turns a denial branch into
password login. Terminal rows are unusable and purged after 24 hours; restart
does not reset count.

TOTP promotion updates a strictly newer durable step with the session.
Recovery promotion removes exactly the matched hash with the session. F06
creates/replaces one WebAuthn `FederatedLogin` ceremony bound to the F02 row,
provider/version/activation-generation/link/user/RP/origin/challenge and no-later expiry. F03 consumes
that exact ceremony and passkey counter only with session promotion. Wrong
WebAuthn consumes the failed ceremony and increments the shared F03 count;
wrong TOTP/recovery consumes no anti-replay authority. Method proof types are
sealed and non-substitutable.

A private bounded `FederatedMfaVerifier` is the sole producer of those proof
types. Only after browser/CSRF/pending/provider-version-generation/user binding
does it produce a sealed method-specific valid candidate or a sealed
`BoundRejected` with its own closed reason. Binding failure produces neither.
Handlers cannot construct either candidate or choose a row-touching rejection
reason; F03 accepts only the verifier output and rechecks authoritative state.

F03 post-commit Class-B variants are exactly
`auth.federation.signin.success`, `auth.federation.mfa_rejected`,
`auth.federation.mfa_exhausted`, and `auth.federation.mfa_invalidated`, with the
closed RFC 096 reason enums. F05 selects exactly
`auth.federation.signin.upstream_failure`,
`auth.federation.takeover_blocked`, `auth.federation.link_required`, or
`auth.federation.signin.denied` with those closed reason enums. No arbitrary
upstream/identity/proof value enters the payload.

F04's event actor is the newly created user through the sealed verified
federated principal and its target is that user. Bounded attributes contain
internal provider/link IDs and the fixed `provision_on_first_login` mode; they
exclude subject, email, username, issuer, token, and upstream text. No separate
`user.create`, `auth.federation.link.created`, or login-success event is
emitted for the same F04 intent.

### Refresh issuance and rotation boundary

Initial root-family refresh-token issuance is T09, a Protocol-class command
separate from rotation. It uses the shared insertion primitive under
`WriteTx<Protocol>`, has its own owner/rationale/test, and cannot call the T04
Class-A runner. Its risk acceptance is the same bounded non-audit-equivalence
decision as the other P rows.

T04 begins with a query-only preliminary read of immutable presented-token data,
then prepares every fallible non-database input before opening its synchronous
transaction: generate the successor raw token and identifiers, hash/seal it,
and build the complete successor row. The raw token lives in a secret wrapper
that zeroizes on drop and is returned only if the transaction commits; every
reuse/error path therefore performs zeroizing destruction rather than relying
on ordinary Rust `Drop`. The Class-A closure re-reads and validates the
authoritative row before mutation and never suspends while borrowing the write
transaction. Inside that transaction:

1. select the presented token and re-check expected client, expiry, and current
   revocation/family state;
2. **normal winner:** guarded revoke of the presented active row must affect
   exactly one row; insert the precomputed successor in the same family; build
   the `Rotated` event variant; append `auth.refresh.rotated`; commit;
3. **reuse:** if the row is already revoked, revoke every still-active family
   member; build the `TheftDetected` variant with the affected count; append
   `auth.refresh.theft_detected`; commit;
4. unknown, expired, wrong-client, or otherwise non-committing outcomes return
   `Err` and roll back without a success event.

Successor insertion failure or rotated-event append failure rolls back the old
row revocation. Family-revocation or theft-event failure rolls back the reuse
branch. Under N-way contention, one caller may commit `Rotated`; later losers
commit `TheftDetected` and revoke the winner's successor according to the
existing fail-safe family policy. Tests reconcile old/successor/family state
and exact event counts for both branches, including injection at guarded
revoke, successor insert, family revoke, audit append, and commit.

## Structural coverage gate

The new gate consumes a checked-in command inventory such as
`ci/audit-commands.toml` and a registry dump/test API. Each inventory row has:

- stable command ID and owning module/function;
- mutation repository surface;
- event kind and class;
- command-associated sealed event sum and closed descriptor set;
- failure-injection test identifier;
- capability/class (`atomic`, `protocol`, `operational`, `internal`, or
  `bootstrap`);
- threat/rollback notes.

CI runs `cargo +stable xtask audit-structure --locked --policy
ci/write-authority.toml --commands ci/write-commands.toml`. It fails if a source
write site lacks a generated command declaration, a manifest row lacks a
registry descriptor, two commands claim one exclusive binding, a Class-A
command lacks its failure test, generated documentation differs, or an
unapproved raw-write module exists. The generated declaration set must equal
the reviewed inventory set. Grep is not authoritative.

The M1 string script may remain for vocabulary drift diagnosis, but the M2 job
and documentation identify this structural gate as authoritative.

## Failure injection

`repos::audit` receives a test-only/injected fault seam at these points:

- before chain-head read;
- after mutation but before append;
- after hash construction but before audit insert;
- after audit insert but before commit;
- commit failure where the database harness can deterministically induce it.

For every Class-A inventory row, a parameterized test snapshots the relevant
domain rows and audit tail, injects each applicable fault, invokes the real
domain command, and asserts:

- the command returns failure;
- domain state is byte/field equivalent to the pre-state;
- no event or partial chain link was committed;
- retry without injection commits once and emits exactly once.

Concurrency-sensitive commands also test one winner, no false success event
from losers, and chain continuity.

## Signing-key rotation

Signing-key rotation has no filesystem private-key phase: private signing keys
remain sealed in SQLite. `K01` retires the old key, inserts/activates the new
key, and appends `signing_key.rotate` in one Class-A transaction. Injection
before append or commit leaves the prior active key unchanged; the unique-active
constraint and K01 test prove exactly one active key after success.

## Master-key rotation — out of scope

Offline master-key rotation, its journal, atomic file publication, old-key
custody, and startup crash recovery are **not** part of this RFC. They were
removed by the 2026-07-28 amendment and are owned by
[RFC 100](./100-master-key-rotation-recovery.md).

The split is by subject matter, not size. Every other part of this RFC answers
one question: does a mutation and its audit record commit or roll back together?
Master-key rotation answers a different one — can offline key rotation survive a
crash at any point? That is a filesystem-atomicity and recovery problem with its
own threat model (partial key material, mixed ciphertext, an unrecoverable
active key). Reviewed as an appendix to audit atomicity it would receive the
wrong kind of scrutiny.

`K04` and `K05` are removed from this RFC's inventory and move to RFC 100. Their
database phases will use the Class-A seam this RFC establishes, which is why
RFC 100 depends on M2a.

Until RFC 100 is Implemented, master-key rotation remains a manual operator
procedure. The operator documentation must state plainly that it is **not yet
crash-safe** and name the recovery steps required if rotation is interrupted.
Leaving the current documentation implying safety it does not have would be the
same false-assurance defect this RFC exists to correct.

## Data model and compatibility

No audit table replacement is required. Existing event names remain stable
unless the inventory review finds a provably misleading name; any rename is an
explicit documented alias/migration. Existing audit rows remain readable.

The registry may add a schema/version attribute to new events, but it must be
backward-compatible and must not place secrets in `note`. External consumers
receive a documented vocabulary delta before closure.

## Multiple implementation steps

1. **Inventory freeze:** independently reconcile and approve the attached
   exact write-command inventory, classifications, owners, threat delta, and
   generated write-site universe.
2. **Registry foundation:** add typed descriptors/payloads, documentation
   generation/checking, and duplicate/class tests without changing behavior.
3. **Transaction foundation:** add the sealed write-capability runners, the
   typed `ReadConn` wrapper with static denial, within-transaction repository
   mutations, and the failure injector; remove the misleading best-effort
   `audit_and` construction path.

### M2a — foundation and priority conversion

4. **Priority conversion waves:** user administration; credential, consent and
   session security; token and registration security **including C15 baseline
   atomic dynamic registration**; and signing keys (`K01`). Each wave follows
   the handoff checklist, carries its own rollback evidence before the next
   begins, and keeps the structural gate green over converted commands.

**C15 is pinned to M2a and may not be deferred to M2b.** RFC 095 builds directly
on that seam and cannot start without it.

### M2b — remaining conversion and authority switch

5. **Remaining conversion waves:** server and security settings; pending
   sensitive settings; federation configuration; client administration
   metadata.
6. **Authority switch:** land the `syn` AST boundary gate, make the structural
   gate blocking, downgrade/remove the string gate as authority, correct
   audit-chain claims, and collect the full adversarial evidence package.

The `syn` AST boundary check lands here rather than with the transaction
foundation. Its purpose is to detect a *newly introduced* bare write absent from
both inventory and registry, and that value exists only once the boundary it
protects exists; building it before conversion is premature. Until it lands, the
manifest↔registry agreement check is the interim control and the compiler
capability remains the primary one.

**The bound on that compiler capability during M2a, corrected 2026-08-26.** It
reaches the **crate** boundary at most: Rust privacy is per-module, not
per-caller, so it can never restrict raw access to the individually converted
functions. Inside `sui-id-store`, unconverted repository modules keep raw access
for all of M2a and no structural control prevents a new bare write there — only
review does. Reaching even the crate boundary additionally requires sealing
`Database`'s four raw closures **and** `backend::SqliteBackend::new`, which is
public and accepts a raw `rusqlite::Connection`; neither is sealed today. See
the Part A review §A2. The residual M2a carries is therefore precise: a new bare
write inside `sui-id-store`'s unconverted repository modules, caught by review
until the AST gate lands.

Stages 1–2 may be reviewed before conversion. No partial wave may claim Class-A
atomicity until its failure tests pass.

### Interim documentation honesty (normative)

Between M2a and M2b the audit coverage matrix and every derived document must
state, per command, whether it is converted, and must carry a visible statement
that conversion is partial. **No unconverted command may be described as
Class-A atomic.** Overstating coverage during the interval would recreate the
exact false-assurance defect this RFC exists to correct, and is a blocking
closure finding for M2a.

## Test plan

- Registry unit tests: stable names, unique descriptors, complete required
  fields, secret-type rejection, deterministic documentation output.
- Transaction-runner tests: mutation failure, event-build failure, every append
  injection point, commit failure, and successful exactly-once behavior.
- Compile/structural event tests: direct context construction, caller-supplied
  authority fields, wrong command/event binding, arbitrary kind, unmapped or
  duplicate variant mapping, and unauthorized system principal.
- Per-command parameterized rollback tests for every Class-A inventory row.
- Closed-branch tests cover every U01/U22/T04/C17/C19 and, under the accepted
  amended design, C17-preflight/C18/C23/F03–F06 result/event variant and prove
  no committed outcome can omit an event.
- Secret-lifecycle tests exercise every T04 non-returning branch through the
  zeroizing wrapper without logging, formatting, or comparing the raw token.
- T09 separately proves initial refresh issuance cannot enter T04 and retains
  its explicit Protocol-class risk rationale.
- Concurrency tests for guarded mutations and chain ordering.
- Class-B tests prove primary denial remains effective and append failure emits
  structured error telemetry.
- Migration tests prove historic rows remain readable and chain verification
  continues across old/new records.
- Structural-gate negative fixtures prove missing command, missing test,
  duplicate event, class mismatch, new bare SQL write, raw transaction use, and
  prepared UPDATE/writable PRAGMA/ATTACH/backup/indirect helper through
  `ReadConn` fail CI or runtime read-only validation before execution.
- Signing-key injection proves old-active rollback and exactly-one-active
  success. Master-key crash-recovery tests belong to RFC 100.
- Full RFC 093 matrix on the final clean commit.

## Security considerations

The highest-risk failure is false proof: types or documentation that say
“audited” while a mutation can commit alone. Private constructors, a single
transaction capability, post-commit receipts, and injected rollback tests bind
the claim to behavior.

Other threats are secret leakage through flexible notes, forged actor/target
fields, audit-chain forks, duplicate success records on retry, and bypass via a
legacy repository function. Typed payloads, scoped actor IDs, same-transaction
chain updates, guarded winner semantics, and visibility restrictions address
these threats. Independent review must search specifically for bare mutation
paths and raw append calls.

The local hash chain remains tamper-evident only within its trust boundary. It
does not provide external notarization or protection against an attacker who
can rewrite the database and all application evidence. Documentation must say
so plainly.

## Rollback

Registry-only stages can be reverted without data migration. Once a command is
converted, rollback to a best-effort append weakens a security invariant and
requires moving this RFC back to Proposed or a superseding emergency decision.
Event aliases preserve old-row readability. No rollback deletes audit rows or
repairs a chain by rewriting history.

## Acceptance criteria

Criteria are per stage. A criterion listed under M2a is not satisfied by
intention; it must be observed on one clean commit before M2a closes.

### Common to both stages

- The attached exact command inventory and generated write-site reconciliation
  are independently approved before acceptance and again before implementation.
- The typed registry is the sole source of Class-A event identity and class.
- Actor/command/time/correlation authority comes only from an explicit sealed
  `AuthorizedCommandContext<C>` and cannot be forged by a payload.
- `ReadConn` leaks no statement, raw handle, or write API, and its static denial
  rejects every forbidden query fixture.

### M2a

- No **converted** Class-A mutation bypasses the transaction capability.
- Every converted Class-A row has observed rollback evidence for injected audit
  failures and exactly-once success evidence.
- Baseline dynamic registration (C15) is Class-A atomic.
- Every conditional committed outcome among converted commands is Class A and
  exhaustively yields one typed event; distinct operator intents have distinct
  inventory IDs.
- **Raw database access is confined to `sui-id-store` by the dependency graph,
  not by visibility alone.** Owner decision, 2026-08-26. Three conditions:
  1. **`rusqlite` appears in no `[dependencies]` or `[dev-dependencies]` table
     outside `sui-id-store`**, asserted by a gate check that fails if it does.
     This is the primary control: a crate that does not depend on `rusqlite`
     cannot name `Connection`, `Transaction` or `Statement`, so no visibility
     change, feature flag, or later-added accessor can reopen the path, and
     undoing it requires a `Cargo.toml` edit that is visible in review.
  2. `Database::with_conn`, `with_tx`, `with_conn_sync`, `with_tx_sync` and
     `backend::SqliteBackend::new` are `pub(crate)`. The `Backend` trait may stay
     public as the pluggable-backend extension point: `Database`'s backend field
     is private with no accessor, so a published trait grants nothing without an
     instance.
  3. Tests outside `sui-id-store` assert through `ReadConn` or the command seam.
     **No `test-support`-style feature may re-export raw access** — a feature
     enabled in a production build would silently undo condition 2, and this
     workspace has already been bitten once by `--all-features` concealing a gap
     (RFC 093 G07b).

  *This replaces "raw database writes are compiler-capability-gated", which
  claimed a control that did not exist. Measured 2026-08-26: the four raw
  closures were `pub`, `backend::SqliteBackend::new` accepted a raw
  `Connection`, `MasterKey::from_base64` was public, and `rusqlite` was a
  production dependency of both `sui-id-core` and `sui-id` — so any production
  module in either crate could build its own backend over the same database file
  and write freely through public API alone. See the Part A review §A2.*

  *Known work, both small: `sui-id-core` names `rusqlite` in exactly one place,
  `audit_guard.rs`'s `audit_and_tx(tx: &rusqlite::Transaction<'_>, …)`, which is
  already this RFC's own conversion target — mutation primitives receive the
  sealed capability, not a raw `Transaction`, so the dependency dissolves as
  planned work. `sui-id` names it only in `backup/`, which opens database files
  for snapshot and integrity checking rather than serving application writes;
  where that module belongs is a separate owner decision and is tracked in the
  handoff.*

  *Bound on what this closes, so M2a is not read as more than it is: it makes
  raw access impossible **outside** `sui-id-store`. Inside that crate,
  unconverted repository modules keep raw access until their wave converts, and
  only M2b's AST gate detects a new bare write there. The residual is bounded to
  one crate whose declared purpose is database access.*
- **`ReadConn` rejects every data-modifying statement at runtime.** Negative
  fixtures must observe rejection of `INSERT … RETURNING`, `UPDATE … RETURNING`,
  `DELETE … RETURNING` and `REPLACE … RETURNING`, in addition to the existing
  PRAGMA, ATTACH/DETACH, multi-statement, batch, backup/restore, raw-handle and
  indirect-helper cases. An ordinary `SELECT` is the control fixture and must
  pass. Rejection must be observed to occur **before** the statement is stepped.
- **`rusqlite`'s `functions`, `vtab` and `load_extension` features are disabled**
  in the resolved dependency graph, asserted by a gate check that fails if any is
  enabled. This is what makes `ReadConn`'s read-only guarantee complete rather
  than contingent on an unwatched manifest line.
- The structural gate passes over converted commands and agrees with the
  registry and generated documentation for those commands.
- The audit coverage matrix states conversion status per command and carries a
  visible partial-conversion statement. No unconverted command is described as
  Class-A atomic.

### M2b

- Every remaining Class-A inventory row is converted with the same rollback and
  exactly-once evidence.
- **No production Class-A best-effort append or downstream exception remains
  anywhere.**
- The AST negative fixture proves a write omitted from both manifest and
  registry is rejected.
- The structural gate is blocking, and it, the generated/reference
  documentation, and the registry agree in full.
- Audit-chain and atomicity claims describe the implemented boundary exactly,
  and the partial-conversion statement is removed because it is no longer true.
- Independent closure review accepts adversarial, rollback, concurrency, and
  clean-matrix evidence.

Master-key rotation crash recovery is no longer an acceptance criterion of this
RFC; it moved to RFC 100 with the 2026-07-28 amendment.

## Open questions

None. Exact Rust generic spelling and module placement are implementation-level
choices so long as the capability, construction, and transaction invariants
above remain intact.
