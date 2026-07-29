# RFC 099 M7 — soak operations

**Governing RFC:** [RFC 099](../../proposed/099-operational-hardening-soak-readiness.md)
**Audience:** operator/tester running the soak

RFC 099 must turn the ROADMAP's minimum contract below into exact event counts,
traffic volumes, thresholds, commands and evidence formats **before M6 closes**.
This document is the checklist that contract must satisfy; it is not itself the
contract.

## The baseline is immutable

Soak runs against **one M6 artifact digest** plus its sanitized
configuration/environment manifest. Changing that baseline requires review and
restarts the affected window. Never patch the running artifact.

## Required traffic

The environment must sustain representative **successful and rejected** traffic
across: login, MFA, authorization-code flow, refresh rotation **and reuse**,
session lifecycle, dynamic registration, and federation.

Rejected traffic matters as much as successful — most of the remediation
programme hardened denial paths, and a soak that only exercises happy paths
evidences nothing about them.

**Quiet or unavailable periods pause elapsed soak time.** Track wall-clock and
exercised-time separately; only exercised time counts toward the four weeks.

## Required exercises

At least one successful instance of each, where applicable:

| Exercise | Notes |
|---|---|
| Backup and restore | Restore must be proven, not just backup |
| Signing-key rotation | RFC 094 `K01` |
| **Master-key rotation** | Requires RFC 100 Implemented — see the entry gate |
| Restart and upgrade | Including a migration if one is pending |
| LDAP outage and recovery | |
| Upstream JWKS rotation and failure | |
| Registration-concurrency exercise | Contended limited-use token |
| Dependency-alert handling drill | |
| Incident-response drill | |
| Rollback drill | |

## Defect handling and reset

- Exit requires **zero unresolved blocker or high-severity defects**, and
  authentication/security error rates within the thresholds RFC 099 fixes.
- A security-sensitive behaviour change, data migration, authn/authz change, or
  a fix for a blocker/high-severity defect **restarts the relevant soak window**.
- The owner and independent reviewer decide together whether a narrower change
  resets a targeted exercise or the complete four-week window. That decision is
  recorded, not assumed.

## What passing means

**Calendar completion never passes M7.** At least four *meaningfully exercised*
weeks and every workload criterion must pass, and the owner and independent
reviewer must accept the evidence.

The earliest outcome is a **release-readiness discussion** — not production
approval, not a version tag, and not a "security-reviewed" designation. Nothing
in this milestone grants those automatically.

## Evidence to keep throughout

- The artifact digest and configuration manifest in force for each window.
- A dated log of exercised versus paused time, with the reason for each pause.
- Per-exercise records: what was run, when, observed outcome, any deviation.
- The running defect list with severity, discovery date, and resolution or
  reset consequence.
