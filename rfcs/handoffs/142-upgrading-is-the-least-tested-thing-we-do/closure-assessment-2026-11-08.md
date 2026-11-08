# RFC 142 — closure assessment, 2026-11-08

> ## Closure approved. `@nabbisen`, 2026-11-08: *"Approved."*
> RFC 142 moves to `done/` as `Implemented (v0.88.0)` in the `0.88.0`
> preparation, with this document as its `Closure evidence`.

**Prepared by.** The architect, 2026-11-08.
**Asked of you.** Approve RFC 142's closure. On approval, the
architect moves it to `done/` with `Status: Implemented (v0.88.0)` **in the
`0.88.0` preparation**, not before. RFC 000 marks an RFC Implemented when its
work has shipped, and stages 4–5 ship in `0.88.0`.
**Closure evidence commit.** `1573a72`, the commit on which stage 5 landed
and the last change to RFC 142's code.
**Level B on that exact commit** (RFC 131 D2). `workflow_dispatch` run
`42435137349`: **28 jobs, 28 success, 0 failed, 0 skipped**. `Security audit` on the same commit: run
`42435139326`, **success**.

## Independence, stated plainly

**The architect is not independent of this RFC**, so the field below must not
be read as if it were:
- wrote it;
- reviewed its design at the owner's request (§S1c);
- dispatched all five stages;
- and **folded code into stages 4 and 5 at review** (the URI encoding and the
  runner-observation test in stage 4; the operator size wording and a test
  guard in stage 5).

That last point makes the architect a co-implementer. The project's rule is
that *"the implementer cannot be the sole approver of a security-sensitive
design or its closure evidence"* (`docs/development-specification.md:1011`).
**This document is therefore preparation, not approval.** Your approval is the
closure.

## The four closure prerequisites, against the evidence

RFC 142's field, verbatim, one clause per row:

| Clause | Met by | Evidence |
|---|---|---|
| *"A populated database at each covered historical schema version migrates to head in CI with its rows and key invariants intact"* | Stage 2: **G22**, 38 fixtures (37 shipped schema versions by replay, two `audit_log` fixtures, the hand-built `0.29.7` one), per-table row counts and each migration's claimed invariant | G22 green in Level B above; `crates/sui-id-store/src/tests_rfc142.rs` |
| *"the supported upgrade range is stated in `docs/src/guides/upgrade.md` and, where a floor exists, enforced in code with a negative fixture"* | Stage 3: *"You can upgrade straight to the current release from any earlier one"*, under **Migration behaviour**. **No floor exists**, since G22 brings every shipped version to head, so no `SchemaTooOld` and no fixture is owed | `docs/src/guides/upgrade.md` §Migration behaviour |
| *"an operator can see what an upgrade would do before it does it"* | Stage 4: `sui-id preview-upgrade`. Read-only, leaves the directory unchanged (three journal modes tested), needs no key, lists the pending migrations by name, and its list is checked against what the runner actually applies | `tests_rfc142_stage4.rs`, `tests/e2e/r142_stage4_preview_upgrade.rs` |
| *"whatever D4 settles on is implemented with a test that proves the refusal or the backup actually happens"* | Stage 5: the pre-migration `VACUUM INTO` snapshot. One test proves it happens (valid, at the old schema, `0600`), others that it is skipped when nothing is pending, and that **both refusals happen**: short space and a failed write, each with nothing written | `tests/e2e/r142_stage5_pre_migration_backup.rs`, `backup/tests.rs` |

**All four are met.** No prerequisite rests on a document's own claim; each
cites a test or the guide's text, and the tests ran green in Level B on the
cited commit.

## What the stages found that the RFC did not foresee

Recorded so that closure does not tidy them away:

1. **D2's phrasing was too broad, and was narrowed before it reached an
   operator** (stage 3). *"Any supported release upgrades directly to any
   later one"* is not what G22 proves. The guide says *"straight to the
   current release from any earlier one"*, which is what it does prove.
2. **"It must not write" needed more than a read-only open** (stage 4). A
   read-only open of a cleanly closed WAL database leaves `-shm`/`-wal`
   files behind. The preview uses `immutable=1` when no `-wal` exists, through
   a **percent-encoded** URI. Unencoded, a `#` or `?` in a directory name made
   a populated database preview as **empty**.
3. **Migrations run on every database open, not only at server start**
   (stage 5). So the backup policy became a required parameter of
   `sui_id::database::open`, and all eleven callers chose: the server, nine
   `admin` subcommands, and `setup`.

## Observations, none blocking

- **G22 widened silently.** Its command filters on `tests_rfc142`, and cargo
  matches test names by substring, so since stage 4 it also runs
  `tests_rfc142_stage4`. That is useful coverage, but the lane's scope grew
  without anyone deciding it should. Either rename the filter to say what it
  means, or record that the lane now covers stage 4. **Recommended:** record
  it in RFC 142's lane section at closure.
- **`Touches`** lists four paths. Stages 4–5 also changed
  `crates/sui-id-store/src/backup/ops.rs`, `crates/sui-id/src/main.rs`,
  `crates/sui-id/src/runtime/startup.rs`, `docs/src/reference/configuration.md`,
  and `Cargo.toml` (a direct `rustix` dependency).
  **Recommended:** correct the field at closure, so the record names what the
  RFC actually touched.
- **Two measured residuals, recorded rather than fixed.** On a database this
  build **refuses**, the refusal path's own read-only open can leave
  `-shm`/`-wal` behind, which is an error state already. And `runtime/dev_mode.rs`
  opens its pinned database without the snapshot, by design: it truncates
  the file first, so there is never anything to back up.

## Recommendation

**Approve closure.** On your word, the `0.88.0` preparation moves RFC 142 to
`done/` with:
- `Status: Implemented (v0.88.0)`;
- `Closure reviewed on: 2026-11-08`;
- `Closure approved by:` you, with your words and date;
- `Closure evidence:` this document;
- and the two corrections above, G22's recorded scope and `Touches`, in the
  same change.
