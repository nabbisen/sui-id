# RFC 100 developer handoff

**Governing RFC:** [RFC 100](../../accepted/100-master-key-rotation-recovery.md) — **Accepted 2026-11-14** ([design review record](./design-review-2026-11-14.md))
**Milestone:** M2c — master-key rotation recovery
**Audience:** implementer assigned after RFC 100 is Accepted
**Status:** Planning companion. The RFC is **Accepted** (2026-11-14), and **every
implementation prerequisite is met**: the crash-injection harness was approved
2026-11-14. The build is planned for `0.92.0`, stage by stage
([`implementation.md`](./implementation.md), rewritten 2026-11-14); nothing of it
is dispatched yet. `0.91.0` stage 1, with its part A2, **landed** 2026-11-14.

## Entry gate

All of:

1. RFC 100 Accepted after independent design review — **by a reviewer who did
   not author it**; the RFC records that its author should not be its security
   reviewer. **Met 2026-11-14.**
2. RFC 094 **M2a** Implemented. `K04` and `K05` are Class-A database commands and
   consume that transaction seam; they cannot be built before it exists.
   **Met** (RFC 094 closed in `v0.88.0`; the RFC's refresh makes K07 the
   database phase, so `K04` is not built).
3. Crash-injection harness approved. **Met 2026-11-14** —
   [`crash-injection-harness-design-2026-11-14.md`](./crash-injection-harness-design-2026-11-14.md).

## Why this is separate from RFC 094

Every other part of RFC 094 answers *"do a mutation and its audit record commit
or roll back together?"* This answers *"can offline key rotation survive a crash
at any point?"* — filesystem atomicity and recovery, with its own threat model:
partial key material, mixed ciphertext, an unrecoverable active key.

It was moved out of RFC 094 on 2026-07-28 so it receives the right kind of
scrutiny rather than being reviewed as an appendix to audit atomicity. No design
decision was changed in the move.

## Why it must land before M6

The M7 workload contract requires a successful master-key rotation exercise, and
RFC 099 requires zero blocker-severity defects at soak entry. Rotating a key
that is not crash-safe against the immutable soak artifact risks unrecoverable
database/key divergence. If the owner elects to defer this RFC past M6 instead,
the soak exercise must be struck by an explicit recorded decision and the
residual risk carried into RFC 097 — it may not be silently skipped.

## Documents

| Document | Contents |
|---|---|
| [`implementation.md`](./implementation.md) | Ordered build stages, file/DB surfaces, platform refusals |
| [`crash-matrix.md`](./crash-matrix.md) | Every injection point and its required outcome |
| [`verification.md`](./verification.md) | Evidence contract and closure criteria |
| [`release-0.91.0-preparation-2026-11-15.md`](./release-0.91.0-preparation-2026-11-15.md) | **Dispatched 2026-11-15**: `0.91.0`'s preparation, the mechanical half |
| [`crash-injection-harness-design-2026-11-14.md`](./crash-injection-harness-design-2026-11-14.md) | **Proposed 2026-11-14, for approval**: the harness, the last implementation prerequisite. Three failure modes; its prototype found K07 is not durable on return |
| [`0.91.0-stage-1-addendum-the-reseal-is-not-durable-on-return-2026-11-14.md`](./0.91.0-stage-1-addendum-the-reseal-is-not-durable-on-return-2026-11-14.md) | **Dispatched 2026-11-14**: stage 1's part A2, a checkpoint inside K07 |
| [`cycle-m-and-0.91.0-2026-11-14.md`](./cycle-m-and-0.91.0-2026-11-14.md) | **Proposed 2026-11-14**: `0.91.0`'s scope. This RFC's build is planned for `0.92.0`, after the harness approval. §4 says what the harness must add: a power-loss mode |
| [`0.91.0-stage-1-two-rotation-fixes-2026-11-14.md`](./0.91.0-stage-1-two-rotation-fixes-2026-11-14.md) | **Dispatched 2026-11-14, ahead of this RFC and not its implementation**: the staged key and its renames are synced to disk, and rotation refuses an environment-supplied key. Measured in RFC 100's refresh, authorized the same day |

## Stop and return to the architect if

- a supported platform cannot provide durable create, atomic replacement,
  atomic no-replace publication, or file/directory fsync;
- a crash prefix appears to have no safe resolution;
- recovery would need to regenerate a key under the old fingerprint;
- the journal would have to contain key material to make recovery work;
- an audit event would have to assert more than the phase that committed;
- rotation would need to run online rather than offline.

Any of these is a design change. Raise it; do not work around it.

## Out of scope

External KMS/HSM integration; online or zero-downtime rotation; any
secure-erasure claim for flash or copy-on-write storage; changes to the
encryption algorithm, key derivation, or encrypted-column set; and rotation when
a key provider other than the configured file is authoritative — that path
refuses before starting.
