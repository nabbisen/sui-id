# RFC 142 stage 5 — the backup the guide asks for, taken before the migration runs

> ## Whose turn it is: **the implementation role's.**
> Cycle J stage 9, the last of the cycle —
> [`../094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md`](../094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md)
> §D. Scoped and authorized 2026-11-08.

**Dispatched.** 2026-11-08, by the architect.
**Governing RFC.** [RFC 142](../../accepted/142-upgrading-is-the-least-tested-thing-we-do.md)
**D4**, **Accepted**, in its **settled** shape. Building is permitted.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 1 — what was settled, verbatim

> **D4.** Shape **(a)**, with its three refinements: act only when a
> migration will actually run; `VACUUM INTO` rather than a file copy;
> free-space check before writing. A plain snapshot beside the database, not
> the encrypted `backup.rs` path, so a restore needs `sqlite3` and nothing
> else.

And shape (a) itself: *"Take one automatically before the first migration of
a run, to a path derived from the database path, with `--no-backup` to
decline."* The RFC's own argument for (a): **the operator who most needs
the backup is the one who did not read the guide.**

## 2 — where migrations run, measured, and therefore where the backup goes

**Every `Database::open` runs pending migrations**
(`crates/sui-id-store/src/db.rs:40-46`), not only the server's start.
Measured callers of the runtime wrapper `sui_id::database::open`:
- `runtime/startup.rs:163`, the server;
- **ten** call sites in `cli.rs` (`:297`, `:335`, `:384`, `:565`, `:743`,
  `:1070`, `:1122`, `:1257`, `:1283`, `:1523`): every `admin` subcommand, and
  `setup`.

So an operator who runs `sui-id admin unlock-user` with a new binary against
an old database **migrates it**, and needs the backup exactly as much as the
server does.

**The decision goes in the runtime wrapper, and the compiler enforces it.**
Change `sui_id::database::open(path, key)` to take the policy as a parameter,
for example `open(path, key, PreMigrationBackup::Take |
PreMigrationBackup::Declined)`. **Every one of the eleven callers must then
choose**, and a new caller cannot silently skip it. Do not make it a default
argument, a global, or an environment variable.

**What stays out:**
- **`sui_id_store::Database::open` is unchanged.** The store's tests and
  in-memory databases go through it, and none of them has anything to back
  up.
- **`runtime/dev_mode.rs:372`** calls `Database::open` directly for the
  pinned dev database. Leave it, and name it in the return as the one
  deliberate bypass.
- **`sui-id backup` is unaffected**: it snapshots through a raw connection
  and `VACUUM INTO` (`crates/sui-id-store/src/backup/ops.rs:108-113`), not
  `Database::open`, so a backup taken with a new binary does not migrate.
  Measured; no change needed.

## 3 — the sequence inside `database::open`

1. **The existing refusal first.** A database this build refuses gets no
   backup and the same line as today. `check_database_file`'s behaviour is
   unchanged.
2. **Decide with stage 4's `migrations::preview`.** Only
   `Preview::Pending` takes a backup. `NoFile` and `Fresh` have nothing to
   lose, so they get no backup. `AtHead` runs no migration, so it gets none
   either: *"act only when a migration will actually run"*.
3. **Free space** (§5). Refuse if there is not enough.
4. **The snapshot** (§4). Refuse if it fails, and remove the partial file.
5. **Only then** `Database::open`, which migrates.
6. One line to stderr and one `tracing` event naming the file (§6).

**A race, and why it is acceptable:** another process may migrate between
step 2 and step 5. Then the snapshot is of the pre-migration state, as
intended, or of an already-migrated one, which is an unneeded but harmless
extra file. Neither loses data. Say so in one comment.

## 4 — the snapshot, with both premises measured before dispatch

The architect measured both (Python `sqlite3`, scratchpad, not the tree):

- **`VACUUM INTO` works from a read-only source connection**, and it
  **includes committed WAL frames**. With a live writer holding a row only in
  the WAL, the snapshot had it (1001 rows, not 1000). **So the source is
  opened read-only**, never for writing. Use stage 4's
  `open_preview_connection`, which already avoids leaving `-shm`/`-wal`
  behind and already percent-encodes the path.
- **`VACUUM INTO` accepts a pre-created empty file**, and the file keeps its
  mode. Created with `O_CREAT | O_EXCL` and mode `0600`, the snapshot was
  `0600`. **So it is never world-readable, not even for an instant.** Create
  it that way (`std::fs::OpenOptions` with `create_new(true)` and
  `std::os::unix::fs::OpenOptionsExt::mode(0o600)`), then `VACUUM INTO` it.
  `create_new` also means **an existing file is never overwritten**: refuse
  instead. The code still compiles off Unix (`main.rs:403` keeps a
  `cfg(not(unix))` branch), and `mode` is Unix-only. Put it under
  `#[cfg(unix)]`, keep `create_new` everywhere, and say in the comment what
  a non-Unix build gets.

**Re-measure both against the bundled SQLite 3.53.2 through `rusqlite`**,
first, and report it. If either differs, stop and report before building.

**Where:** a new function in `crates/sui-id-store/src/backup/ops.rs`, beside
the existing snapshot, which is where raw database file access belongs (RFC
094's `backup-into-store` handoff). It is **not** `O04`, whose rationale is
*"operator-invoked"*. Add the next free command id (`O07`; `O01`–`O06` are
taken, measured) to `contracts/write-commands.toml`, with its own rationale
(*"automatic pre-migration snapshot; no domain mutation in the live
database"*) and test id. G23 must stay green.

**The name, derived from the database path and saying what it is:**
`<db file name>.pre-upgrade-schema-<from>-to-<to>-<YYYYMMDDTHHMMSSZ>.sqlite`,
in the database's own directory. For example,
`sui-id.sqlite.pre-upgrade-schema-44-to-49-20261108T091500Z.sqlite`. An
operator with three of them can see which one to restore without opening
any.

**Not encrypted, and that is not a new exposure.** The snapshot holds what
the database file already holds: the encrypted columns stay sealed under the
master key, which is not in it. It has the same directory and an owner-only
mode. Say this in one sentence in the guide, so nobody assumes otherwise
either way.

**Never deleted automatically.** Deleting an operator's backup is worse than
leaving one too many. The guide says when it is safe to delete.

## 5 — free space

**The workspace forbids `unsafe`** (`Cargo.toml:200`, and every crate's
`#![forbid(unsafe_code)]`), so `libc::statvfs` is out. **`rustix` 1.1.5 is
already in `Cargo.lock`**, and `rustix::fs::statvfs` is a safe API. Adding it
as a direct dependency of `sui-id-store` with only the features it needs is
acceptable. **Say exactly what `Cargo.toml` and `Cargo.lock` gain**, and
confirm that no new crate enters the lockfile. If a better already-locked
option exists, use it and say why.

**The rule:**
- `needed` = the database file's size plus its `-wal`'s, if any.
- **Refuse unless the filesystem holding the database has at least `2 ×
  needed` available**: one for the snapshot, and one of headroom for the
  migrations themselves, since a table rebuild can briefly double a table.
- **If free space cannot be measured** on this platform, take the snapshot
  anyway, and log that it was not checked. A `VACUUM INTO` that runs out of
  space fails cleanly, and step 4 then removes the partial file and refuses.

**The refusal**, one line on stderr, first and alone, in the style of
`database.rs`'s existing lines:

```
sui-id: refusing to migrate: the pre-upgrade backup of <db> needs about <N> MB free beside it and <M> MB are available. Nothing was changed. Free some space, or start with --no-backup if you have taken a backup yourself.
```

**Exit code 1, not 65.** `database.rs`'s module doc says the convention is
*"deliberately two lines long"*, with 65 meaning *the database is not one
this build can use*. This one is a disk, not the database. **A restart loop
under systemd is harmless here**, because a refused check writes nothing.
Say that in the code comment.

A snapshot that fails for any other reason gets the same shape: `refusing to
migrate: could not write the pre-upgrade backup <path>: <cause>. Nothing was
changed. …`, with the partial file removed.

## 6 — what the operator reads when it works

```
sui-id: backed up the database to <path> before migrating schema 44 → 49. To undo the upgrade, stop sui-id and see "Rollback" in docs/src/guides/upgrade.md.
```

Plus the same as one `tracing` event (`pre_migration_backup = "<path>"`,
`from`, `to`), so `journalctl` finds it.

**`--no-backup`:** one line, *"sui-id: --no-backup: migrating schema 44 → 49
without a backup."*, so the decision is on the record in the log, not only in
a unit file.

**`preview-upgrade` must say it too.** In the `Pending` case, before its
`Nothing was changed.` line: *"Before migrating, this sui-id writes a backup
beside the database (about N MB; N MB available) unless started with
--no-backup."* Use the same free-space function, so the preview predicts
exactly what the start will decide, including *"not enough space: this
sui-id would refuse to migrate"*.

## 7 — `--no-backup`

A boolean flag, accepted by the server's start and by every subcommand that
reaches `database::open`. It is found the way boolean flags already are
(`args.iter().any(...)`), so **it must not be added to
`VALUE_TAKING_FLAGS`**. Document it in `print_help` once, as applying to
anything that may migrate, not per subcommand.

## 8 — tests

Black-box, real files, temp directories:

1. **It happens.** A database at schema 44, populated with rows: after
   `database::open(…, Take)`, exactly one snapshot exists with the specified
   name. It is a valid SQLite file **at schema 44** whose row counts equal
   the pre-migration counts. **Its mode is `0600`.** The live database is
   at head.
2. **It is skipped when nothing is pending**: at head, and also for
   `NoFile` and `Fresh`. No snapshot file, and the directory's file set is
   unchanged apart from what `Database::open` itself creates.
3. **`--no-backup`**: no snapshot, the migration runs, and the line is
   printed. Through the real binary, once.
4. **Free space short** (inject the measurement through a seam; do not fill
   a disk): the refusal line, exit 1, **no snapshot, and the database still
   at 44, byte-identical.**
5. **The snapshot fails** (inject the failure through a seam, or point it at
   a directory it cannot write): the refusal, the partial file removed, the
   database untouched.
6. **An existing file of the same name**: refused, and **the existing file
   is byte-identical afterwards.**
7. **A WAL-only row is in the snapshot**: a live writer holds one, and the
   snapshot has it.
8. **`preview-upgrade` predicts the start**: for both the enough-space and
   not-enough-space cases, the preview's backup line matches what
   `database::open` then does.
9. **Every caller chose**: this is compile-time by design (§2). Show it in
   the return by listing the eleven call sites and the policy each passes.

**Mutations: run them and report them.** Each must turn a named test red:
- backing up on `AtHead`;
- not backing up on `Pending`;
- creating the file without `0o600`;
- `create(true)` in place of `create_new(true)`;
- a free-space rule of `1 × needed`;
- not removing the partial file.

## 9 — documentation

- **`docs/src/guides/upgrade.md`**:
  - **General procedure, step 1** keeps `sui-id backup`, which carries the
    master key and is the one to keep. Add one sentence: starting the new
    binary also takes a database snapshot automatically when it will
    migrate.
  - **Rollback**: it opens *"Rollback is not supported."* and then lists
    rollback steps. Fix that contradiction. Then add the snapshot route:
    stop sui-id, replace the database file with the snapshot (and remove any
    `-wal`/`-shm` beside it), and start the **previous** binary.
  - What the snapshot is, that it is unencrypted for the reason in §4, and
    that it is safe to delete once the upgrade is verified.
- **`docs/src/reference/configuration.md`**: `--no-backup` in the
  subcommands section.
- **`docs/src/reference/audit-events.md`**: nothing. The snapshot is not an
  audit event, because no database row changes. Say so in the return, so the
  absence is visibly a decision.
- **`CHANGELOG.md`** `[Unreleased]` → `### Added`, in an operator's words,
  including the exit-1 refusal and `--no-backup`.
- **RFC 142's closure.** This is its last stage. **Do not mark RFC 142
  Implemented or move it**: closure is the owner's, after the architect's
  closure review. List in the return each closure prerequisite and the test
  or document that meets it.

## 10 — constraints

- **No migration**, no schema change, nothing in G22's corpus.
- **`check_database_file`, `Database::open` in the store, and dev mode are
  unchanged** (§2).
- **Gate evidence**: `bash scripts/local-matrix.sh --changed`, quoting its
  scope line, on the real tree. It will be the complete matrix. A pre-check
  clone, if you use one, goes beside the tree on the main disk.
- No tag, no `workflow_dispatch`, no Level B claim.

## 11 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage9-rfc142-stage5-pre-migration-backup-2026-11-08.md`:
- the baseline;
- §4's two re-measurements on the bundled SQLite;
- what `Cargo.toml`/`Cargo.lock` gained for §5;
- the eleven call sites and their policies;
- the `O07` row;
- the nine tests and the mutation table;
- the four operator lines (success, `--no-backup`, short space, failure)
  and the preview's backup line, pasted from a real run;
- the RFC 142 closure-prerequisite table;
- per-hunk hashes;
- the matrix scope line and summary from the real tree.

**On sizing:** one round. If the free-space dependency turns out to need a
crate not already in `Cargo.lock`, stop and say so first: a new dependency
is a decision, not a detail.
