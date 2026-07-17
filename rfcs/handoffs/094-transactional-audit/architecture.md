# RFC 094 implementation architecture

## Component boundary

```text
HTTP/CLI handler
    │ validated command + scoped Actor
    ▼
core domain command
    │ Database::class_a(AuthorizedCommandContext<C>, closure)
    ▼
private WriteTx<AtomicAudit> capability
    ├── mutation_repo::*_within_tx(...)
    ├── typed event payload from actual result
    └── audit::append_within_tx(...)
             │
             ▼
       single SQLite commit
             │
             ▼
       Audited<T> returned
```

Handlers do authorization, input validation, and response mapping. Domain
commands own the logical mutation and typed event choice. Repositories own SQL
but not actor policy or event vocabulary. The registry owns stable identifiers,
class, and payload requirements. Only the transaction runner can construct the
receipt used by `Audited<T>`.

## Proposed module responsibilities

| Surface | Responsibility | Forbidden |
|---|---|---|
| `core::audit_registry` | typed event kinds, descriptors, payload conversion | arbitrary event strings; secret-bearing attributes |
| `core::audit_guard` | `Audited<T>`, sealed receipt, Class-B must-attempt helper | claiming atomicity after async append |
| `store::db` | expose query-only `ReadConn` returning owned mapped values; dispatch sealed A/P/O/X write capabilities; test fault hooks | raw connection/transaction/statement escape; async suspension while transaction is borrowed |
| `store::repos::audit` | chain-head read, hash, append on caller transaction | opening a second connection/transaction |
| mutation repositories | declared command bodies scoped to sealed capability | public bare write entry points of any class |
| structural gate | generated write-site universe plus inventory/registry/test/doc agreement | manifest-only or grep-only assurance |

Names may change, boundaries may not.

## Transaction algorithm

1. Validate and authorize before acquiring the database writer when those
   checks do not depend on mutable transaction state.
2. Consume a private-field `AuthorizedCommandContext<C>` produced by a
   successful authorization decision (or sealed permitted system adapter), and
   call `Database::class_a(context, closure)`; start one immediate transaction.
3. Re-check state whose validity can race (existence, version, active status,
   rows-affected guard) inside the transaction.
4. Execute mutation using only the transaction capability.
5. Construct one variant of the command-associated sealed `C::Event` sum from
   the authoritative mutation result.
   Payloads contain target/result/attributes only; actor, command ID,
   correlation ID, and time cannot be caller-supplied.
6. Resolve the type-level `C` descriptor; the runner supplies authority fields
   from the context, validates payload fields, reads chain head, computes hash,
   and inserts the row on the same transaction.
7. Commit. Only after commit create the private receipt and return `Audited<T>`.

Never call an async logger or network/file API while holding the rusqlite
transaction. Raw rusqlite handles exist only inside the private command
executor. `ReadConn` returns only owned mapped results, validates SQLite
read-only status internally, denies writable PRAGMA/ATTACH/backup/restore and
never returns a statement or raw handle. Prepare external file changes before
the transaction and activate them through the RFC's explicit recoverable
master-key state machine after commit.

## Registry data model

Each descriptor needs stable kind, serialized name, class, actor requirement,
target requirement, allowed result vocabulary, payload schema/version, and
documentation text. Prefer enums/newtypes for identifiers and bounded enums or
validated strings for reasons. Never implement `From<String>` for event kind.

The inventory binds a logical command to one closed descriptor set/capability
and one rollback or exclusion test per event variant.
Do not infer completeness from every descriptor being used: one event can have
multiple command paths, and every path must be inventoried.

## Error behavior

- Validation/authorization failure before mutation: no Class-A success event.
  A separate Class-B denial event may be required by the registry.
- Guarded mutation loses race: roll back/no-op and no success event.
- Mutation/event/append/commit failure: return internal failure; both durable
  effects absent.
- Class-B append failure: preserve the primary denial/detection and emit
  structured error telemetry with event kind, never its sensitive payload.

Conditional guarded commands never switch A/P capability. Login-failure
recording and refresh rotation are Class A for all committed outcomes. Their
sealed result enums exhaustively select one registered payload; no committed
variant lacks an event.

T04 performs a query-only preliminary refresh-row read, prepares the successor
and zeroizing secret wrapper without holding a write transaction, then re-reads
and validates authoritative state inside the synchronous Class-A closure. No
async preparation occurs while the transaction is borrowed.

## Visibility migration

Introduce the private executor, `ReadConn`, sealed declaration macro, and
capability runners first; convert all call sites, then remove general
`with_conn`/`with_tx` and bare mutation functions. A temporary compatibility
wrapper may exist only inside an active conversion branch and must be listed in
the inventory with an expiry in the same wave. It cannot survive M2 closure.

## External key state

Signing-key rotation is database-only and uses one Class-A transaction.
Master-key rotation follows RFC 094's two-command state machine: atomically
create an exclusive private workspace; write/fsync/readback and no-replace
publish the authenticated Intent from a temporary; publish verified next/backup
artifacts from workspace temporaries; K04 database reseal/pending audit; atomic
active-file replacement and directory fsync; then K05 completion audit. Startup recovery
runs before service readiness and validates key fingerprints, sentinel
decryption, canonical paths, ownership, permissions, symlink absence, and the
database state. A workspace containing only a partial unpublished Intent is
safe to clean by its exact reserved-path contract; every authenticated prefix
resumes or cleans back to OldReady. K05 also
records an explicit finite-review retention/custody or verified-removal
disposition for the old-key backup.
