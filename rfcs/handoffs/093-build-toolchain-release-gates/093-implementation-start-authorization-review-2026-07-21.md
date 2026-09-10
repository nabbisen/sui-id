# RFC 093 implementation-start authorization review record

**Review date.** 2026-07-21 (Asia/Tokyo)
**RFC.** RFC 093 — Build, Toolchain, and Release-Gate Contract
**Verdict.** Accept with notes; implementation start GO after the complete authorization package is owner-committed
**Independent reviewer.** Independent architecture and security reviewer (external vendor; not the design reviewer)
**Accountable project owner.** `@nabbisen`
**Authorized implementer.** `codex-developer` (OpenAI Codex)
**Authorized clean baseline.** `959f089983ce51e53ca403a422a1fe308c276036`

## Reviewed authority and evidence

The reviewer independently verified the baseline, clean starting tree,
candidate hashes, absence of competing changes/worktrees/stashes, governing RFC
and design-review references, minimal diff, handoff structure, and the explicit
absence of G01–G12 results. The review found the record honest about start
authorization versus closure evidence.

The handoff records the owner authorization as `Good. Authorized. Proceed.`,
given in direct response to a request naming the implementer, the exact clean
baseline, and the required non-competing ownership confirmation, and records
further owner rulings on the bounded scope interpretations below. Those
utterances and their preceding context are outside the repository; the
reviewer verified neither and relies on the owner's personal commit of this
package as the repository-visible attribution.

Repository attribution remains completed by the owner committing the full
authorization package personally; a signed commit is preferred. This record
does not claim cryptographic attribution before that commit exists.

## Finding disposition

### F1 — `ci/` and G09 test target: owner-resolved

The RFC header abbreviates the touch set, while its normative body mandates
`ci/gate-inputs.toml`, `ci/rfc-policy.toml`, `ci/ui-invariants.toml`, and the G09
LDAP test target. The owner ruled that required `ci/` manifests and
`crates/sui-id-store/tests/ldap_smoke.rs` are inside RFC 093 scope. This is a
bounded interpretation of G09–G12, not open-ended test or CI ownership.

### F2 — RFC 098 document boundary: owner-resolved

M1 may mechanically repair link/anchor/path/case/moved-file failures in any
G10-scanned document. It may not alter semantic claims, feature descriptions,
scope statements, or security-posture wording; those remain RFC 098/M5.

### F3 — Accepted-RFC prerequisite wording: resolved

The candidate removes the added `and its focused review passes` clause from the
Accepted RFC metadata. Review remains required by the handoff and this durable
record, so the package records prerequisite satisfaction without introducing a
new material RFC prerequisite or triggering RFC 000 return-to-Proposed.

### F4 — Owner attribution: commit-time condition

The owner must commit the RFC, handoff, and this tracked review together.
Personal owner authorship is the repository-visible attribution; signing is
preferred but not represented as already observed.

### F5 — Review visibility: resolved

This tracked record replaces reliance on ignored `.git-exclude/` review
artifacts and is directly referenced by the implementation authorization.

## Authorization decision

RFC 093 implementation start is GO only after the synchronized three-file
package is committed by the owner. This authorizes bounded implementation work
against the named baseline; it does not establish any G01–G12 result, M1
closure, RFC 094–096 implementation authority, release readiness, or security-
reviewed status.

Every implementation review request must reference the governing RFC, design
review, authorization handoff, this review record, exact baseline/diff,
applicable Gate Matrix rows, observed results, and durable evidence paths.

## Standing limitations

- Stable clippy, all-features LDAP, and mdBook failures remain open inputs to
  M1 and are not re-credited by this record.
- The legacy audit literal check remains diagnostic-only and cannot prove
  structural completeness or transaction atomicity.
- A stale ignored preparation snapshot is not repository evidence and must not
  be used as current lifecycle or gate state.
- M1 closure and dependent RFC implementation remain NO-GO until their own
  repository-visible gates pass.
