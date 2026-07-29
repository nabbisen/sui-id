# RFC 100 implementation stages

**Governing RFC:** [RFC 100](../../proposed/100-master-key-rotation-recovery.md)

Build in this order. Each stage is independently reviewable, and **recovery
lands before the thing it recovers** — never build a write path whose crash
prefixes have no resolution yet.

## Stage 1 — schema and platform adapter

**Surfaces:** new migration; a platform adapter module.

Add the singleton `master_key_rotation` row: `rotation_id`, old and new
non-secret key fingerprints, `state` (`db_committed_pending_activation` |
`complete`), prepared/committed/completed timestamps, backup filename, and the
recorded old-key custody disposition.

Add a platform adapter exposing exactly: durable create, atomic replacement,
atomic **no-replace** publication, and file and directory fsync. Rotation
refuses before creating any workspace unless the active, workspace, next,
journal-temporary, and backup paths all share a filesystem whose adapter
provides all four.

Rotation also refuses if environment or key-provider precedence supplied the
active key rather than the configured file — never write a file that startup
would ignore.

**Tests:** migration up/down; adapter capability probe on the supported
platform; both refusal paths asserted before any file is created.

## Stage 2 — the journal

**Surfaces:** journal reader/writer; the reserved workspace.

Create `<active>.rotation-work` atomically with mode 0700 and the current owner,
following no symlink, then fsync the parent. **Its existence is the rotation
lock** — if it already exists, a new rotation refuses and recovery owns it.

Write `intent.write` with checked write-all, fsync, read back, verify both
authentication tags, then atomically publish no-replace to `intent.json` and
fsync the workspace directory.

The journal contains **no key material**. It binds canonical active, workspace,
next and backup paths, every reserved temporary path, rotation ID, old and new
fingerprints, expected owner and mode, requested backup disposition, and the
monotonic preparation state:

```
Intent -> NextTempVerified -> NextPublished
       -> BackupTempVerified -> BackupPublished -> Prepared
```

Authentication tags are domain-separated and derived independently from the old
and the new key, so recovery can authenticate the journal with whichever
candidate decrypts the database sentinel.

Every state advance uses a separate reserved update temporary, checked
write-all, fsync, readback and tag verification, atomic replacement, and
workspace-directory fsync. A crash mid-update therefore leaves either the prior
authenticated state or the new one — never a torn state.

**Tests:** lock behaviour when the workspace already exists; tag verification
with each key; a partial `intent.write` is non-authoritative and safely
deletable; every state advance is atomic.

## Stage 3 — startup recovery

**Build this before Stage 4.** Every write path added later must already have a
recovery answer.

Recovery runs before normal database open and implements the full table in
[`crash-matrix.md`](./crash-matrix.md). The general rules:

- authenticate the journal with whichever key decrypts the database sentinel;
- act only on paths the journal binds — never infer from filenames, timestamps
  or shape;
- delete only bound partial temporaries;
- a lost generated key is **never** regenerated under the old fingerprint;
- any fingerprint, ownership, permission, symlink, sentinel or state mismatch
  fails closed before service readiness and preserves all files.

**Tests:** every row of the crash matrix, entered by direct state construction
rather than by racing a real rotation.

## Stage 4 — preparation

Write the new key to the journal-bound `next.write` with create-new,
0600/current-owner, no-symlink and checked write-all; fsync; read back; verify
fingerprint and encoding; record `NextTempVerified`; atomically rename without
replacement to `next.ready`; fsync the workspace; record `NextPublished`.

The old-key backup follows the identical protocol to `BackupPublished`, then
`Prepared`.

Before `Intent` the operator selects, and the journal binds, one disposition:
`RetainUntil { review_at, custody_reference }` or
`RemoveAfterVerified { verification_reference }`.

**Tests:** short write at every point; crash between rename and directory fsync;
fingerprint mismatch on readback; disposition required before `Intent`.

## Stage 5 — K04, activation, K05

`K04` opens with the old key, reseals every encrypted column and the sentinel
under the new key, writes `db_committed_pending_activation`, and appends
`admin.master_key.database_resealed` — **one Class-A transaction on the RFC 094
seam**. That event asserts only that the database phase committed and that file
activation is pending. It must not imply completion.

Activation atomically replaces the active key path with the already-fsynced
`next.ready` **without first removing the active path**, then fsyncs the
directory.

`K05` reopens with the active new key, marks the row `complete`, and appends
`admin.master_key.activated` in one transaction. **CLI success prints only after
K05 commits.**

**Tests:** injection at K04 mutation, append and commit; injection around the
replacement; K05 idempotent re-run; assert no path prints success before K05.

## Stage 6 — custody and operator surface

`K05` cannot mark `Complete` without recording the disposition. A retained
backup is never an active startup candidate and produces an operator finding
after `review_at` until renewed or removed. Removal unlinks only after the
referenced pre-rotation backup is retired or re-encrypted, and records timestamp
and operator identity.

Update operator documentation: **remove the "not yet crash-safe" warning only in
the change that closes M2c**, and replace it with the recovery procedure.

**Tests:** retained backup is not selected as a startup key candidate; finding
fires after `review_at`; removal records identity.
