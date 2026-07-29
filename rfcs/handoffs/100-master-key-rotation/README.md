# RFC 100 developer handoff

**Governing RFC:** [RFC 100](../../proposed/100-master-key-rotation-recovery.md) — Proposed
**Milestone:** M2c — master-key rotation recovery
**Audience:** implementer assigned after RFC 100 is Accepted
**Status:** Planning companion; inherits the governing RFC's current status —
Proposed, pending independent design review and acceptance. Implementation is
blocked until then.

## Entry gate

All of:

1. RFC 100 Accepted after independent design review — **by a reviewer who did
   not author it**; the RFC records that its author should not be its security
   reviewer.
2. RFC 094 **M2a** Implemented. `K04` and `K05` are Class-A database commands and
   consume that transaction seam; they cannot be built before it exists.
3. Crash-injection harness approved.

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
