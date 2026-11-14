# RFC 100 — Master-Key Rotation Crash Recovery

**Status.** Accepted
**Accepted on.** 2026-11-14
**Approved by.** `@nabbisen`, 2026-11-14: *"RFC 100 and 146 are accepted."*, after reviewing it himself.
**Security review.** Required
**Design prerequisites.** RFC 094 Accepted in its amended form with master-key
rotation removed to this RFC; the filesystem-atomicity threat delta
independently reviewed.
**Implementation prerequisites.** RFC 094 M2a Implemented — this RFC's `K04`/`K05`
database phases use that Class-A transaction seam; this RFC Accepted; the
crash-injection harness approved. **As of 2026-11-14 the first two are met**:
RFC 094 closed in `v0.88.0`, and its seam carries a reseal command already
(K07; see *Refresh against the code as built*); this RFC was accepted
2026-11-14. **The harness approval is open.**
**Closure prerequisites.** Every transition prefix in the recovery table either
resumes idempotently or returns cleanly to `OldReady`; no prefix yields mixed
ciphertext, a missing active key, or an overstated audit record; independent
adversarial closure review accepts the durable evidence. **Consolidated 2026-10-06 with `ROADMAP.md`'s M2c row**, which required that **no prefix yields mixed ciphertext, a missing active key, or an overstated audit record** — stated there and not here. The ROADMAP row now points at this field.
**Tracks.** ROADMAP M2c — Master-key rotation recovery. Begins after RFC 094 M2a (it consumes that Class-A seam) and runs in parallel with M2b/M3/M4. **Required before M6 entry**, because the M7 workload contract requires a master-key rotation exercise and RFC 099 requires zero blocker-severity defects; rotating a key that is not crash-safe against the immutable soak artifact is not an acceptable exercise.
**Handoff.** [`../handoffs/100-master-key-rotation/README.md`](../handoffs/100-master-key-rotation/README.md)
**Touches.** Master-key loading and rotation CLI, key-file handling, startup
recovery, `master_key_rotation` schema, `sui-id-store` encrypted-column reseal,
audit registry entries `admin.master_key.database_resealed` and
`admin.master_key.activated`, operator documentation.
**Independent design review.** [Design review record, 2026-11-14](../handoffs/100-master-key-rotation/design-review-2026-11-14.md). By `@nabbisen`, who did not author this RFC or its refresh.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** Independent architecture reviewer acting as
requirements architect, 2026-07-28.
**Implementation owner.** To be named after acceptance.

> **Note on independence.** I authored this RFC in my requirements-architect
> role. Under the role separation recorded for this project, I should not also
> be its independent security reviewer. This RFC needs a reviewer who did not
> shape it. Flagging so the assignment is deliberate rather than inherited.

## Summary

Make offline master-key rotation crash-safe. Rotation spans a database
transaction and an atomic filesystem replacement, so it cannot be made atomic by
the database alone. This RFC defines a journaled state machine, two Class-A
database commands around the file activation, and a startup recovery procedure
that resolves every crash prefix without producing mixed ciphertext, a missing
active key, or an audit record claiming more than actually committed.

## Provenance

This content was carried out of RFC 094 substantially unchanged during the
2026-07-28 amendment. No design decision is revisited here; only the review
boundary moved. The rationale for the split: every other part of RFC 094 answers
"does a mutation and its audit record commit or roll back together?" This
answers "can offline key rotation survive a crash at any point?" — a filesystem
atomicity and recovery problem with its own threat model. Reviewed as an
appendix to audit atomicity it would receive the wrong kind of scrutiny.

## Refresh against the code as built (2026-11-14)

This RFC was written on 2026-07-28, before RFC 094's seam existed. Since then:
- RFC 094 closed (`v0.88.0`);
- a reseal command was sealed on that seam (K07, RFC 094 cycle J stage 4);
- the rotation command's ordering was fixed urgently (`v0.87.1`,
  `rfcs/handoffs/094-transactional-audit/urgent-master-key-ordering-2026-11-03.md`).

The architect re-measured the rotation path on `6774d29`. **Nothing below
changes the design's invariants (M1–M6).** It records where the code stands
against them, and four changes to the design that the measurements imply.

### What exists today

| Piece | As built | This RFC's design |
|---|---|---|
| Database phase | **K07**, `commands::reseal_master_key` (`sui-id-store/src/commands.rs:6755`). One Class-A transaction reseals six tables and appends `admin.master_key.resealed` (`:6615`). | `K04`: the same reseal, plus the sentinel and the pending `master_key_rotation` row, appending `admin.master_key.database_resealed` |
| Completion record | `admin.master_key.rotated`, a **best-effort** append after both renames (`sui-id/src/cli.rs:671`), outside any transaction | `K05`: Class-A, appending `admin.master_key.activated` |
| Staging the new key | `<key_file>.new`, `create_new` with mode `0600` (`runtime/keyring.rs:73`, called at `cli.rs:621`). A rotation refuses if it already exists (`cli.rs:578`). **No `sync_all` on the file and no fsync of its directory**: `write_key_file` returns after `write_all` (`keyring.rs:90`) | journal-bound temporaries: fsync, read back, verify, publish without replacement, fsync the directory |
| Activation | `rename(key_file → .bak.<stamp>)` (`cli.rs:648`), **then** `rename(.new → key_file)` (`cli.rs:655`). **The active path is removed first**, and no directory fsync follows either rename | replace the active path atomically, without first removing it, then fsync the directory |
| Where the key comes from | `SUI_ID_MASTER_KEY` beats the key file (`keyring.rs:31-32`). Rotation reads the old key through the same `resolve` (`cli.rs:592`) and **never checks its origin** | refuse when the environment or a provider supplied the active key |
| A missing key file | `resolve` **generates a fresh key and writes it** (`keyring.rs:59-60`). Startup (`runtime/startup.rs:168`) and ten CLI commands call it. **Measured 2026-11-14**: a start with the key file removed logs one WARN, writes a new key, and serves (`/.well-known/openid-configuration`: 200) | M6: fail closed before readiness |
| Detecting the wrong key | **None at startup.** Nothing in `startup.rs` decrypts. The active signing key is first unsealed at token issuance (`sui-id-core/src/oidc/authorize.rs:1019`). Traced, **not run** against a populated database: the measured instance above had no sealed rows yet | a sentinel, which recovery authenticates candidates against |
| Journal, `master_key_rotation`, startup recovery | none | as specified below |

**The crash prefixes this leaves open today**, in the order rotation runs:
1. **After K07 commits, before the staged key reaches the disk.** The
   database's commit is durable; the key may still be only in the page
   cache. A power loss here **makes every sealed column unreadable**, short of
   restoring the pre-rotation backup the command tells the operator to take
   (which opens with the old key). The `v0.87.1` fix
   closed the disk-full case, where the write fails before the reseal, but
   not this one. Its design said *"the key is on disk"* without requiring an
   fsync. That was the architect's error.
2. **Between the two renames.** No file is at `key_file`. The real key is
   at `.new`, but the next start or CLI command takes `resolve`'s
   generate branch and writes a fresh, wrong key there.
3. **After both renames, before a directory fsync.** Either rename may not
   have persisted. After a power loss the directory can come back in an
   earlier state: prefix 2's, or the old key at `key_file` over a database
   already resealed under the new one. That is a wrong-key start, which
   nothing detects today (the table's *Detecting the wrong key* row).

None of these is attacker-reachable. Rotation is an offline command the
operator runs, which is why this RFC exists rather than a security advisory.
The motivation's *"not crash-safe"* stands. These prefixes make it specific.

### What this changes in the design

1. **K07 is the database phase. `K04` is not built.** Extend K07's
   transaction with the sentinel reseal and the pending `master_key_rotation`
   row. Keep its event name, `admin.master_key.resealed`: it already asserts
   only that the database phase committed, which is M4's boundary, and
   renaming a shipped event costs every audit reader for no gain. `K04`'s
   inventory id stays retired. **`K05` is built as specified, under the
   shipped name too**: `admin.master_key.rotated` becomes a Class-A append
   in K05's transaction, instead of the best-effort append it is today. It
   already fires at the same point, after activation. The operator docs
   already teach the `resealed`/`rotated` pair (`docs/src/reference/audit-events.md`,
   and `docs/src/guides/operators.md`, *Diagnosing a rotation from the audit
   log*). So **M4's two events are `admin.master_key.resealed` and
   `admin.master_key.rotated`**, wherever this RFC says
   `database_resealed` and `activated`.
2. **A startup key check, for every start, not only recovery.** The sentinel
   is checked before readiness on every start. A wrong key, whether from
   rotation, a restore from a backup whose key was lost, or a mistyped
   environment variable, refuses then, instead of starting and failing at
   the first sign-in. This is M6 applied to startup in general. It is in this
   RFC's scope because recovery cannot work without it.
3. **`resolve` generates a key only for a database that has never been
   sealed.** With a sentinel present, a missing key file fails closed, at
   startup and in every CLI command.
4. **Two fixes need not wait for this RFC.** Each is small, closes a prefix
   above, and is required by the design anyway:
   - `write_key_file` calls `sync_all` and fsyncs the parent directory;
     rotation fsyncs the directory after each rename. That closes prefixes 1
     and 3. It also makes `cli.rs`'s *"the only durable copy"* (`:484`, `:616`)
     true.
   - Rotation refuses when the key's origin is `Env`, before opening the
     database.

   Prefix 2 needs change 3 (or the journal) and stays open until this RFC
   is built. **When the two fixes land is still open** (*Still open*
   below).

## Motivation

Master-key rotation re-encrypts every encrypted column under a new key and then
replaces the active key file. A crash between those steps can leave the database
readable only by a key that is no longer active, or an active key that cannot
decrypt the database. Either outcome is an outage that ordinary backups do not
resolve, because the backup and the key have diverged.

The current implementation has no journal, no recovery procedure, and no
crash-injection tests. Until this RFC lands, rotation must be documented as a
manual procedure that is **not crash-safe**.

## Non-goals

- No external key-management service, HSM, or KMS integration.
- No online/zero-downtime rotation. Rotation remains offline.
- No secure-erasure claim for flash or copy-on-write storage.
- No change to the encryption algorithm, key derivation, or column set.
- No rotation support when a key provider other than the configured file is
  authoritative — that path refuses before starting.
- No per-tenant keys. This RFC assumes **one master key per deployment**,
  which is what sui-id has (single-realm, [RFC
  022](../done/022-single-realm-scope-statement.md)). If multi-tenancy is
  ever adopted, whether each tenant gets its own master key is a question
  for that work — it is recorded in [RFC
  025](./025-multi-tenant-expansion.md)'s open question 7, and it is **not**
  a requirement on this RFC. Stated here only so a later reader knows the
  single-key assumption was made knowingly rather than overlooked.
  Accepted by `@nabbisen` on 2026-10-14, in his word: *"Both accepted."* —
  as this RFC's own assumption, and not as a direction on tenancy.

## Security invariants

- **M1 — no mixed ciphertext.** The service never starts against a database
  containing columns sealed under different keys.
- **M2 — no lost active key.** No crash prefix leaves the system without a key
  that decrypts the database.
- **M3 — journal-before-artifact.** No externally published key artifact exists
  before the authenticated journal entry that binds its exact path.
- **M4 — truthful audit.** `admin.master_key.database_resealed` asserts only
  that the database phase committed; only `admin.master_key.activated` asserts
  completion.
- **M5 — bounded old-key custody.** The old key is retained only by an explicit,
  reviewable disposition decision, never indefinitely by default.
- **M6 — fail closed.** Any fingerprint, ownership, permission, symlink,
  sentinel, or state mismatch stops before service readiness and preserves all
  files for operator recovery.

## State machine

```text
OldReady
  ──exclusive workspace + atomically published Intent/artifacts──> Prepared
  ──K04 reseal DB + pending row + audit, one tx──> DbCommitted
  ──atomic replace active file + directory fsync──> FileActivated
  ──K05 mark complete + audit, one tx──> Complete
```

Add a singleton `master_key_rotation` row carrying `rotation_id`, old and new
non-secret key fingerprints, state `db_committed_pending_activation | complete`,
prepared/committed/completed timestamps, backup filename, and the recorded
backup disposition.

### Preparation and the journal

Preparation atomically creates the reserved directory `<active>.rotation-work`
with mode 0700 and the current owner, following no symlink, and fsyncs the
parent. Its existence is the rotation lock; if it already exists, a new rotation
refuses and recovery owns it.

Inside that workspace the command writes `intent.write` with checked write-all
semantics, fsyncs, reads it back, verifies both authentication tags, then uses
atomic no-replace publication to rename it to `intent.json` and fsyncs the
workspace directory. A crash or short write before publication can leave only a
non-authoritative temporary inside the reserved workspace; it cannot produce a
partial published journal.

The published journal contains **no key material**. It binds canonical
active/workspace/next/backup paths, every reserved temporary path, rotation ID,
old and new fingerprints, expected owner and mode, requested backup disposition,
and monotonic preparation state:

```text
Intent -> NextTempVerified -> NextPublished
       -> BackupTempVerified -> BackupPublished -> Prepared
```

It carries domain-separated authentication tags derived independently from the
old and the new key, so recovery can authenticate it with whichever candidate
decrypts the database sentinel. Every state advance uses a separate reserved
update temporary, checked write-all, fsync, readback and tag verification,
atomic replacement, and workspace-directory fsync — so a crash mid-update leaves
either the prior authenticated state or the new one.

New-key and backup publication follow the same protocol: write to the bound
temporary with create-new and 0600/current-owner and no-symlink semantics, fsync,
read back, verify fingerprint and encoding, record the `*TempVerified` state,
atomically publish without replacement, fsync the parent, then record the
`*Published` state.

### Platform and provider preconditions

Rotation refuses before creating the workspace unless the active, workspace,
next, journal-temporary, and backup paths share a filesystem whose supported
platform adapter provides durable create, atomic replacement, atomic no-replace
publication, and file and directory fsync. It also refuses if environment or
key-provider precedence supplied the active key, rather than writing a file that
startup would ignore.

### K04, activation, K05

`K04` opens with the old key, reseals every encrypted column and the sentinel
under the new key, writes `db_committed_pending_activation`, and appends
`admin.master_key.database_resealed` — all in one Class-A transaction on the RFC
094 seam. That event states only that the database phase committed and that file
activation is pending.

Activation atomically replaces the active key path with the already-fsynced
`next.ready`, without first removing the active path, then fsyncs the directory.
On platforms where secure atomic replacement with the required ownership and
mode cannot be guaranteed, rotation refuses to start.

`K05` reopens with the active new key, marks the row `complete`, and appends
`admin.master_key.activated` in one transaction. CLI success prints only after
K05 commits.

## Startup recovery

Recovery runs before normal database open.

| Observed state | Recovery |
|---|---|
| No workspace | Normal `OldReady`/`Complete` startup; no preparation prefix exists. |
| Workspace exists, no published authenticated `intent.json` | Verify reserved path, directory owner/mode/no-symlink, old active key and DB state, and that entries are only reserved partial intent/update temporaries; delete those regular files, remove and fsync the workspace, return to `OldReady`. Any unexpected entry fails closed. |
| `Intent`, partial `next.write` | Authenticate the journal with the old active key; delete or rewrite only the bound partial temporary. Resume with an operator-provided matching new key, or clean to `OldReady`. A lost generated key is never regenerated under the old fingerprint. |
| `Intent`/`NextTempVerified`, `next.write` or `next.ready` present | Re-read fingerprint, encoding, owner and mode; complete no-replace publication and directory fsync and advance, or delete the bound temporary and clean to `OldReady`. |
| `NextPublished`, partial `backup.write` | Revalidate `next.ready`; delete or rewrite only the bound partial backup temporary, then resume or clean to `OldReady`. |
| `NextPublished`/`BackupTempVerified`, backup temp or final present | Verify old fingerprint and sentinel, owner and mode; complete publication and fsync and advance, or clean according to the bound disposition. |
| `BackupPublished`/`Prepared`, DB and active both old | K04 did not commit: validate all artifacts, then resume K04 or remove `next.ready`, journal and workspace. Backup follows the bound disposition and is never guessed from its filename. |
| DB new and pending, active old, `next.ready` new | Verify fingerprints and sentinel, atomically activate `next.ready`, fsync, then run K05. |
| DB new and pending, active new | Replacement completed before the crash: run K05 idempotently. |
| DB new and complete, active new | Normal startup; remove only matching stale journal metadata. |
| Any fingerprint, ownership, permission, symlink, sentinel or state mismatch | Fail closed before service readiness; preserve all files for operator recovery. |

## Old-key custody

Before `Intent`, the operator selects and the journal binds one disposition:
`RetainUntil { review_at, custody_reference }` or
`RemoveAfterVerified { verification_reference }`. K05 cannot mark `Complete`
without recording it in `master_key_rotation`.

A retained backup is never an active startup candidate and produces an operator
finding after `review_at` until renewed or removed. Removal unlinks the file only
after the referenced pre-rotation backup is retired or re-encrypted; this design
makes no secure-erasure claim for flash or copy-on-write storage. A removed
backup records timestamp and operator identity.

## Test plan

Inject short writes and process termination at: workspace creation; every
initial and update temporary write; file fsync; readback verification;
no-replace and replace publication; directory fsync; each journal transition;
K04 mutation, append and commit; active replacement; and K05 append and commit.

Every pre-K04 prefix — including a workspace with partial unpublished intent —
must idempotently resume or cleanly return to `OldReady`. Every post-K04 prefix
must reach `Complete`. Assert at every prefix that the service either starts
against a consistent database or refuses before readiness, and that no audit
record claims more than committed.

Add provider-precedence and cross-filesystem refusal tests, and a test proving a
retained backup is not selected as a startup key candidate.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Operator loses the generated new key mid-preparation | Recovery never regenerates under the old fingerprint; it requires the matching key or a clean return to `OldReady`. |
| Filesystem does not honour atomic replacement | Refuse before creating the workspace. |
| Journal authenticated with the wrong key after partial reseal | Domain-separated tags derived from both keys; recovery authenticates with whichever decrypts the sentinel. |
| Audit implies completion that did not occur | Two distinct events; M4 makes the boundary explicit. |
| Old key retained indefinitely | M5 requires an explicit bound disposition before `Intent`. |

## Acceptance criteria

- Every recovery-table row has an executed crash-injection test.
- No prefix produces mixed ciphertext, a missing active key, or an overstated
  audit record.
- `K04`/`K05` use the RFC 094 Class-A seam with observed rollback evidence.
- Operator documentation describes rotation, recovery, and custody accurately.
- Independent closure review — by a reviewer who did not author this RFC —
  accepts the adversarial evidence.

## Open questions

1. Should rotation support a dry-run that exercises preparation and rollback
   without K04? Useful operationally; adds a code path that must itself be
   crash-safe.
2. Should a retained old-key backup expire automatically at `review_at`, or only
   produce a finding? This RFC currently specifies a finding only.

### Proposed answers (architect, 2026-11-14)

These are proposals, not decisions.

1. **No dry-run of preparation. A read-only `--check` instead.** It runs every
   refusal this RFC defines before the workspace exists, and creates
   nothing:
   - the platform and same-filesystem preconditions;
   - the key's origin;
   - that the active key opens the sentinel;
   - that no workspace or `.new` is left over.

   A dry-run that prepares and rolls back is a second crash-safe path to
   build and test, and it is exercised less than the real one. `--check`
   gives the operator what they want, *will this refuse, before I take
   the downtime?*, with no new state to recover from.
2. **A finding only, never automatic removal.** Removing a retained backup
   is irreversible, and it may be the only key that opens a database backup
   taken before the rotation. A finding can be acted on, renewed or ignored,
   and costs nothing if it is wrong. M5 already bounds custody: no disposition,
   no `Complete`.

### Still open, and not the architect's

- **The independent design review** this RFC requires, which also covers its
  design prerequisite, the filesystem-atomicity threat delta. The architect
  refreshed this RFC and cannot also review it. **The reviewer is named**:
  `@nabbisen`, 2026-11-14: *"Understood. I will review it."*
- **When the two fixes in *What this changes*, item 4, land: settled.** The
  first stage of `0.91.0`, ahead of this RFC and independent of its
  acceptance; not `0.90.0`, so the tagged tree stays the one the live run
  tests. `@nabbisen`, 2026-11-14: *"You recommendation is accepted."*
  Dispatched: [`../handoffs/100-master-key-rotation/0.91.0-stage-1-two-rotation-fixes-2026-11-14.md`](../handoffs/100-master-key-rotation/0.91.0-stage-1-two-rotation-fixes-2026-11-14.md).
- **The crash-injection harness**, whose approval is an implementation
  prerequisite.
