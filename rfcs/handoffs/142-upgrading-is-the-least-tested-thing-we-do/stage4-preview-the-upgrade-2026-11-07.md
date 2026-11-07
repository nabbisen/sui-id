# RFC 142 stage 4 — see the upgrade before it happens: `sui-id preview-upgrade`

> ## Whose turn it is: **the implementation role's.**
> Cycle J stage 8 —
> [`../094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md`](../094-transactional-audit/cycle-j-and-0.88.0-2026-11-01.md)
> §D. Scoped and authorized 2026-11-07.

**Dispatched.** 2026-11-07, by the architect.
**Governing RFC.** [RFC 142](../../accepted/142-upgrading-is-the-least-tested-thing-we-do.md)
**D3**, **Accepted**. Building is permitted.
**Baseline.** The tip of `main` when you start — `git log -1`, record it.

## 1 — what it is, in the RFC's words

> A read-only subcommand that opens nothing for writing and reports: the
> stored schema version, the release that last migrated the file, the list
> of migrations that **would** apply, and the version it would end at. Then
> exits. … **It must not write.** Not the version, not `sui_meta`, not a WAL
> frame it did not need.

`sui-id preview-upgrade [--config PATH]`. The name follows the CLI's
verb-object convention (`verify-backup`). It belongs to the binary being
upgraded *to*: *"what would **this** sui-id do to my database?"*

## 2 — what the operator reads

**The words below are fixed by this dispatch.** Change them only to make
them true, and say so in the return if you do. The versions, counts and path
in the examples are illustrative; the real ones are computed (see the end of
this section).

**Migrations pending:**

```
sui-id 0.88.0: upgrade preview for /var/lib/sui-id/sui-id.sqlite
  Schema version now:      44 (last migrated by sui-id 0.86.0)
  This sui-id migrates to: 49
  5 migrations would run when this sui-id starts:
    0045_<file stem>
    0046_<file stem>
    …
  Nothing was changed. Back up before starting this sui-id: docs/src/guides/upgrade.md
```

**Already at head:** `The database is already at schema version 49.
Starting this sui-id runs no migrations. Nothing was changed.`

**No database yet** (the path does not exist), or a fresh one
(`StoredVersion::Fresh`): `No database exists at <path> yet` / `The database
at <path> is empty`, then `Starting this sui-id creates its schema and runs
all 49 migrations. Nothing was changed.`

**The release is not recorded** (`last_migrated_by` is `None`): `(the release
that last migrated it is not recorded)`. **Measure which release first wrote
`last_migrated_by`** (RFC 112 D7) and, if it can be stated truthfully, say
*"recorded since sui-id X"* in the guide, not in this line.

**A database this build refuses** (`SchemaTooNew`, `SchemaVersionInvalid`):
print **exactly the line `database::check` already produces**, and exit
**65**. The preview's answer to *"what would happen?"* is *"it would refuse,
and here is the line it would print"*. Exit 65 already means *"this database
is not one this build can use"* across the CLI, and must keep one meaning.
Any other failure is exit 1, as everywhere else.

**Exit 0** otherwise, including when migrations are pending: a preview that
succeeded is a success.

Counts and versions are computed, never typed. Use `MAX_SCHEMA_VERSION`, and
the length of the pending list.

## 3 — one definition of "pending", shared with the runner

`migrations::run` decides what to apply with `if i64::from(m.version) <=
current { continue; }` (`migrations.rs:605-610`). **The preview must not
re-derive that.** Factor it into one function in `migrations.rs`, for example
`pending(stored) -> impl Iterator<Item = &'static Migration>` or a `Vec` of
`(version, name)`. Then:
- **`run` iterates it.** `run_up_to` may keep its own `max_version` bound,
  but it takes its lower bound from the same function.
- **The preview lists it.**

The preview and the runner then cannot disagree about which migrations will
run, by construction. **A test that runs both and compares them is still
required** (§6), because "by construction" is a claim about today's code and
the test is what keeps it true.

## 4 — migrations get their names

`struct Migration { version, sql }` has no name, so the preview could only
print numbers. A list of numbers tells an operator nothing. **Add `name:
&'static str`, the file stem** (`"0049_client_auth_method_and_grant_types"`),
to all 49 entries, and print it verbatim. It is greppable, and it is what
`git log` and the CHANGELOG already call it.

**And make it impossible for a name to lie.** One test in the store crate,
reading `CARGO_MANIFEST_DIR/src/migrations/`:
- every entry's `<name>.sql` exists and its bytes equal that entry's `sql`;
- the numeric prefix of `name` equals `version`;
- **every `.sql` file in that directory is registered**, so a migration
  file added but never wired into `MIGRATIONS` fails here instead of
  silently never running.

If an existing test already asserts part of this, extend it rather than
adding a second one, and say which.

## 5 — "it must not write": measured, and the measurement found a trap

**The architect measured the premise before writing this** (Python's
`sqlite3`, SQLite 3.54.0, scratchpad, not in the tree). For a cleanly closed
**WAL** database, the shape sui-id leaves after a clean stop:

| step | files beside the database |
|---|---|
| before | `wal.sqlite` |
| during a `mode=ro` connection | `wal.sqlite`, `wal.sqlite-shm`, `wal.sqlite-wal` |
| **after it closes** | **`wal.sqlite`, `wal.sqlite-shm`, `wal.sqlite-wal` — left behind** |

The database's own bytes were unchanged. A rollback-journal database got no
sibling files. With a live writer, the read-only connection saw its committed
WAL frames, and the files went away when the *writer* closed.

**Why it matters, beyond the RFC's own "not a WAL frame":** an operator who
runs the preview **as root** before starting the service leaves root-owned
`-shm`/`-wal` files beside a database the service user must then write. The
preview's whole purpose is to be the harmless thing you run first. It must
leave the directory exactly as it found it.

**The workspace bundles SQLite 3.53.2** (`libsqlite3-sys` 0.38.2), not 3.54.0.
**Re-measure with it, through `rusqlite`, first**, and report the table above
for the bundled library. If the bundled library does **not** create the files,
say so and use plain `SQLITE_OPEN_READ_ONLY`.

**If it does, the remedy is:**
- **No `<db>-wal` beside the database** (a clean stop, so no writer has
  unseen frames): open `file:<path>?mode=ro&immutable=1`. `immutable` tells
  SQLite the file cannot change, so it creates no `-shm` and reads no WAL.
- **`<db>-wal` exists** (a running server, or an unclean stop): open plain
  read-only. The sibling files already exist, so nothing is created, and the
  committed WAL frames are read.
- **The race**, a server starting between that check and the open, can make
  the preview's *reading* stale. It cannot make it *write*, because
  `immutable` connections are read-only. Say so in the code comment, in one
  sentence.

**Do not change `check_database_file`** or the startup path in this stage.
They leave the same files, but both callers, `Database::open`
(`crates/sui-id-store/src/db.rs:41`) and `admin rotate-key`'s early check
(`crates/sui-id/src/cli.rs:385`), go on to open the database for writing,
which removes them on close. **The exception is a refusal**: a `SchemaTooNew`
or `SchemaVersionInvalid` database is refused after the read-only open
(including `last_migrated_by_file`), so the files stay. That is an error
state already, but a root-run command would leave root-owned files there
too. Record it in the return as a measured observation, not a fix.

**The preview needs no master key.** It reads `sui_meta` only, so it loads the
config for `storage.db_path` and **must not resolve the key**. A preview that
fails because the key file is elsewhere would be the wrong tool. Test it
(§6).

## 6 — tests

All black-box against real database files, in a temp directory:

1. **Pending.** A database built to version 44 (the store crate already has
   `run_up_to`; expose what the CLI test needs through a test-only seam, or
   build in the store crate and call the preview's core function from
   there). The output lists exactly 0045–0049, in order, with names, and
   ends at `MAX_SCHEMA_VERSION`. Exit 0.
2. **At head.** Exit 0, the "already at" line, no list.
3. **No file**, and **an empty file**. The "creates its schema" lines.
   **For the missing path, assert that the path still does not exist
   afterwards.**
4. **Too new**, and **invalid**. The exact `database::check` line, exit 65.
5. **Writes nothing**, for each of a rollback-journal database, a cleanly
   closed WAL database, and a WAL database **with a live writer connection
   open**: the database file's SHA-256 is identical before and after, and the
   **set of file names in the directory is identical** before and after. In
   the live-writer case, the preview reports the version committed in the
   WAL, not a stale one.
6. **No key needed.** A config whose `key_file` does not exist. The preview
   succeeds.
7. **The preview and the runner agree.** For every `N` in `0..MAX`: build to
   `N`, take the preview's list, run `migrations::run`, and assert that the
   versions it applied are exactly that list. Observe them however is
   cleanest, for example by reading `schema_version` before and after.
8. **Names cannot lie** (§4).

**Mutations: run them and report them.** Each must turn a named test red:
- the preview using its own `>` comparison instead of the shared function,
  off by one (`>=`);
- dropping `immutable=1` (if §5 needed it);
- resolving the key;
- an unregistered `.sql` file;
- swapping two entries' `name`s.

## 7 — wiring and documentation

- `cli.rs`: `TOP_LEVEL_SUBCOMMANDS`, `print_help`'s text (in the existing
  style, and saying plainly that it writes nothing), and `main.rs`'s
  dispatch. The existing `cli_bindings_cover_help_and_dispatch` test must
  cover it unedited.
- **`docs/src/guides/upgrade.md`, General procedure.** A new step between
  "Replace the binary" and "Start": run `sui-id preview-upgrade --config
  <config>` with the **new** binary, read which migrations will run, and
  know it changed nothing. Renumber the steps after it.
- **`docs/src/reference/configuration.md`, the Subcommands table: add the
  row, and correct the rows around it.** Measured today against `cli.rs`,
  **the table is wrong in at least these places**:
  - `backup` takes `--to`, not `--dest`; and it is not always encrypted
    (`--encrypt` is optional);
  - `restore` and `verify-backup` take `--from`, not `--src`; and `restore`
    **refuses** to overwrite unless `--force` is given. It does not prompt;
  - `admin unlock-user` takes `--username NAME`, not a positional name;
  - **`admin rotate-key` is described as *"Create and activate a new Ed25519
    signing key; retire the old one."* It does nothing of the kind.** It
    re-seals every encrypted column under a new **master** key, offline. An
    operator who believes that row could run it expecting a routine signing
    key rotation;
  - `admin reset-mfa`, `admin revoke-registration-token`, `admin
    list-registration-tokens` and `admin legacy-dynamic-candidates` are
    missing.

  **Check every row against `print_help` and the code yourself**, not
  against this list. Adding a correct row beside wrong ones would leave the
  table looking more trustworthy than it is.
- **`CHANGELOG.md`**, `[Unreleased]` → `### Added`: the subcommand, in an
  operator's words. Then `### Fixed` (or `Changed`, if that fits this
  file's convention better; say which and why): the reference table now
  matches the commands, naming `rotate-key` specifically.

## 8 — constraints

- **No migration**, no schema change, and nothing in G22's corpus.
- **Do not change `check_database_file`** or the startup open (§5).
- **Gate evidence:** this is a Rust change, so `bash scripts/local-matrix.sh
  --changed` *is* the complete matrix. Quote its scope line, which will say
  *"Rust scope touched"*, as `docs/src/contributing/release-process.md` now
  describes. Run it on the real tree. A pre-check clone, if you use one, goes
  beside the tree on the main disk.
- No tag, no `workflow_dispatch`, no Level B claim.

## 9 — what to return

A review request at
`.git-exclude/review-requests/cycle-j-stage8-rfc142-stage4-preview-upgrade-2026-11-07.md`:
- the baseline;
- §5's table for the bundled SQLite, and which open you chose;
- the release that first wrote `last_migrated_by`;
- the shared "pending" function and its two callers;
- the names test, and anything it found;
- the eight tests and the mutation table;
- the corrected `configuration.md` rows, each with the code line that
  proves it;
- the four output forms, pasted from a real run;
- per-hunk hashes;
- the matrix scope line and summary from the real tree.

**On sizing:** one round, I think. The CLI is small. §4 touches 49 entries
mechanically, §6's agreement test is a loop, and the table correction is
documentation. If §5's measurement differs from mine, stop and report it
before building on either reading.
