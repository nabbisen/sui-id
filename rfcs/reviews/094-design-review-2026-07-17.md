# RFC 094 independent design-review approval

**Review date.** 2026-07-17 (Asia/Tokyo)  
**RFC.** RFC 094 — Transactional Audit Completeness and Typed Event Registry  
**Verdict.** Accept with notes  
**Independent reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex)  
**Accountable project owner.** `@nabbisen`  
**RFC author / architect.** `codex-project-architect` (OpenAI Codex)  
**Implementation owner.** `codex-developer` (OpenAI Codex)

## Reviewed artifact identity

The approved RFC and companion handoff bytes are preserved by proposal commit
`ad9bb6dd67bf4aaa00e45b4b1526662670d7cdcb`.

| Reviewed path at the proposal commit | SHA-256 |
|---|---|
| `rfcs/proposed/094-transactional-audit-registry.md` | `ecab63d374351a527bb75dee03dd0539cb166b945b64668b2b2fc0d94a567faa` |
| `rfcs/handoffs/094-transactional-audit/README.md` | `0204f057fb3890d093595eedfe91939b09c8903bd24baaaf0e22977b161af91d` |
| `rfcs/handoffs/094-transactional-audit/architecture.md` | `7e1ee70885ed3f61aabb0f7f3d484c2492332e57d13447a229bb1665ad1095f8` |
| `rfcs/handoffs/094-transactional-audit/command-inventory.md` | `83e622380115f6c1828d3c26c9d7f9505f4b819d402dcdffa2d03ab48750196b` |
| `rfcs/handoffs/094-transactional-audit/migration-checklist.md` | `202f4940074433e7ea035b4fc58c9246a30fcb5733c241426c11862687570d21` |
| `rfcs/handoffs/094-transactional-audit/verification.md` | `9472678d726b519370246a3baa96caf10efe499af4bf4f66b506491b439a34f9` |

The lifecycle transition changes only RFC 094's header metadata and location,
the RFC index, and the companion lifecycle labels and links that identify the
governing RFC's new state. It does not amend the approved requirements,
transaction boundaries, inventory rows, threat decisions, recovery protocol,
or implementation instructions.

## Decision and findings disposition

The independent review approved RFC 094 after three focused amendment rounds.
The final design:

- restricts query wrappers and seals raw write authority so prepared statements,
  writable pragmas, helper indirection, and omitted write sites cannot bypass
  the reviewed executor;
- represents conditional Class-A outcomes and command-specific event sums
  without permitting arbitrary, unmapped, duplicate, or cross-command event
  descriptors;
- derives actor, command, correlation, and time authority from sealed context
  rather than caller-supplied payload fields;
- freezes an independently reconciled 87-row command inventory with classes
  A=60, P=14, I=6, O=5, and X=2, including distinct T04 rotation/reuse and T09
  initial-issuance boundaries;
- binds every committed T04 mutation branch to its correct typed audit event in
  one Class-A transaction; and
- defines authenticated, atomic master-key preparation, publication, recovery,
  fault injection, and old-key disposition across every crash prefix.

The reviewer explicitly accepted the P/O/I/X classifications and their threat
rationales. Protocol or operational telemetry is not represented as equivalent
to the Class-A tamper-evident audit chain.

No blocking design finding remains. The final proposal already contains the
review-requested `CommandSpec: Sized` correction and names unmapped and duplicate
event-variant fixtures in the verification companion.

## Accepted non-blocking notes

Implementation and evidence must preserve these review notes:

- use zeroizing destruction, not ordinary `Drop`, for unused successor secrets;
- perform a query-only preliminary refresh-row read before preparing successor
  material, then re-read and validate authoritative state inside T04;
- keep the generated event-sum fixtures synchronized across the RFC, migration,
  and verification documents; and
- treat the shown Rust surface as normative behavior while allowing
  implementation-equivalent type details that compile literally.

These notes clarify implementation hygiene and evidence; they do not reopen the
approved transaction, capability, event, attribution, inventory, or recovery
design.

## Independence attestation

The named independent reviewer performed the design-review execution and did
not author RFC 094 or act as its implementation agent. The role separation is
process separation within OpenAI Codex; organizational or vendor independence
is not claimed.

## Owner authorization and implementation boundary

On 2026-07-17, after the independent design approval and after RFC 093 entered
Accepted state in commit `ecf6b1c070f6503dfec0cfea3a915e9081546054`,
`@nabbisen` explicitly authorized RFC 094's atomic Accepted transition.

This authorization does not authorize implementation. RFC 094 implementation
remains prohibited until RFC 093 is Implemented with its clean-tree matrix
passing, the Class-A inventory and threat delta have repository-visible
approval evidence, and every handoff entry gate is satisfied. RFC 095 and later
RFCs remain Proposed.

## Observed review checks

The independent review reconciled 87 unique inventory identifiers and examined
the RFC, architecture, inventory, migration, and verification documents,
relevant current write sites, transaction boundaries, negative-fixture design,
refresh-token flows, and master-key recovery prefixes. It also compiled the
illustrative type surface during review, which led to the `Sized` correction
already present in the bound proposal.

This is a design decision record. It does not claim that RFC 093 is Implemented
or that any RFC 094 build, test, clippy, audit, fault-injection, concurrency,
LDAP, mdBook, package, integration, or runtime gate passed.
