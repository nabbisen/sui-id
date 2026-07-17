# RFC 094 — Transactional Audit Completeness and Typed Event Registry

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFC 093 may be reviewed jointly; its gate contract must be Accepted before this RFC is accepted; the attached durable-write inventory, threat delta, and key-recovery state machine require independent design approval.
**Implementation prerequisites.** RFC 093 is Implemented and its clean-tree matrix passes; this RFC is Accepted; the Class-A inventory and threat delta have independent approval.
**Closure prerequisites.** Every Class-A production path uses the approved transaction seam; injected append failures roll back mutation for every inventory row; structural coverage passes; independent adversarial closure review accepts durable evidence.
**Tracks.** ROADMAP M2 — Transactional security records.
**Touches.** `crates/sui-id-core/src/audit_guard.rs`, `events.rs`, privileged domain services, `crates/sui-id-store/src/db.rs`, `repos/audit.rs`, mutation repositories, audit documentation and CI.
**Handoff.** [`../handoffs/094-transactional-audit/README.md`](../handoffs/094-transactional-audit/README.md)
**Command inventory.** [`../handoffs/094-transactional-audit/command-inventory.md`](../handoffs/094-transactional-audit/command-inventory.md)
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Implementation owner.** `codex-developer` (OpenAI Codex), after acceptance and prerequisites.
**Independent security and closure reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex).

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
| Key material | master-key reseal/pending and activation-completion phases |
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
6. Before a read query executes, the private wrapper prepares it internally and
   requires SQLite's `sqlite3_stmt_readonly` result (via the safe rusqlite
   equivalent) to be true. Multi-statement SQL and trailing executable text are
   rejected. PRAGMAs are denied by default; the versioned read-only allowlist
   contains only introspection forms whose prepared statement is also reported
   read-only. `ATTACH`, `DETACH`, writable PRAGMAs, load-extension, backup,
   restore, serialize/deserialize replacement, transaction control, batches,
   and FFI/raw-handle access are unavailable through `ReadConn`.
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
it. Additional fixtures attempt a prepared `UPDATE`, writable PRAGMA, ATTACH,
backup API, returned raw statement, and an indirect helper accepting a raw
statement/connection through `ReadConn`; they must fail to compile or be
rejected as non-read-only before execution. A read-only SELECT control must
still succeed.

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

## Master-key and signing-key recovery state machines

Signing-key rotation has no filesystem private-key phase: private signing keys
remain sealed in SQLite. `K01` retires the old key, inserts/activates the new
key, and appends `signing_key.rotate` in one Class-A transaction. Injection
before append or commit leaves the prior active key unchanged; the unique-active
constraint and K01 test prove exactly one active key after success.

Master-key rotation is offline and uses two truthful Class-A commands around an
atomic filesystem replacement. Add a singleton `master_key_rotation` row with
`rotation_id`, old/new non-secret key fingerprints, state
`db_committed_pending_activation | complete`, prepared/committed/completed
timestamps, and backup filename. The state machine is:

```text
OldReady
  ──exclusive workspace + atomically published Intent/artifacts──> Prepared
  ──K04 reseal DB + pending row + audit, one tx──> DbCommitted
  ──atomic replace active file + directory fsync──> FileActivated
  ──K05 mark complete + audit, one tx──> Complete
```

Preparation uses an exclusive workspace and atomically publishes the journal
before any externally published key artifact. After generating/loading the new
key in memory, the command atomically creates the reserved directory
`<active>.rotation-work` with mode 0700/current owner, no symlink following,
and fsyncs the parent. Its existence is the rotation lock. If it already
exists, a new rotation refuses and recovery owns it.

Inside that workspace, the command writes `intent.write` with checked
write-all semantics, fsyncs it, reads it back, and verifies both authentication
tags. It then uses the platform's atomic no-replace publication to rename it to
`intent.json` and fsyncs the workspace directory. A crash/short write before
publication can leave only a non-authoritative temporary inside the reserved
workspace; it cannot create a partial published journal. The published journal
contains no key material. It binds canonical active/workspace/next/backup
paths, every reserved temporary path, rotation ID, old/new fingerprints,
expected owner and mode, requested backup disposition, and monotonic
preparation state:

```text
Intent -> NextTempVerified -> NextPublished
       -> BackupTempVerified -> BackupPublished -> Prepared
```

It contains domain-separated authentication tags derived independently from
the old and new keys, so recovery can authenticate it with whichever valid
candidate decrypts the database sentinel. Journal creation and every state
advance use a separate reserved update temporary, checked write-all, fsync,
readback/tag verification, atomic replacement, and workspace-directory fsync.
A crash during an update leaves either the prior authenticated state or the new
one. A partial update temporary is safe to delete because the authenticated
base journal binds its exact workspace path and remains authoritative.

After `Intent` is durable, preparation writes the new key to the bound
`next.write` inside the workspace with create-new, 0600/current-owner,
no-symlink and checked write-all semantics; fsyncs and reads it back; verifies
its fingerprint/encoding; then records `NextTempVerified`. It atomically
renames without replacement to `next.ready`, fsyncs the workspace, and records
`NextPublished`. A crash during rename or directory fsync leaves the prior
journal state; recovery validates whether `next.write` or `next.ready` exists,
re-fsyncs as needed, and either advances or deletes only the bound temporary.

The old-key backup follows the same protocol. Write `backup.write` inside the
workspace, fsync/readback and verify old fingerprint plus sentinel, record
`BackupTempVerified`, atomically publish without replacement to the bound final
backup path on the same filesystem, fsync its parent, then record
`BackupPublished` and `Prepared`. A short write never reaches a published path.
Journal state, exact paths, fingerprints, ownership, and modes must agree with
observed artifacts before K04.

Offline rotation is supported only when the configured file is the
authoritative active key. If environment/key-provider precedence supplied the
old key, the command refuses before `Intent` and requires that provider's
separately reviewed atomic activation procedure; it never writes a file that
startup would ignore.
The active, workspace, next, journal temporaries, and backup paths must share a
filesystem whose supported platform adapter provides durable create, atomic
replacement, atomic no-replace publication, and file/directory fsync. Otherwise
rotation refuses before creating the workspace.

K04 opens with the old key, reseals every encrypted column plus sentinel under
the new key, writes `db_committed_pending_activation`, and appends
`admin.master_key.database_resealed` in one transaction. That event states only
that the database phase committed and file activation is pending.

Activation atomically replaces the active key path with the already-fsynced
workspace `next.ready` file without first removing the active path, then fsyncs
the directory.
On platforms where secure atomic replacement with the required ownership/mode
cannot be guaranteed, rotation refuses to start. K05 reopens with the active
new key, marks the row `complete`, and appends `admin.master_key.activated` in
one transaction. CLI success is printed only after K05 commits.

Startup recovery runs before normal database open:

| Observed state | Recovery |
|---|---|
| No workspace | Normal OldReady/Complete startup; no preparation prefix exists. |
| Workspace exists; no published authenticated `intent.json` | Verify exact reserved path, directory owner/mode/no-symlink, old active key/DB state, and that entries are only reserved partial intent/update temporaries; delete those regular files and remove/fsync the workspace to return OldReady. Any unexpected entry fails closed. |
| `Intent`; partial `next.write` | Authenticate journal with old active key; delete/rewrite only the bound partial temporary. Resume with an operator-provided matching new key or clean the workspace to OldReady; a lost generated key is not regenerated under the old fingerprint. |
| `Intent`/`NextTempVerified`; `next.write` or `next.ready` | Re-read fingerprint/encoding and owner/mode; complete no-replace publication/directory fsync and advance, or delete the bound temp and clean to OldReady. |
| `NextPublished`; partial `backup.write` | Revalidate `next.ready`; delete/rewrite only the bound partial backup temporary, then resume or clean to OldReady. |
| `NextPublished`/`BackupTempVerified`; backup temp/final exists | Verify old fingerprint/sentinel and owner/mode; complete no-replace publication/directory fsync and advance, or clean according to the bound disposition. |
| `BackupPublished`/`Prepared`; DB/active both old | K04 did not commit: validate all artifacts; resume K04 or remove `next.ready`/journal/workspace. Backup follows the bound disposition and is never guessed from its filename. |
| DB new/pending; active old; `next.ready` new | verify fingerprints/sentinel, atomically activate `next.ready`, fsync, then run K05. |
| DB new/pending; active new | replacement completed before crash: run K05 idempotently. |
| DB new/complete; active new | normal startup; remove only matching stale journal metadata. |
| Any fingerprint, ownership, permission, symlink, sentinel, or state mismatch | fail closed before service readiness; preserve all files for operator recovery. |

Before `Intent`, the operator selects and the journal binds one old-key backup
disposition: `RetainUntil { review_at, custody_reference }` or
`RemoveAfterVerified { verification_reference }`. K05 cannot mark Complete
without recording the disposition in `master_key_rotation`. A retained backup
is never considered an active startup candidate and produces an operator/M6
finding after `review_at` until renewed or removed. Removal unlinks the file
only after the referenced pre-rotation backup is retired or re-encrypted; the
design makes no secure-erasure claim for flash/copy-on-write storage. A removed
backup records timestamp and operator identity. Thus an old decryption key is
retained only by an explicit reviewable custody decision, not indefinitely by
accident.

Fault tests inject short writes and termination during workspace creation,
every initial/update temporary write, file fsync, readback verification,
no-replace/replace publication, directory fsync, journal transition, K04
mutation/append/commit, active replacement, and K05 append/commit. Every
pre-K04 prefix—including a workspace with partial unpublished intent—must
idempotently resume or cleanly return OldReady; every post-K04 prefix must reach
Complete. Service never starts with unreadable/mixed ciphertext or overstated
completion.

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
3. **Transaction foundation:** add the sealed write-capability runners,
   read-only connection wrapper, AST boundary gate, within-transaction
   repository mutations, and failure injector; remove the misleading
   best-effort `audit_and` construction path.
4. **Conversion waves:** users/sessions; clients/settings; keys/federation;
   token/credential paths. Each wave follows the handoff checklist and keeps
   the structural gate green.
5. **External/key and registration closure:** implement the master-key recovery
   state machine and baseline atomic dynamic-registration command.
6. **Authority switch:** make the structural gate blocking, downgrade/remove
   the string gate as authority, correct audit-chain claims, and collect the
   full adversarial evidence package.

Stages 1–2 may be reviewed before conversion. No partial wave may claim Class-A
atomicity until its failure tests pass. M2 closes only after every Class-A row,
including dynamic registration, is converted with no best-effort exception.

## Test plan

- Registry unit tests: stable names, unique descriptors, complete required
  fields, secret-type rejection, deterministic documentation output.
- Transaction-runner tests: mutation failure, event-build failure, every append
  injection point, commit failure, and successful exactly-once behavior.
- Compile/structural event tests: direct context construction, caller-supplied
  authority fields, wrong command/event binding, arbitrary kind, unmapped or
  duplicate variant mapping, and unauthorized system principal.
- Per-command parameterized rollback tests for every Class-A inventory row.
- Closed-branch tests cover every U01/U22/T04/C17/C19 event variant and prove
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
- Master-key crash tests cover every transition/fault row above and signing-key
  injection proves old-active rollback/exactly-one-active success.
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

- The attached exact command inventory and generated write-site reconciliation
  are independently approved before acceptance and again before implementation.
- The typed registry is the sole source of Class-A event identity and class.
- No production Class-A mutation bypasses the transaction capability.
- Every Class-A inventory row has observed rollback evidence for injected audit
  failures and exactly-once success evidence.
- Baseline dynamic registration is Class-A atomic in M2; no production Class-A
  best-effort append or downstream exception remains.
- Raw database writes are compiler-capability-gated and the AST negative
  fixture proves a write omitted from both manifest and registry is rejected.
- `ReadConn` leaks no statement/raw handle/write API and runtime SQLite
  read-only validation rejects every forbidden query fixture.
- Every conditional committed outcome is Class A and exhaustively yields one
  typed event; distinct operator intents have distinct inventory IDs.
- Actor/command/time/correlation authority comes only from an explicit sealed
  `AuthorizedCommandContext<C>` and cannot be forged by a payload.
- Master-key rotation recovers from every database/file crash boundary without
  mixed ciphertext, missing active key, or overstated audit completion; every
  preparation prefix is journaled first and old-key custody/removal is explicit.
- The structural gate, generated/reference documentation, and registry agree.
- Audit-chain and atomicity claims describe the implemented boundary exactly.
- Independent closure review accepts adversarial, rollback, concurrency, and
  clean-matrix evidence.

## Open questions

None. Exact Rust generic spelling and module placement are implementation-level
choices so long as the capability, construction, and transaction invariants
above remain intact.
