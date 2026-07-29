# RFC 100 verification and closure

**Governing RFC:** [RFC 100](../../proposed/100-master-key-rotation-recovery.md)

## Evidence contract

Inherits [RFC 093's contract](../093-build-toolchain-release-gates/verification.md):
hash-pinned packages, every change described, files frozen while review is open,
staging by path, and evidence bound to one commit.

Additional to this RFC:

- **Every crash-matrix row is observed, not argued.** A row marked "handled by
  construction" without an executed test does not close.
- **State is constructed directly.** Racing a real rotation to reach a prefix is
  not deterministic and is not accepted as evidence for that row.
- **Both failure modes per injection point** — short write and process
  termination — are exercised independently.
- **Fingerprints, never key material, appear in evidence.** No log, test output,
  journal, audit record, or review package may contain key bytes.

## Closure criteria

1. Every row of the recovery table has an executed crash-injection test at every
   applicable injection point.
2. No prefix produces mixed ciphertext, a missing active key, or an overstated
   audit record.
3. `K04` and `K05` use the RFC 094 Class-A seam with observed rollback evidence
   for injected append and commit failures.
4. `admin.master_key.database_resealed` is proven not to imply completion: a
   test asserts that a crash between K04 and activation leaves an audit trail
   that correctly describes a pending activation.
5. CLI success is proven unreachable before K05 commits.
6. Old-key custody is recorded before `Complete`; a retained backup is never a
   startup candidate; removal records timestamp and operator identity.
7. Platform and provider refusals occur before any file is created.
8. Operator documentation describes rotation, recovery and custody accurately,
   and the "not yet crash-safe" warning is removed **in the same change** that
   closes M2c — not before.
9. Independent adversarial closure review, **by a reviewer who did not author
   RFC 100**, accepts the evidence.

## What closure does not establish

M2c closure does not establish M6 readiness, soak entry, or any release claim.
It satisfies one prerequisite of RFC 099: that the M7 master-key rotation
exercise can be run against the immutable soak artifact without risking
unrecoverable divergence.
