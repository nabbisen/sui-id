# Move `crates/sui-id/src/backup/` into `sui-id-store`

**Governing RFC.** [RFC 094](../../accepted/094-transactional-audit-registry.md),
the M2a exit condition that raw database access is confined to `sui-id-store` by
the dependency graph. The migration checklist's open item *"Owner decision
required: where `crates/sui-id/src/backup/` belongs"* is now ruled.
**Ruling.** `@nabbisen`, 2026-09-16: **move it into `sui-id-store`** (option (a)),
under the standing principle of a finally clean, safe and secure, robust and
sophisticated design. The rejected alternative was to keep it in `sui-id` as a
declared raw-access exception.
**Why this matters.** `backup/` is the last reason the `sui-id` crate depends on
`rusqlite` (`crates/sui-id/Cargo.toml:84`). With it moved, the dependency graph
alone guarantees no module outside the store can open the database, with no
exception anyone has to remember.
**Implementer.** Mid-capability model. **Baseline.** `49a2897` or later.

## This is not a mechanical move — design first

Measured at the ruling. `backup/ops.rs` cannot compile inside `sui-id-store` as it
stands:

| Coupling | Where | Resolution |
|---|---|---|
| Takes `sui-id`'s runtime `Config` | `run_backup(cfg: &Config, …)` and `run_restore`, reading `storage.db_path`, `storage.key_file`, `server.issuer` | The store API takes those three values explicitly. `sui-id` keeps a thin CLI adapter that reads them from `Config`. |
| Uses `anyhow` (`Context`, `bail!`) | throughout `ops.rs` and `tar.rs` | A typed error in the store (a `BackupError` enum, or new `StoreError` variants — choose one and say why), one variant per distinct refusal, so the CLI can still print the same messages. |
| Passphrase KDF uses `argon2` | `ops.rs` encrypted-archive path | `sui-id-store` gains `argon2` as a dependency (it already carries `chacha20poly1305` and `rand`). Do not route the KDF through `sui-id-core`. |

**Target shape.** `crates/sui-id-store/src/backup.rs` + `backup/{ops,tar,types}.rs`
(file-plus-subdirectory, tests in `backup/tests.rs` per the repository's test
rule). Public surface: `run_backup`, `run_restore`, `run_verify`, `BackupOptions`,
`RestoreOptions`, `VerifyReport`, `Manifest`, `FORMAT_VERSION` — the same names.
`crates/sui-id/src/cli.rs` calls them through the adapter. `crates/sui-id/tests/e2e/backup.rs`
keeps testing through the CLI-facing path.

**Then** remove `rusqlite` from `crates/sui-id/Cargo.toml`. If anything else in
`sui-id` still names `rusqlite`, stop and report it: that is a second raw-access
site the M2a exit condition has to account for.

## The observational-equivalence record — RFC 096's bar, applied here

Required by the migration checklist for this move, and defined in
[RFC 096](../../accepted/096-upstream-oidc-federation-validation.md) (the
preparatory change record, strengthened by owner decision 2026-08-27, Part B,
B2). Relocating code is exactly where a check gets silently dropped.

1. **The move list.** Every item moved, from which file to which file. Anything
   that is not a pure move — the `Config` split, the error type, the dependency —
   listed separately with its reason.
2. **Enumerate every security and safety check in the moved code before moving
   it.** At minimum, from `ops.rs` and `tar.rs` as they stand:
   - refusing to overwrite an existing backup file;
   - refusing to back up when the database file or the key file is missing;
   - refusing to restore over an existing database or key file without `--force`;
   - refusing an archive whose `format_version` is newer than this build knows;
   - refusing an archive whose `schema_version` is newer than the latest migration;
   - archive and restored files written with mode `0600`, atomically;
   - an encrypted archive failing to open with a wrong passphrase (the AEAD tag);
   - tar parsing: entry name too long, truncated entry, archive with no entries,
     invalid octal digit.

   If the read finds more, add them. If a check cannot be triggered in isolation,
   say so explicitly — per RFC 096, that is a finding about the code, not a gap
   to pass over.
3. **One triggering input per check, isolated.** An input that only that check
   rejects, shown rejected before the move (at the baseline) and after it, with
   the same outcome. Outcome coverage alone does not count: several checks share
   the single outcome "refused", so a dropped check can hide behind another.
4. **Happy paths unchanged.** Backup → verify → restore round-trip, plain and
   encrypted, with the restored database opening and its schema version equal.

## Evidence

- `crates/sui-id/Cargo.toml` no longer names `rusqlite`; `cargo tree -p sui-id -e normal`
  shows `rusqlite` only via `sui-id-store`.
- The record above, with a table of checks × trigger × before × after.
- fmt, both clippy scopes, `cargo test --workspace` (report the count before and
  after; moved tests keep their names), MSRV 1.95 check.
- G13 unchanged (backup writes no audit events), G11, G14, G15.

## Stop and report if

- a check has no isolatable trigger;
- any caller outside `cli.rs` and `tests/e2e/backup.rs` exists;
- moving the KDF or the tar parser needs a behaviour change to compile.

## After this lands

The architect ticks the migration checklist item, updates RFC 094's note that
called this "a separate owner decision", and re-measures the M2a exit condition.

## Rulings on the stop of 2026-09-16 — proceed with the move

The review request stopped before moving any code. The "before" half of the
equivalence record is committed as `f18af38`: 31 e2e triggers plus K28, with the
mutation isolation shown. Rulings:

1. **K23, K25, K27 (§1.1): move as they are.** Record them in the equivalence
   table as *unreachable defensive checks*, with the reason for each. Nothing is
   deleted in the move: a pure move stays pure, and one review should not have to
   judge a simplification and a relocation at once. K23's dead guard is noted for
   the hardening package below.
2. **`rusqlite` in `crates/sui-id/Cargo.toml` (§1.2): it stays for now.** The move
   removes `backup/`, the last *production* use in `sui-id`, and that is this
   handoff's goal. The five e2e files (and `r11_login_failure.rs`) use raw SQL
   through the public `Database::with_conn`. That is the migration checklist's
   separate M2a exit item: a `ReadConn` assertion path, `with_conn`/`with_tx` made
   `pub(crate)`, and the manifest gate. Its rule already excludes the shortcuts:
   no `test-support` feature re-exporting raw access, and `rusqlite` in no
   `[dev-dependencies]` outside the store. The dependency is removed when that
   item lands. **Replace the evidence item "`crates/sui-id/Cargo.toml` no longer
   names `rusqlite`" with:** `grep -rn rusqlite crates/sui-id/src` returns nothing.
3. **The new caller `tests/e2e/backup_checks.rs` (§1.3): accepted.** It is the
   equivalence mechanism, and it goes through the adapter.
4. **The design in §4: approved as written.**
   - A dedicated `BackupError` enum, not new `StoreError` variants, because the
     `CoreError` HTTP mapping must not grow archive refusals no request can
     produce.
   - One variant per refusal, with today's messages as `Display`.
   - Sources kept with `#[source]`.
   - One `FORMAT_VERSION`.
   - A thin adapter in `sui-id` with today's signatures, so `cli.rs`,
     `tests/e2e/backup.rs` and `tests/e2e/backup_checks.rs` stay byte-identical.
     Show that they are, by hash, before and after.
5. **F4 (the docs say `verify` "never writes anything") is fixed in this
   package.** It stages a snapshot in a temporary directory, and
   `docs/src/guides/deployment.md` says so after the move.
6. **F1, F2, F3 and F5 are not part of the move.** A move must not change them
   silently, and it does not change them loudly either.
   - **F1** — a manifest-less archive bypasses both version refusals.
   - **F2** — `verify` does not apply the compatibility checks.
   - **F3** — `restore` validates neither the database nor the key it writes.

   These three are proposed to the owner as a separate hardening package. F5 (the
   swallowed `create_dir_all` error) and K23 go with it.
