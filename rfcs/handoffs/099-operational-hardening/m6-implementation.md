# RFC 099 M6 — release-assurance closure

**Governing RFC:** [RFC 099](../../proposed/099-operational-hardening-soak-readiness.md)

Five workstreams. They are independent apart from the evidence manifest, which
consumes all of them and therefore lands last.

## 1 — Runtime file permissions

Enforce required ownership and mode on every file the runtime creates, and
verify at startup rather than trusting directory policy.

- Database, WAL and journal files: `0600`, current owner.
- Key file: `0600`, current owner, no symlink.
- Any generated configuration or secret material: `0600`.
- Create with restrictive mode **atomically** — do not create then `chmod`,
  which leaves a window.
- At startup, verify mode and ownership; refuse to start on a permissive
  database or key file rather than silently repairing, so an operator learns
  their deployment was exposed.

**Tests:** fresh install creates `0600`; a pre-existing `0644` database refuses
startup with an actionable message; umask variations do not widen the result;
the atomic-create path is asserted, not the chmod path.

## 2 — Fuzz execution

RFC 084 requires all six targets to run on the agreed schedule and a build on
`fuzz/` changes.

- Build **all six** targets in the PR-triggered job; confirm the job is actually
  reachable for that event.
- Run all six on the scheduled and manual lanes — not only `accept_language` and
  `ids_fromstr`.
- Fix the corpus and duration per target so a run is deterministic and its
  evidence comparable between runs.
- Upload artifacts on failure, as the workflow already does.

**Evidence:** per target, the exact command, duration, seed/corpus identity,
exit status, and any artifact digest.

## 3 — Packaging

Replace the manual tar with an automated, inspectable step.

- Build the release archive reproducibly from one clean commit.
- Archive layout per the project rule: files at the archive root, **no**
  intermediate parent directory; version appended to the archive name.
- Inspect the produced archive against an expected-contents manifest — assert
  both inclusion and **exclusion** (no `.git`, no `target/`, no local
  `sui-id.toml`, `sui-id.key`, or `sui-id.sqlite`, no `.git-exclude/`).
- Record the artifact digest.

**Tests:** a deliberately added stray file fails inspection; a missing required
file fails inspection; two builds of the same commit produce the same digest.

## 4 — Live integration

Representative, not exhaustive.

- **LDAP:** against a controlled directory, exercise bind, shadow-user upsert,
  and an outage/recovery cycle. No real production credentials.
- **Upstream OIDC:** against a representative provider, exercise discovery,
  JWKS retrieval, a full authorization-code login, and a JWKS rotation. This
  depends on RFC 096-A being Implemented; if it is not, M6 cannot close.

**Evidence:** provider identity (sanitized), exact flows exercised, observed
outcomes, and any deviation from the designed behaviour.

## 5 — Evidence manifest

The manifest is the deliverable that makes soak meaningful. It binds:

- the exact commit SHA and a dirty-tree assertion;
- the release artifact digest;
- a sanitized configuration and environment manifest — no secrets, but enough
  to reproduce the deployment class;
- the full Gate Matrix v1 result set from **one clean commit** (G01–G12);
- fuzz results per target;
- packaging inspection result;
- live integration results;
- the open-defect list, which must contain zero blocker or high-severity items.

**Mixed-commit evidence is the primary threat this manifest exists to prevent.**
Results from different trees may not be assembled. If one lane needs re-running,
re-run the matrix.

## Closure

M6 closes when every workstream above has observed evidence on one clean commit,
the manifest is complete, no blocker or high-severity defect remains, and
independent review approves **entry into soak only**. M6 closure is not a
production-readiness statement and carries no version tag.
