# RFC 099 — Operational Hardening and Soak Readiness

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** RFCs 097 and 098 Accepted; M7 minimum workload/reset contract approved in ROADMAP.
**Implementation prerequisites.** RFCs 097 and 098 Implemented; this RFC Accepted; operator/tester handoff and evidence schema independently approved.
**Closure prerequisites.** Full clean-tree matrix, runtime mode checks, every fuzz target, package inspection, representative live LDAP/federation integration, immutable artifact/configuration manifest, zero blocker/high defects, and independent approval for soak entry. M7 workload and reset rules then pass separately.
**Tracks.** ROADMAP M6–M7 — Release-assurance closure and real-environment soak.
**Touches.** Runtime file creation, fuzz harnesses, package/release automation, integration fixtures, operational procedures, evidence manifests and soak records.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** `codex-project-architect` (OpenAI Codex).
**Implementation owner.** `codex-developer` (OpenAI Codex), after acceptance and prerequisites.
**Independent security and soak/closure reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex).

## Summary

Harden runtime file permissions and assemble current, reproducible release
evidence for one immutable artifact/configuration baseline, then define exact
workload, exercise, defect, pause, and reset rules for at least four meaningfully
exercised soak weeks. Passing permits a readiness discussion only; it does not
declare production readiness or assign a v1 date.

## Requirements

- Define required ownership/modes and atomic creation/replacement for database,
  master/signing keys, secrets, backups, temporary files, logs, sockets, and
  configuration; fail safely on insecure existing state.
- Enumerate and execute every fuzz target with fixed minimum runs/time, corpus
  identity, sanitizer/tool versions, artifact retention, and triage rules.
- Build and inspect packages reproducibly; record source commit, artifact
  digest, SBOM/dependency audit, contents, permissions, install/upgrade/remove,
  and smoke results.
- Exercise representative live LDAP and upstream OIDC integrations without
  committing real credentials; record sanitized environment characteristics.
- Bind all M6 evidence to one clean commit, artifact digest, exact tool versions,
  timestamps, commands, exit status, configuration class, and reviewers.
- Translate every ROADMAP M7 minimum into exact event counts, traffic volumes,
  error-rate thresholds, exercises, evidence fields, and pause/reset decisions.
- Every “not applicable” exercise needs a written reason, compensating evidence,
  and independent reviewer approval. An unchecked N/A never passes.
- Quiet/unavailable time does not count. Blocker/high defects remain zero. A
  security behavior/data/auth change or blocker/high fix resets the applicable
  window under a recorded owner/reviewer decision.
- No production-ready, security-reviewed, v1, rc, beta, tag, or automatic
  release claim follows from calendar passage or M6/M7 completion alone.

## Planned design work

Before acceptance, provide the exact file-mode matrix, fuzz inventory and
budgets, package inspection commands, live-integration protocols, evidence JSON
schema, immutable manifest format, M7 traffic/event counts, thresholds, exercise
runbooks, pause/reset decision tree, defect taxonomy, and required multi-file
operator/tester handoff.

## Security considerations

Evidence substitution and quiet-soak inflation are primary threats. Artifact
digests, clean-tree binding, sanitized configuration identity, exact workload
counters, signed/independently reviewed decisions, and reset rules prevent an
unexercised or changed artifact from inheriting confidence. Evidence never
contains real secrets.

## Open questions

Exact volumes and thresholds require the implemented system’s post-M5 baseline
and are deliberately deferred to detailed design; ROADMAP minimum exercises
cannot be removed to meet a date.
