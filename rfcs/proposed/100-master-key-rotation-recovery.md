# RFC 100 — Master-Key Rotation Crash Recovery

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFC 094 Accepted in its amended form with master-key
rotation removed to this RFC; the filesystem-atomicity threat delta
independently reviewed.
**Implementation prerequisites.** RFC 094 M2a Implemented — this RFC's `K04`/`K05`
database phases use that Class-A transaction seam; this RFC Accepted; the
crash-injection harness approved.
**Closure prerequisites.** Every transition prefix in the recovery table either
resumes idempotently or returns cleanly to `OldReady`; no prefix yields mixed
ciphertext, a missing active key, or an overstated audit record; independent
adversarial closure review accepts the durable evidence.
**Tracks.** ROADMAP M2c — Master-key rotation recovery. Begins after RFC 094 M2a (it consumes that Class-A seam) and runs in parallel with M2b/M3/M4. **Required before M6 entry**, because the M7 workload contract requires a master-key rotation exercise and RFC 099 requires zero blocker-severity defects; rotating a key that is not crash-safe against the immutable soak artifact is not an acceptable exercise.
**Handoff.** [`../handoffs/100-master-key-rotation/README.md`](../handoffs/100-master-key-rotation/README.md)
**Touches.** Master-key loading and rotation CLI, key-file handling, startup
recovery, `master_key_rotation` schema, `sui-id-store` encrypted-column reseal,
audit registry entries `admin.master_key.database_resealed` and
`admin.master_key.activated`, operator documentation.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** Independent architecture reviewer acting as
requirements architect, 2026-07-28.
**Implementation owner.** To be named after acceptance.
**Independent security and closure reviewer.** Not the requirements-architect
role, which authored this RFC — see the note below. Role independence per RFC
018; vendor is not a criterion. Routes to the implementation role for
implementability, with the crash-recovery design judgments named in the review
request and ruled on by `@nabbisen` where no other role can adjudicate them.

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
