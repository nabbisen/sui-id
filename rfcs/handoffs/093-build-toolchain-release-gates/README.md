# RFC 093 developer handoff

**Governing RFC:** [RFC 093](../../done/093-build-toolchain-release-gates.md) — Accepted, amended 2026-07-28
**Audience:** `codex-developer` (Lane A) and the second implementer (Lane B)
**Status:** Planning companion. It translates RFC 093 into ordered implementation
stages. It does not approve design, authorize coding beyond the recorded
authorization, or override the RFC.
**Authorization:** [`implementation-start-authorization.md`](./implementation-start-authorization.md),
reviewed at [`093-implementation-start-authorization-review-2026-07-21.md`](093-implementation-start-authorization-review-2026-07-21.md)

## Stage map

RFC 093 is delivered in two milestones plus one preparatory refactor. They share
no files and can run concurrently in separate lanes.

| Stage | Contents | Document |
|---|---|---|
| **M1a** | MSRV floor, corrective fixes, clippy, gate lanes G01–G09, manifest enforcement | [`m1a-implementation.md`](./m1a-implementation.md) |
| **M1b** | Gate lanes G10a/G10b/G11, `ci/rfc-policy.toml`, documentation and lifecycle debt | [`m1b-implementation.md`](./m1b-implementation.md) |
| **prep** | `handlers/federation.rs` split — unblocks Lane B, owned by neither lane | [`m1a-implementation.md`](./m1a-implementation.md) §Theme B |
| all | Evidence contract every package must satisfy | [`verification.md`](./verification.md) |

Programme-wide ordering is in [`ROADMAP.md`](../../../ROADMAP.md) §Execution order.

## What is already done

G12 (UI invariants) is implemented and locally green: `ci/ui-invariants.toml`,
`scripts/check-ui-invariants.sh`, and 19 negative fixtures, committed across
`d83ffdd` and `50bd4a7`. Its CI job `ui-invariants-v1` and the SHA-pinned action
manifest landed in `7055894`. **No hosted run has been observed yet**, so G12 is
implemented but not evidenced.

Eleven of twelve lanes remain. G05 (stable build/test, default features) and G08
(`cargo fmt`) pass today but are not wired.

## Stop and return to the architect if

- a Gate Matrix command cannot be executed as written;
- a lane needs a negative fixture that cannot be made to fail deterministically;
- a lint fix would change runtime behaviour;
- repairing a documentation link would change a claim rather than a path;
- an RFC's status is genuinely unknown and would have to be guessed;
- the MSRV floor turns out to differ from the recorded 1.95 on your toolchain;
- any change would touch product source outside the declared touch set.

Design questions are settled upstream of implementation. Raise them; do not
decide them.

## Boundary

In scope: build, toolchain, CI, gate scripts and fixtures, `ci/` manifests,
`docs/book.toml`, RFC lifecycle metadata and links, and the G09 LDAP smoke test.

Out of scope: product behaviour, RFC 094 structural audit, RFC 095/096 runtime
behaviour, RFC 098 semantic documentation reconciliation, RFC 099 fuzz and
packaging, and any release or security-readiness claim.

## Review records

Dated review records for RFC 093, relocated here on 2026-09-10 when
`rfcs/reviews/` was retired: RFC 000 sanctions four lifecycle folders plus
the optional `draft/` and this non-lifecycle `handoffs/` companion folder,
and per-RFC review records are companion documents, not a sixth lifecycle.

| Record | What it is |
|---|---|
| [`093-design-review-2026-07-17.md`](093-design-review-2026-07-17.md) | Independent design review — RFC 093's **Independent design review** evidence |
| [`093-implementation-start-authorization-review-2026-07-21.md`](093-implementation-start-authorization-review-2026-07-21.md) | Review of the implementation-start authorization |
| [`093-closure-review-request-2026-08-03.md`](093-closure-review-request-2026-08-03.md) | Closure review request |
| [`093-closure-review-2026-08-12.md`](093-closure-review-2026-08-12.md) | First closure review — superseded by the 2026-08-27 record |
| [`093-closure-review-2026-08-27.md`](093-closure-review-2026-08-27.md) | Closure review — RFC 093's **Closure evidence** |

These are historical records. They are not edited to reflect later
decisions; only their link paths were rewritten by the relocation.
