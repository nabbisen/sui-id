# M2c — closure assessment (RFC 100)

> ## M2c CLOSED 2026-11-25, when P7 landed
> P7 landed as `070e7f7`, CI `45577018318` 28/28, with the review's two
> fold-ins: [review](0.94.0-p7-what-the-philosophy-check-found-2026-11-25.md).
> The condition below is met.
>
> ## Accepted 2026-11-25, on a condition
> `@nabbisen`: *"Accepted. Verify carefully they match my fundamental
> philosophy."* The architect walked the shipped build and found it did
> not yet match either half:
> - a rotation runs beside a running server, which breaks M1 silently;
> - operator text an operator can misread.
>
> Asked how the closure should stand, he chose *"Fix first; close M2c on
> that (Recommended)"*. The findings, and their fix, are
> [P7](0.94.0-p7-what-the-philosophy-check-found-2026-11-25.md). **M2c was
> recorded closed when P7 landed** (above).

**Written.** 2026-11-25, by the architect.

**Not independent.** The architect wrote RFC 100's refresh, every stage's
dispatch, and folded code into most of them.

**CI on every stage's commit is green.** The last is P3's, `2660276`: run
`45518872512`, 28/28 jobs.

## 1 — what was built

| Stage | What | Commit |
|---|---|---|
| 0 | the crash-injection harness: Real, Failpoint, and the Simulated power-loss adapter | `ddcf4b0` |
| 1 | the sentinel and the startup key check; never a key generated over sealed data | `92b3d01` |
| 2 | preconditions, and `--check` | `79ed9c5` |
| 3 | the journal: a lock you can see, states that only move forward | `445a97e` |
| 4 | startup recovery of the preparation prefix (rows 1–7, 11) | `643aa47` |
| P1 | the database half: the row's full shape (0053), K08, K05, rows 8–10 | `c3b8920` |
| P2 | the live rotation, rewritten once to the whole protocol; the liveness lock; the custody choice | `59ff4c9` |
| P3 | custody (K09, K10, the overdue finding), and the operator's guide | `2660276` |

Every stage landed with the full local matrix green and CI green.

## 2 — RFC 100's acceptance criteria, one by one

| Criterion | Evidence |
|---|---|
| **Every recovery-table row has an executed crash-injection test** | Rows 1–7 and 11: stage 4's tests, each state built directly, plus recovery's own crash safety in power-loss mode. Rows 8–10: P1's, including activation under power loss. **Every injection point, 1–10, in all three modes**: P2's `every_crash_matrix_point_reaches_the_required_outcome`, which materializes every power-loss and kill state and runs the real recovery; the real binary under termination, sampled; and **the release-binary drill** ([record](m2c-release-binary-drill-2026-11-25.md)) at three points |
| **No prefix produces mixed ciphertext, a missing active key, or an overstated audit record** | **Mixed ciphertext:** K08 reseals every sealed column and the sentinel in one Class-A transaction, so a prefix either has all of it or none (rollback evidence below). **A missing key:** activation is one `rename`, never remove-then-rename, and P1's power-loss test asserts the active key file is never missing (its mutation, remove-then-rename, was caught). **An overstated audit:** `resealed` claims only the database phase, and `rotated` is written only by K05, after activation. The crash matrix requires `rotated` exactly once at `Complete`, and none before |
| **K04/K05 use the RFC 094 Class-A seam with observed rollback evidence** | K08 (RFC 100's K04): named failpoints between reseal and row, and between row and commit, each leaving nothing. K05: likewise. Both are durable on return (a WAL checkpoint to zero, asserted). G20, the M2a rollback-coverage gate, is satisfied for all 66 commands |
| **Operator documentation describes rotation, recovery and custody accurately** | `docs/src/guides/operators.md`'s rotation section, rewritten in P3 and corrected in its review: the custody choice, the lock, `--check`, recovery unaided, what it refuses, the way back, `backup remove`/`renew`, and the finding. Also `audit-events.md`'s rows, and the CLI reference, generated |
| **Independent closure review accepts the evidence** | **This document's purpose.** Not yet given |

## 3 — the invariants, and the crash matrix's additional cases

- **M1, never mixed keys:** as above, one transaction.
- **M2, always a key that opens the database:** activation is atomic, and
  P2's review fold-in means a non-crash failure cannot leave an instance
  that refuses to start. The kept-backup and stray-`next` preconditions
  refuse before anything is created.
- **M3, no artifact before the journal binds it:** `next` and the backup
  are published only after `Intent`, and row 2 refuses an unbound one.
- **M4:** two events, as above.
- **M5, custody explicit:** the disposition is required before `Intent`
  (a usage error otherwise), stored structured with `CHECK`s, and settled
  by K09/K10.
- **M6, any mismatch fails closed:** P1's fold-in added the fingerprint
  case, the one the dev team had judged unreachable. The others (owner,
  mode, symlink, sentinel, rotation ID) are tested in stage 4 and P1.

The crash matrix's *Additional required cases*:

| Case | Evidence |
|---|---|
| provider precedence supplied the key | refused before `Intent` (`0.91.0`; P1 precondition) |
| paths span filesystems, or a primitive is missing | refused before the workspace (stage 2's probe) |
| a workspace already exists | a new rotation refuses (P3 precondition); **a running one cannot be cleaned** (the liveness lock) |
| journal authenticated with the new key after reseal | rows 8–9 |
| a retained backup present at startup | never a candidate (P3, by mutation) |
| `review_at` elapsed | the finding at every start and in `--check`, repeated (P3) |
| K05 run twice | idempotent, no second event (P1) |

## 4 — what the evidence does not show (read this first)

1. **A real power loss.** It is shown only in simulation. The Simulated
   adapter's model is stage 0's, with the second directory level added in
   stage 3, and it is only as faithful as that model. The release-binary
   drill covers process death on a real filesystem, not power loss.
2. **The reseal of populated tables in the drill.** The drill's instance
   had no sealed rows. That property rests on K08's six-table test and the
   crash-matrix test's seeded database.
3. **`flock` is advisory, and local.** The liveness lock assumes the key
   file's directory is on a local filesystem, as the same-filesystem
   precondition already does. Over a network filesystem, `flock`'s
   guarantees are the server's. No test covers that case.
4. **The CLI's operator identity is not authenticated.** K09 records the
   OS account, labelled `os_user`. That is the only identity the command
   line has.
5. **The overdue finding has no console card.** It is at every start, and
   in `--check`. The card is carried into RFC 149, cycle Q.
6. **Found and fixed during the build, not in a release:**
   - P2's second-rotation brick;
   - P1's untested M6 case;
   - stage 3's vacuous tamper test.

   **Each was found by review, not by a gate**, which argues for keeping
   adversarial review on stages like these.

## 5 — recommendation

**Close M2c.** Every acceptance criterion but the last is met by landed
evidence, cited above, and §4 states its limits. Closing it lets
`0.94.0`'s release say rotation is crash-safe, which the operator's guide
already does. RFC 100 then moves to `done/` at `0.94.0`'s cut, as
*Implemented (v0.94.0)*.
