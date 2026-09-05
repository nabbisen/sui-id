# RFC 094 migration checklist

Complete Stage 0 before changing production behavior. Convert one bounded wave
at a time; the workspace and structural gate must remain green between waves.

## Stage 0 — inventory freeze

> **Completion marking, added 2026-09-05.** Items carry `**Done <sha>**` when
> landed. Until today none did — Stage 0, most of Stage 1 and Stage 2 item 1 were
> complete while every box stayed empty, because the implementation role
> correctly left ticking to the architect and the architect never did it. Anyone
> reading this file would have had to reconstruct completion state from review
> documents. `*(proving slice)*` means the item is satisfied against the
> five-command Stage 1 slice and scales during the conversion waves — it is not a
> claim about all 92 commands.


- [x] Enumerate every production durable write entry point across core, store,  **Done `d70ead3`.**
  web handlers, CLI/setup, background tasks, and federation/registration.
- [x] Reconcile every row in `command-inventory.md` to exact Rust function and  **Done `d70ead3`.**
  SQL write-site IDs in `ci/write-commands.toml`.
- [x] Reclassify any current Class-B row that performs a durable security  **Done `d70ead3`.** *(swept; no row required reclassification, stated rather than manufactured)*
  mutation, or split mutation and observation into Class A and B.
- [x] Confirm `client.dynamic_register` is Class A in M2; RFC 095 owns later  **Done `d70ead3`.**
  metadata/concurrency completion, not an audit-atomicity exception.
- [x] Independently review the inventory against code and approve the threat  **Done `d70ead3`.**
  delta before Stage 1.

## Stage 1 — registry foundation

- [x] Add event kind and class enums plus deterministic descriptors.  **Done `5b986c9`.** *(proving slice)*
- [x] Add typed payloads with actor/target requirements and bounded attributes.  **Done `5b986c9`.** *(proving slice)*
- [x] Prove secret types cannot be **implicitly** coerced into payload attributes:  **Done `52eae0f`.** *(sealed `AttributeValue`)*
      a secret-bearing type must not satisfy the attribute API's bound, proven by
      a compile-fail fixture. *Narrowed 2026-08-28. This read "cannot be
      formatted/coerced", which claims more than any test can discharge —
      `expose_secret()` exists precisely so a caller can deliberately produce a
      `String`, and no type-level control can prevent that. The realistic mistake
      is passing the secret directly, and that is what must be blocked. Do not
      write a fixture asserting the stronger claim; it would pass while proving
      something narrower than it says.*
- [x] Generate or mechanically verify audit reference documentation.  **Done `5b986c9`.** *(generator built and tested; deliberately not wired to a tracked file until the registry is worth documenting)*
- [x] Add duplicate-name, class-mismatch, missing-field, and stable-serialization  **Done `5b986c9`.** *(proving slice)*
  tests.
- [ ] Add the checked-in command inventory and structural comparison tool.

      **Also check reserved event-variant field names here.** RFC 094 requires
      `C::Event` to carry no `actor`, `command_id`, `correlation_id`,
      `request_id` or `timestamp` field. `declare_write_command!` rejects those
      at expansion (verified 2026-09-05 by injecting `actor` into K01 and
      observing the error), but that check **can never have a standing
      compile-fail fixture**: the macro's expansion needs `pub(crate)` items, so
      no external harness — trybuild or doctest, both separate crates — can
      invoke it. Its only proof to date is a one-time manual injection.

      A source-scanning tool has no such wall. Hand this gate a fixture file
      containing a variant with a reserved field and assert it rejects. Until
      that lands, this guarantee has strictly weaker regression protection than
      every other sealed property in the module — bounded and tracked here
      rather than accepted as permanent.

      **One declared command has no inventory row, by design.**
      `ProofOnlyForbiddenSystemPrincipalCommand`
      (`PROOFONLY-NOT-AN-INVENTORY-ROW`) exists solely so the
      `system_principal: forbidden` compile-fail fixture has a type to name from
      outside the crate; it cannot be declared inside the fixture because
      `declare_write_command!` expands `impl sealed::Sealed` and `sealed` is
      `pub(crate)`. It will fail this tool's "declared command with no inventory
      row" check the day the check works.

      Exempt it **by exact command ID, as a single-entry allow list, with the
      reason recorded, and fail if the list grows**. Do not pattern-match
      `PROOFONLY*` — a prefix rule would silently admit a second one, and the
      whole point of this tool is that nothing reaches the database without an
      inventory row. *Recorded 2026-09-03, before the gate exists, so it is not
      met later as a puzzling failure and worked around.*
- [x] Add `ReadConn`, sealed `declare_write_command!`, A/P/O/X runners, and the  **Done `5b986c9`.** *(proving slice)*
  exact reviewed raw-write module policy.
- [x] Ensure `ReadConn` returns only owned mapped rows, requires SQLite  **Done `5b986c9`.**
  read-only statements, and denies statements/batches, writable PRAGMA,
  ATTACH/DETACH, backup/restore, extension, raw-handle, and FFI access.
- [ ] **M2b only — not an M2a gate.** Observe compile/AST failure for a new bare
  write absent from inventory and registry. The `syn` AST boundary gate lands
  with M2b's authority switch (RFC 094 §M2b).

  **M2a's interim boundary control**, which *is* blocking: the sealed
  `declare_write_command!` capability is the only production constructor for
  `WriteTx`, and `ReadConn` rejects every data-modifying statement at runtime via
  `sqlite3_stmt_readonly` — measured 2026-08-26, see the Part A review §A1 for
  the per-statement table and for the forms (`ATTACH`, `DETACH`, transaction
  control) that only the static denial list catches.

  **What it does not do — stated plainly, because the previous wording claimed
  otherwise.** This paragraph read "a new bare write in M2a therefore cannot
  compile outside the reviewed module list." That was false on two counts, both
  established against the code on 2026-08-26:

  1. `Database::with_conn`/`with_tx`/`with_conn_sync`/`with_tx_sync` are `pub`,
     not `pub(crate)`. Three production call sites outside `sui-id-store` use
     them today: `sui-id/src/http/handlers/index.rs` (health probe),
     `sui-id-core/src/oidc/key_rotation.rs`, and
     `sui-id-core/src/account/forgot_password.rs`.
  2. Sealing those four would still not close it. `lib.rs` exports
     `pub mod backend`, which exposes
     `SqliteBackend::new(rusqlite::Connection, MasterKey)` and public
     `with_conn_sync`/`with_tx_sync`; `MasterKey::from_base64` is public; and
     `rusqlite` is a `[dependencies]` entry — not dev — in both `sui-id-core`
     and `sui-id`. Any production module in either crate can therefore build its
     own backend over the same database file and write freely, using public API
     only, with no `unsafe` and no test feature.

  Rust's privacy is per-module, not per-caller, so the strongest boundary this
  control can express is the **crate** boundary — never "only the converted
  functions." Inside `sui-id-store`, unconverted repository modules retain raw
  access throughout M2a by construction, since the application cannot stop
  serving settings changes while M2b is pending, and **nothing structural
  prevents a new bare write in those modules during M2a**. That is what M2b's
  AST gate supplies. It does not sharpen a control M2a has; it provides one M2a
  lacks.

- [ ] **M2a exit condition — confine raw database access by the dependency
  graph, not by visibility** (owner decision, 2026-08-26; see RFC 094's M2a exit
  criteria). This is an **exit** condition, not an entry one: two of its three
  blockers dissolve as conversion proceeds, so it costs little if sequenced last.
  - [x] ~~Convert `audit_guard.rs`'s `audit_and_tx` to take the sealed
        capability instead of `&rusqlite::Transaction<'_>`.~~ **Achieved by
        deletion, not conversion — done in Stage 2 item 3.** `audit_guard.rs` had
        zero callers, so it was removed rather than converted, and
        `sui-id-core`'s `rusqlite` dependency dropped as this item predicted.
        `grep -rc rusqlite crates/sui-id-core/` is now 0 and the manifest line is
        gone. **This is the first piece of the crate-confinement exit condition to
        land, and it cost nothing** — which is the property that made option E
        affordable: its blockers dissolve as conversion proceeds rather than
        needing separate work.
  - [ ] `sui-id-core/src/account/forgot_password.rs` and
        `oidc/key_rotation.rs` stop calling `with_tx`/`with_tx_sync`. Both are
        already inside M2a's own waves (credential/session security, and signing
        keys), so no extra conversion work is created.
  - [ ] Replace `sui-id/src/http/handlers/index.rs`'s `SELECT 1` health probe
        with a `ReadConn` probe or a `Database::health_check()`.
  - [ ] **Owner decision required:** where `crates/sui-id/src/backup/` belongs.
        It opens database *files* for snapshot and integrity checking rather than
        serving application writes — a genuinely different category. Either it
        moves into `sui-id-store` (which already owns database files) or it is
        declared a reviewed raw-access module with recorded justification. If it
        moves, it carries an **observational-equivalence record** meeting RFC
        096's bar as strengthened on 2026-08-27 — including a triggering case for
        every security check in the moved code, since outcome coverage alone does
        not detect a dropped check that no scenario exercises. Same hazard as the
        RFC 096 federation split, where relocating code can silently drop a
        check.
  - [ ] Give the 11 e2e call sites in `crates/sui-id/tests/` a `ReadConn`-based
        assertion path. **No `test-support`-style feature may re-export raw
        access** — a feature enabled in a production build would silently undo
        the whole control, and `--all-features` has already concealed one gap in
        this workspace (RFC 093 G07b).
  - [ ] `Database::with_conn`/`with_tx`/`with_conn_sync`/`with_tx_sync` and
        `backend::SqliteBackend::new` become `pub(crate)`. The `Backend` trait
        may stay public: `Database`'s backend field is private with no accessor.
  - [ ] Add the gate check that **fails if `rusqlite` appears in any
        `[dependencies]` or `[dev-dependencies]` table outside `sui-id-store`**.
        This is the control that makes the rest durable; without it the boundary
        is a convention again. **Dispatched as a condition of
        `cargo xtask audit-structure`**, not as a separate script or lane — see
        RFC 094's Structural coverage gate section for why. Read every crate
        manifest in the workspace, not only the three that name `rusqlite`
        today; the check exists to catch the fourth.
- [x] Observe compile/runtime rejection for prepared UPDATE, writable PRAGMA,  **Done `5b986c9`.** *(fixtures grouped by controlling mechanism, per the 2026-08-28 measurement)*
  ATTACH, backup API, returned raw statement, and indirect raw-helper attempts.

## Stage 2 — transaction foundation

- [x] Add private `WriteTx<AtomicAudit>` capability and `Database::class_a`  **Done `5b986c9`.** *(proving slice)*
  runner taking private-field `AuthorizedCommandContext<C>` explicitly.
- [x] **Gate `AuthorizedCommandContext` construction per command.** RFC 094 §  **Done `d3003db`.**
      *Class-A transaction seam* requires it to be created "only by consuming a
      successful authorization decision for command type `C`, or by a sealed
      CLI/system authority adapter for commands whose **descriptor permits that
      principal**". The Stage 1 slice ships `for_system_actor` as the only
      constructor, ungated — so the descriptor does not yet decide anything, and
      the property is *unrepresentable* rather than merely unenforced.

      Add the principal to the descriptor and make the system-authority
      constructor reject a command whose descriptor does not permit it. **Do this
      before converting commands beyond the Stage 1 slice**: every command
      converted against the ungated shape is a call site written against a
      signature that is about to change.

      Needs a negative proof, not just a positive one: a command whose descriptor
      forbids the system principal must **fail to compile or fail at
      construction** — say which, and prove it. A runtime-only check on a
      type-level claim is not the claim.

      *Added 2026-09-03. Raised by the implementation role in the Stage 1
      submission and answered in review three rounds running, but tracked
      nowhere — so it survived on someone remembering a review document. That is
      the gap this item closes, and it was mine.*
- [ ] Generate sealed `C::Event` sums and exhaustive descriptor matches; event
  variants cannot carry actor, command ID, correlation ID, or timestamp.
- [ ] Add arbitrary-kind, unmapped/duplicate variant, and wrong-context/event
  compile/structural fixtures.
- [ ] Make receipt construction private and post-commit.
- [ ] Add test-only failure points before/within append and at commit.
- [ ] Replace misleading `audit_and` best-effort semantics; make Class-B API
  explicitly must-attempt and observable on append failure.
- [ ] Prove the audit chain is read and written on the caller transaction.
- [ ] Convert login failure and refresh rotation as always-Class-A commands with
  exhaustive typed event variants for every committed outcome.
- [ ] Register initial root-family refresh issuance separately as T09/P and
  bind only its initial-issuance insert call site.
- [ ] Precompute T04 successor outside the transaction; atomically combine
  guarded old-row revoke, successor insert, and rotated event, or family revoke
  and theft event, inside one synchronous transaction.
- [ ] Use a query-only preliminary refresh-row read, re-read authoritative state
  inside T04, and hold the unused raw successor only in a zeroize-on-drop
  wrapper across every reuse/error path.
- [ ] Split client enable/disable and registration authorization issue/revoke
  into their distinct command IDs/tests.

## Wave A — users, credentials, sessions

- [ ] user create / warned create / disable / enable / delete / role change.
- [ ] admin password reset, MFA reset, unlock.
- [ ] self password change and password-reset consumption.
- [ ] MFA enrollment/disable/recovery regeneration mutations.
- [ ] force logout and self/admin session revocation commands.

## Wave B — clients and settings

- [ ] client create/update/scope/URI/disable/enable/delete/secret rotation.
- [ ] server security settings and metrics-token changes.
- [ ] pending sensitive change create/apply/cancel.
- [ ] registration-token create/revoke and baseline atomic dynamic registration.
- [ ] scope-definition create/delete.

## Wave C — keys, tokens, federation identity

- [ ] signing-key rotation/activation and deletion/retirement.
- [ ] master-key database reseal phase with external-file recovery contract.
- [ ] refresh-family and administrative token revocation commands.
- [ ] durable federation link creation/change.
- [ ] federation-provider create/enable/disable/delete.
- [ ] C17 enable consumes fresh sealed exact-version/policy/activation-generation
  preflight, checked-increments generation, and stores evidence atomically;
  disable increments generation and clears evidence; C23 increments version +
  generation; C18 increments generation before delete; no standalone writer.
- [ ] C23 provider trust-policy replacement under the accepted amended design, with
  guarded version/generation increment, forced disable, attempt invalidation, audit
  rollback, and post-commit cache eviction.
- [ ] F01–F06 exact federation attempt/link/user boundaries under the accepted
  amended design, including
  pending-MFA/method-ceremony/session/bookkeeping/event boundaries, five-failure
  state machine, and cross-command guards.
- [ ] verify denial/detection observations remain correctly Class B.

## Per-command conversion checklist

Repeat for every inventory row:

- [ ] authorization and non-racy validation occur before writer acquisition;
- [ ] racy state is rechecked inside the transaction;
- [ ] mutation uses only the private transaction capability;
- [ ] typed event is built from the authoritative mutation result;
- [ ] audit append shares the transaction and failure propagates;
- [ ] handler unwraps `Audited<T>` only after successful commit;
- [ ] legacy/bypass write call sites are removed;
- [ ] every injection point rolls back mutation and audit;
- [ ] successful retry commits one mutation and one event;
- [ ] concurrency loser emits no false success;
- [ ] structural inventory, docs, and tests agree;
- [ ] no audit attribute can contain a secret.
- [ ] command is present in the generated write-site universe and no raw writer
  survives outside the reviewed executor/migration modules.

## Stage 3 — signing-key rotation

> **Known live defect, found 2026-08-28 while building the Stage 1 slice.**
> `authn::session::verify_password_login` records a login failure in **two
> separate non-atomic calls** — bump the counter, then a second best-effort call
> to stamp `locked_until` if the threshold was crossed. A failure between them
> leaves an account whose count crossed the threshold but which is not locked.
> `U22`'s conversion closes this; the wave that owns `U22` must show the lock and
> the counter committing in one transaction, and must not treat the existing
> two-call shape as the behaviour to preserve. Recorded here so it is not
> rediscovered, and because it is a real instance of what RFC 094 exists to fix
> rather than an argument from the RFC.

> **Provisional marking to resolve in this wave (recorded 2026-09-03).** Stage 2
> declares `U01` (admin create user) `system_principal: permitted;`, which is
> almost certainly wrong for the real call site — an admin-create path should
> consume an authenticated admin's authorization decision, making it
> `forbidden`. It was marked `permitted` only because Stage 2 has no
> decision-consuming constructor, so `forbidden` would break an already-approved
> test to buy a property that cannot yet be delivered. **This wave must either
> flip `U01` to `forbidden` or record why an authenticated admin path does not
> need it.** Tracked here rather than left in a `commands.rs` comment, because a
> provisional `permitted` on an admin command is exactly what becomes permanent.

- [ ] Convert `K01` signing-key rotation to the Class-A runner: retire old,
  insert/activate new, and append `signing_key.rotate` in one transaction.
- [ ] Inject failure before append and before commit; assert the prior active
  key is unchanged and no event committed.
- [ ] Assert exactly one active signing key after a successful rotation, backed
  by the unique-active constraint.

**Master-key rotation is not part of RFC 094.** The 2026-07-28 scope amendment
moved `K04`, `K05`, the rotation journal, atomic key-file publication, startup
crash recovery, and old-key custody to
[RFC 100](../../proposed/100-master-key-rotation-recovery.md). Do not implement
them in this milestone; RFC 100 carries its own checklist and crash-injection
matrix.

## Stage 4 — authority switch and closure

- [ ] Make the typed structural gate blocking.
- [ ] Relabel/remove literal parity as authoritative; preserve only useful
  diagnostic output with its limitation.
- [ ] Correct audit hash-chain and atomicity documentation.
- [ ] Run all RFC 093 lanes on one clean commit.
- [ ] Assemble per-command rollback/concurrency evidence and registry diff.
- [ ] Obtain independent adversarial closure review before moving RFC 094 to
  Done.
