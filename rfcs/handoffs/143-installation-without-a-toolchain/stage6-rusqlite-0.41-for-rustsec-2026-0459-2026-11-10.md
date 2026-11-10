# Cycle K stage 6 — `rusqlite` 0.41, for RUSTSEC-2026-0459

> ## Whose turn it is: **the implementation role's.**
> **This document is the dispatch.** It is security maintenance, which the
> freeze admits, and it belongs in `0.89.0`. Release-process step 0's rule:
> a dependency fix found before the preparation goes into the release
> being prepared.

**Dispatched.** 2026-11-10, by the architect.
**Baseline.** The tip of `main` when you start (`git log -1`; record it).
**Found by.** `Security audit` run `42793370475` on `32d04e6` (2026-11-10):
`cargo audit` **denied** RUSTSEC-2026-0459. Step 0's run the day before was
green. The advisory is dated 2026-10-30, and reached the database since then.

## 0 — the advisory, as measured

| | |
|---|---|
| Advisory | RUSTSEC-2026-0459, CVE-2026-119375, GHSA-72w2-rjgg-q729, CVSS 4.0 `AV:L/AC:L/AT:N/PR:N/UI:P/VC:H` (medium) |
| What | SQLite before 3.54.0 can return uninitialised page-cache memory when a database file is opened **together with a sibling `-wal` file whose page size differs**. It needs a crafted pair of files, and the victim must open them |
| Ours | `libsqlite3-sys 0.38.2`, which bundles SQLite 3.53.2, through `rusqlite 0.40.2` with `bundled` (root `Cargo.toml:63`) |
| Fixed | `libsqlite3-sys >= 0.39.0` (SQLite 3.54.0; 0.39.1 bundles 3.54.1). **Only `rusqlite` 0.41 reaches it**: crates.io shows 0.40.2 requires `^0.38.2`, and 0.41.0/0.41.1 require `^0.39.0`. `rusqlite` 0.41 declares Rust 1.89, within this project's MSRV of 1.95 |

**The architect's reading of sui-id's exposure: not meaningfully reachable.**
**Confirm it, don't take it on trust:**
- **The server's own database** is opened in WAL mode (`crates/sui-id-store/src/db.rs:43`).
  A crafted `-wal` beside it needs write access to the data directory, and anyone
  with that could already replace the database and the master key.
- **`verify` and `restore`** stage a backup's database bytes **alone**, into a
  fresh temporary directory (`crates/sui-id-store/src/backup/ops.rs:506-514`,
  `check_snapshot_schema`). There is no `-wal` sibling, so a crafted archive
  cannot supply the pair. **Read every other `Connection::open*` call site**
  (`migrations.rs:476/492/521/523`, `ops.rs:108/126/251`), and say for each
  whether a file another party wrote could be opened with a `-wal` beside it.

## 1 — the change

- Root `Cargo.toml`: `rusqlite = { version = "0.41", … }`, features unchanged.
- **`cargo update -p rusqlite`**, letting cargo move `libsqlite3-sys`. Then
  **quote the `Cargo.lock` diff**, and say if anything other than `rusqlite`,
  `libsqlite3-sys` and their own dependencies moved.
- **Read `rusqlite` 0.41's release notes** (its GitHub release `v0.41.0`, and
  `v0.41.1`) **before compiling**. List each breaking change, and say whether this
  code base uses what it changes. Then fix what the compiler finds. **No behaviour
  change**: this is a version bump, not a refactor.
- **The store's `hooks` feature** (`crates/sui-id-store/Cargo.toml:58`) and
  `chrono` must still exist under the same names in 0.41. Confirm.

## 2 — the record

`CHANGELOG.md` `[Unreleased]`, **`### Security`**, in operator words and in
this project's existing style for advisories (see RUSTSEC-2026-0385's entry):
- the bundled SQLite moves from 3.53.2 to 3.54.x, closing RUSTSEC-2026-0459;
- **what sui-id's exposure is**, stated as plainly as §0 establishes it (after
  you have confirmed it): which files sui-id opens, and why a crafted `-wal`
  pair cannot reach them other than by an attacker who already controls the
  data directory;
- **which builds carry the fix**: every build bundles SQLite (`cargo install`,
  source, and the static binaries), so all of them.

**`0.89.0`'s one-line summary must then name `Security`** (release-summary
rule). That is the preparation's job; this stage only writes the entry.

## 3 — constraints and what to return

- **No tag, no dispatch.** The architect re-runs `Security audit` on the commit.
- **`cargo audit` locally with the lane's own flags**, copied from
  `.github/workflows/audit.yml`'s `Run cargo audit` step, not a bare `cargo audit`.
  Quote the result.
- **`bash scripts/local-matrix.sh`** (the lockfile changes, so expect the
  complete matrix), quoting the scope line and **the names of any failing
  checks**. G22 (the migration corpus) is in it. Say it passed, by name.
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/cycle-k-stage6-rusqlite-0.41-2026-11-10.md`.
