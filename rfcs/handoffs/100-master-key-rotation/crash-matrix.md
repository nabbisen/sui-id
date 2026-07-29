# RFC 100 crash-injection matrix

**Governing RFC:** [RFC 100](../../proposed/100-master-key-rotation-recovery.md)

Normative for implementation and closure. Every row must be entered
deterministically — construct the state directly rather than racing a real
rotation — and every row must be observed, not reasoned about.

## Invariants every row must preserve

- **M1** the service never starts against a database with columns sealed under
  different keys;
- **M2** no crash prefix leaves the system without a key that decrypts the
  database;
- **M3** no externally published key artifact exists before the authenticated
  journal entry that binds its exact path;
- **M4** `admin.master_key.database_resealed` asserts only that the database
  phase committed; only `admin.master_key.activated` asserts completion;
- **M5** the old key is retained only by an explicit reviewable disposition;
- **M6** any fingerprint, ownership, permission, symlink, sentinel or state
  mismatch fails closed before service readiness and preserves all files.

## Recovery table

| Observed state | Required recovery |
|---|---|
| No workspace | Normal `OldReady`/`Complete` startup; no preparation prefix exists |
| Workspace exists, no published authenticated `intent.json` | Verify reserved path, directory owner/mode/no-symlink, old active key and DB state, and that entries are only reserved partial intent/update temporaries; delete those regular files, remove and fsync the workspace, return to `OldReady`. Any unexpected entry fails closed |
| `Intent`, partial `next.write` | Authenticate journal with the old active key; delete or rewrite only the bound partial temporary. Resume with an operator-provided matching new key, or clean to `OldReady`. **A lost generated key is never regenerated under the old fingerprint** |
| `Intent`/`NextTempVerified`, `next.write` or `next.ready` present | Re-read fingerprint, encoding, owner and mode; complete no-replace publication and directory fsync and advance, or delete the bound temporary and clean to `OldReady` |
| `NextPublished`, partial `backup.write` | Revalidate `next.ready`; delete or rewrite only the bound partial backup temporary; resume or clean to `OldReady` |
| `NextPublished`/`BackupTempVerified`, backup temp or final present | Verify old fingerprint and sentinel, owner and mode; complete publication and fsync and advance, or clean per the bound disposition |
| `BackupPublished`/`Prepared`, DB and active both old | K04 did not commit. Validate all artifacts, then resume K04 **or** remove `next.ready`, journal and workspace. Backup follows the bound disposition and is **never guessed from its filename** |
| DB new and pending, active old, `next.ready` new | Verify fingerprints and sentinel, atomically activate `next.ready`, fsync, then run K05 |
| DB new and pending, active new | Replacement completed before the crash: run K05 idempotently |
| DB new and complete, active new | Normal startup; remove only matching stale journal metadata |
| Any fingerprint / ownership / permission / symlink / sentinel / state mismatch | **Fail closed** before service readiness; preserve all files for operator recovery |

## Injection points

Inject short writes **and** process termination at each. Both failure modes,
independently.

**Preparation**
1. workspace creation, before and after parent fsync
2. `intent.write` — mid-write, after write before fsync, after fsync before readback
3. `intent.write` → `intent.json` no-replace rename, before and after workspace fsync
4. each journal state advance: update temporary write, fsync, readback, replacement, directory fsync
5. `next.write` — mid-write, after fsync, after readback verification
6. `next.write` → `next.ready` rename, before and after directory fsync
7. `backup.write` and its publication, same points as 5–6

**Database and activation**
8. K04 — before chain-head read; after reseal before append; after hash before insert; after insert before commit; at commit
9. active-file replacement — before, during, after; and before the directory fsync
10. K05 — append and commit

## Required outcomes

- **Every pre-K04 prefix** either idempotently resumes or cleanly returns to
  `OldReady`. A workspace containing only a partial unpublished intent is always
  safely cleanable.
- **Every post-K04 prefix** reaches `Complete`.
- **No prefix** produces mixed ciphertext, a missing active key, or an audit
  record claiming more than committed.
- At every prefix the service either starts against a consistent database or
  refuses before readiness — never starts degraded.

## Additional required cases

| Case | Required outcome |
|---|---|
| Provider precedence supplied the active key | Refuse before `Intent` |
| Paths span filesystems, or the adapter lacks a required primitive | Refuse before creating the workspace |
| Workspace already exists | New rotation refuses; recovery owns it |
| Journal authenticated with the new key after partial reseal | Succeeds — domain-separated tags make this work |
| Retained old-key backup present at startup | Not selected as an active key candidate |
| `review_at` elapsed on a retained backup | Operator finding raised, repeatedly, until renewed or removed |
| K05 run twice | Idempotent; no duplicate event |
