# RFC 093 implementation-start authorization

## Summary

On 2026-07-21 (Asia/Tokyo), `@nabbisen` explicitly authorized
`codex-developer` (OpenAI Codex) to begin RFC 093 implementation preparation on
clean baseline commit `959f089983ce51e53ca403a422a1fe308c276036`.

The authorization was given in direct response to a request that named the
implementer and baseline and required confirmation that no competing change
owned RFC 093's files. It therefore confirms all three authorization facts.
This record does not state that implementation or any RFC 093 gate has passed.

## Scope followed

The authorized design and evidence contract is:

- governing design: [`RFC 093`](../../accepted/093-build-toolchain-release-gates.md);
- independent design review: [`093-design-review-2026-07-17.md`](../../reviews/093-design-review-2026-07-17.md);
- implementation-start review: [`093-implementation-start-authorization-review-2026-07-21.md`](../../reviews/093-implementation-start-authorization-review-2026-07-21.md);
- roadmap milestone: M1 — Trustworthy build baseline in [`ROADMAP.md`](../../../ROADMAP.md); and
- implementation owner: `codex-developer` (OpenAI Codex).

The exclusive authorized touch set is RFC 093's declared metadata scope plus
the following owner-approved interpretation of its normative body:

- root `Cargo.toml`, workspace crate manifests, and `Cargo.lock`;
- `.github/workflows/`;
- `scripts/` and the RFC 093 `ci/` gate/policy manifests;
- `docs/book.toml` and narrow documentation-link inputs required by G10;
- `rfcs/README.md`, RFC lifecycle metadata/links, and RFC 093 evidence/handoff
  files; and
- the local LDAP smoke fixture/test required by G09.

On 2026-07-21, `@nabbisen` explicitly ruled that the normative G09–G12 design
places the required `ci/` policy/input manifests and
`crates/sui-id-store/tests/ldap_smoke.rs` inside RFC 093 implementation scope,
even though the header's abbreviated `Touches` field does not enumerate them.

For G10 mechanical debt, M1 may correct link targets, anchors, path spelling,
case, and moved-file references in any document scanned by the gate. M1 may not
change a feature claim, scope statement, security-posture wording, or other
semantic documentation content; those remain RFC 098/M5 authority.

No competing change owns those files at authorization time. Work outside this
set, product-feature changes, RFC 094 structural audit implementation, RFC 095/
096 behavior, broad documentation reconciliation, fuzz closure, packaging, or
release/production claims require separate authority.

## Files changed

This authorization package changes only:

- `rfcs/accepted/093-build-toolchain-release-gates.md`; and
- `rfcs/handoffs/093-build-toolchain-release-gates/implementation-start-authorization.md`; and
- `rfcs/reviews/093-implementation-start-authorization-review-2026-07-21.md`.

No implementation source, workflow, manifest, lockfile, test, or generated
gate artifact is changed by this authorization record.

## Design decisions and assumptions

- The exact implementation input is commit
  `959f089983ce51e53ca403a422a1fe308c276036` (`Accept RFC 096 federation
  validation design`). The worktree was observed clean before this record was
  authored.
- Authorization names `codex-developer`; it does not delegate architecture
  approval, closure approval, commit/tag/push authority, or permission to
  weaken a blocking lane.
- Gate Matrix v1, G01–G12 commands, negative self-tests, tool/input versions,
  and diagnostic-only audit wording remain normative exactly as RFC 093 states.
- Separate reviewable implementation commits are allowed, but M1 evidence must
  ultimately bind all mandatory lanes to one clean commit.
- Any shared-file ownership conflict or design invariant conflict pauses work
  and returns to the owner/architect before modification continues.
- Recording prerequisite satisfaction and removing a redundant review clause
  does not amend RFC 093's accepted design prerequisite; the focused review
  obligation remains in this handoff and its tracked review record.
- The owner-approved `ci/`, G09 test, and mechanical-document interpretations
  are bounded readings of already normative G09–G12 requirements, not authority
  to expand product behavior or RFC 098 semantic documentation scope.

## Tests and gates run

Observed for this authorization package:

- `git rev-parse HEAD` returned
  `959f089983ce51e53ca403a422a1fe308c276036`;
- `git status --short` produced no output before authoring, establishing the
  clean authorization baseline; and
- the Accepted RFC, durable design review, and roadmap were read to reconcile
  the owner, scope, and prerequisites.

No G01–G12 command, build, test, lint, LDAP smoke, mdBook build, link check,
RFC-integrity check, UI-invariant check, packaging task, or runtime check was
run for this authorization-only record. Those missing results do not block
authorization, but they block M1 closure and every claim that the gate contract
has been implemented successfully.

## Generated artifacts

None. This package creates tracked authorization/review Markdown records and
synchronizes RFC metadata. It does not generate CI evidence, binaries,
documentation output, manifests, packages, logs, credentials, or test fixtures.

## Known limitations

- The authorization becomes repository-visible only after this record, its
  tracked review, and the synchronized RFC wording are committed together by
  `@nabbisen`; a signed commit is preferred for attribution.
- Baseline cleanliness does not prove current compilation or gate health.
- RFC 093 is not Implemented; RFCs 094–096 implementation prerequisites that
  depend on it remain unsatisfied.
- The legacy audit literal check remains diagnostic-only and cannot establish
  completeness or transaction atomicity.
- Stable Rust and Ubuntu package resolution remain moving inputs whose exact
  versions must be captured in later evidence.

## Recommended next step

Obtain focused amendment review of this authorization package and commit all
three files atomically by the owner. Only then may `codex-developer` inventory
the exact RFC 093 implementation file list, confirm ownership still has no
conflict, and begin
the first small implementation wave. Each implementation review request must
reference RFC 093, its design review, this authorization, the exact baseline/
diff, applicable Gate Matrix rows, observed results, and evidence paths so a
reviewer who did not perform the design review can reconstruct the contract.
