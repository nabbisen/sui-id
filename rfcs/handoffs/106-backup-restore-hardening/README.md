# Backup restore hardening

**Authorized by.** [`ROADMAP.md`](../../../ROADMAP.md) §Non-RFC work packages. Owner
authorization 2026-09-17. **Implementer.** Mid-capability model.
**Baseline.** The commit that adds this file, or later. It builds on the move to
`sui-id-store` (`b824246`).
**Found by.** The backup move's equivalence record (F1, F2, F3, F5, K23).

## The defects (`crates/sui-id-store/src/backup/ops.rs`)
- **F1 — a manifest-less archive bypasses both version refusals.** `parse_backup`
  fabricates a permissive manifest (format 0, schema 0) for pre-0.13 archives. A
  crafted archive without `MANIFEST.json` restores whatever schema it contains.
- **F2 — `verify` does not apply the compatibility checks.** It reports a newer
  `format_version` or `schema_version` without failing, so it passes an archive
  `restore` would refuse.
- **F3 — `restore` validates nothing it writes.** The database bytes get no
  integrity check, and the key bytes no validity check.
- **F5 — `create_dir_all(parent).ok()` swallows errors.**
- **K23 — the envelope-magic check is unreachable.**

## Required
- **F1.** For an archive without a manifest, read `schema_version` from the
  snapshot's own `sui_meta` and apply K16's refusal to it. A snapshot with no
  readable version is refused. Legacy archives with a valid schema still restore.
- **F2.** `verify` applies K15 and K16 and fails with the same messages `restore`
  would give. `deployment.md`'s daily smoke test then catches an unrestorable
  archive.
- **F3.** Before replacing anything:
  - stage the database bytes, run `PRAGMA integrity_check` on the staged copy,
    and refuse on failure;
  - validate the key bytes with the same parser the server's keyring uses for a
    32-byte master key, and refuse on failure.

  Nothing is written to the configured paths unless both pass.
- **F5.** Propagate the directory-creation error, with its own `BackupError`
  variant.
- **K23.** Remove the unreachable magic check, making `is_encrypted` the single
  check. Or make it reachable by passing the whole buffer; say which, and why.
- **Messages.** Each new refusal gets its own variant and message, and an isolated
  trigger added to `crates/sui-id/tests/e2e/backup_checks.rs` (the equivalence
  record's suite). Existing triggers must pass unchanged.

## Evidence
- One isolated trigger per new check, with mutation isolation (each removal fails
  only its own test).
- The existing 31 e2e triggers and the 13 unit tests still pass.
- `docs/src/guides/deployment.md`: `verify-backup` now fails on an archive
  `restore` would refuse; say so.
- fmt, both clippy scopes, test count before and after, MSRV 1.95.
