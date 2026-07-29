# RFC 099 developer and operator handoff

**Governing RFC:** [RFC 099](../../proposed/099-operational-hardening-soak-readiness.md) — Proposed
**Milestone:** M6 — release-assurance closure and soak entry; then M7 soak
**Audience:** implementer and operator/tester
**Status:** Planning companion; inherits the governing RFC's current status —
Proposed. Implementation blocked until Accepted.

## Entry gate

RFCs 097 and 098 Implemented; this RFC Accepted; operator/tester handoff and
evidence schema independently approved; and **RFC 100 Implemented** — the M7
workload requires a master-key rotation exercise, and rotating a key that is not
crash-safe against the immutable soak artifact risks unrecoverable divergence.

If the owner defers RFC 100 instead, the rotation exercise must be struck from
the M7 workload by explicit recorded decision, with residual risk carried into
RFC 097. It may not be silently skipped as "not applicable".

## What M6 produces

One **immutable artifact** plus a sanitized configuration/environment manifest,
and the evidence that the artifact is fit to enter soak. M6 does not declare
production readiness, and passing it authorizes only entry into M7.

## Documents

| Document | Contents |
|---|---|
| [`m6-implementation.md`](./m6-implementation.md) | Runtime file modes, fuzz execution, packaging, live integration, evidence manifest |
| [`m7-soak-operations.md`](./m7-soak-operations.md) | Workload contract, exercises, defect handling, pause and reset rules |

## Known inputs already measured

- **Runtime file permissions.** The application creates the SQLite database
  through `rusqlite` without enforcing mode `0600`; an observed local file was
  `0644`. Production guidance creates `/var/lib/sui-id` as `0750`, which
  mitigates world access, but the quick-start path can leave plaintext PII,
  audit events and password hashes readable to other local users.
- **Fuzz coverage is partially live.** Six targets exist. The scheduled workflow
  runs only `accept_language` and `ids_fromstr`; the four higher-value
  auth/OIDC targets are neither built nor run there. The `fuzz-build` job is
  conditional on `pull_request` — confirm that trigger reaches it after the
  Wave 2 pin-only changes.
- **Release packaging is a manual tar operation**, which risks accidental
  inclusion or exclusion.

## Stop and return to the architect if

- an artifact cannot be made byte-reproducible and the manifest would have to
  record something weaker;
- a required live integration cannot be run without a real credential in the
  repository or CI;
- a fuzz target cannot run to the agreed duration deterministically;
- soak would have to begin against anything other than the exact M6 artifact.
