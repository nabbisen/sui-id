# M2c — the release-binary rotation drill

**Run.** 2026-11-25, by the architect. It is part of M2c's closure
evidence. The [cycle plan](cycle-p-and-0.94.0-2026-11-24.md), §3, set it,
and `@nabbisen` agreed: *"Yes, both (Recommended)"*.

**The commit.** `2660276` (P3), in a clean sibling clone, built
`--release --locked` twice:
- **as shipped**, with no features. This is what an operator runs, and
  it ran every recovery and every server start;
- **with `test-support`**, the only build in which `SUI_ID_FAILPOINT`
  exists, used only for the rotation that is aborted. The shipped binary
  has no failpoints, by design (`key_adapter::default_adapter`).

**What it proves, and what it does not.**
- **It proves:** on a real filesystem, a real process dying at three
  named points of a real rotation leaves a state that the **shipped**
  binary recovers, as `crash-matrix.md`'s table requires. The rotation
  lock is released by the kernel, not by test code.
- **It does not prove** power-loss behaviour: a real power loss cannot be
  produced honestly here. The Simulated adapter's enumeration of every
  power-loss state is that evidence
  (`rotate::tests::every_crash_matrix_point_reaches_the_required_outcome`).
- **It does not prove** the reseal of populated tables. An instance that
  has not completed setup holds no sealed rows (`signing_keys` = 0 here),
  and no CLI command creates one. That is K08's test (all six tables, one
  row each, every count pinned), and the crash-matrix test's seeded
  database.

## Method

For each point, in a fresh directory:
1. start the shipped server once, then stop it, so the instance is real;
2. rotate with the test-support build and
   `SUI_ID_FAILPOINT=<point>:abort`, which aborts the process at that
   point with SIGABRT (exit 134);
3. run any command with the shipped build. `admin user unlock` opens the
   database, so it runs recovery first;
4. check which key the key file holds, the `master_key_rotation` row, and
   the audit log;
5. start the shipped server on the result.

## Results

| Abort point | What the abort left | What the shipped binary did | Outcome |
|---|---|---|---|
| `rotate.next.create_new`, **after `Intent`**, before the new key is written | workspace with `intent.json`, key, lock | *"a stopped rotation was found and cleaned; sui-id continues on the active key"*; removed `intent.json` and the workspace | **Old key** (unchanged). No rotation row. No `resealed`/`rotated` event. Server starts. **`OldReady`**, as the table requires before K08 |
| `rotate.activate.activate`, **after K08**, before activation | workspace, `next`, `pre-rotation`, key | *"a rotation was completed at start … the active key file was replaced with the already-resealed new key"*; then cleaned | **New key**. Row `complete`. `resealed` 1, `rotated` 1. Server starts. **Row 8 → `Complete`** |
| `rotate.activate.sync_active_dir`, **after activation**, before K05 | workspace, `pre-rotation`, key (already the new one) | *"a rotation was completed at start … the active key file already held the resealed new key"*; then cleaned | **New key**. Row `complete`. `resealed` 1, `rotated` 1. Server starts. **Row 9 → `Complete`** |

In every case the backup (`t.key.pre-rotation`) is kept, under its
disposition (stage 7), and only the permanent `t.key.rotation-lock`
remains besides it. Each recovery printed what it did, then the
command's own unrelated result (*"no such user"*, exit 1). Raw output is
kept outside git at `.git-exclude/tmp/drill-results.txt`.

**Verdict.** At all three points, the shipped binary reached the outcome
the table requires, with no operator action, and the server started on
the right key.
