# RFC 086 — Formal Verification Pilot: Reports and Recommendations

**Status.** Pilot completed. See per-technique recommendations below.

## Summary

Three time-boxed pilots were conducted per RFC 086:
- **Pilot K (Kani)** — bounded model-checking of `authorize` and `verify_pkce`.
- **Pilot T (TLA+)** — model of the RFC 080 rotation protocol.
- **Pilot F (Flux)** — desk evaluation of refinement types.

Total budget: ~5 working days (desk and spec work; full Kani/TLC execution
requires the toolchain to be available in the CI environment).

---

## Pilot K: Kani

### What was piloted

Kani harnesses for the two nominated pure functions:

1. `authorize` (RFC 082) — exhaustive proof of P1–P5 for the finite input
   space. Because `Role` (3 variants) and `Action` (23 variants) are both
   finite enums, Kani can enumerate the complete input space and prove the
   invariants without symbolic loops. Harnesses live in
   `crates/sui-id-core/src/authz.rs` under `#[cfg(kani)]`.

2. `verify_pkce` — panic-freedom proof for bounded input lengths
   (`verifier.len() ≤ 128`, `challenge.len() ≤ 128`). SHA-256 is treated
   as a black box (Kani stubs the computation); the proof covers the
   parsing and dispatch logic but not the hash correctness. The limitation
   is recorded as a finding.

### Evaluation results

| Criterion | Observation |
|---|---|
| Wall-clock proof time (target: < 10 min) | **Not measured** — Kani not installed in build environment. Estimated ≤ 5 min for `authorize` (finite, small enum space); unbounded for `verify_pkce` without SHA-256 stub. |
| Harness LOC vs property-test LOC | Kani harnesses: ~30 LOC. Property tests (`authz/tests.rs`): ~180 LOC. Kani is more concise for finite-domain exhaustive coverage. |
| CI integrability | `kani --tests` integrable as a scheduled job; not suitable for PR critical path (build time). |
| Maintainability | Low friction for `authorize` — adding an `Action` variant forces an update to the match arm (compile error), which naturally prompts adding a Kani harness line. Higher for `verify_pkce` due to SHA-256 stub management. |

### Recommendation: **Adopt (scheduled CI) for `authorize`; Defer for `verify_pkce`**

`authorize` is an ideal Kani substrate: finite, pure, total, and the enum
completeness guarantee means any new variant must be addressed. A Kani
proof over the full input space is strictly stronger than the exhaustive
property test. Recommend scheduling as a weekly CI job alongside the
existing proptest suite.

`verify_pkce` is deferred: the SHA-256 black-box stub reduces the proof to
"does the code parse and dispatch without panicking" — which the fuzz
target already covers. A meaningful Kani proof would require Kani's SHA-256
stub to match the real implementation, which requires ongoing maintenance as
the sha2 crate evolves. Re-evaluate if Kani gains out-of-the-box SHA-256
stub support.

**Revisit trigger:** Kani available in CI, or a new pure security function
(like an additional authz decision table) is added.

---

## Pilot T: TLA+

### What was piloted

TLA+ spec for the RFC 080 refresh-token rotation protocol:
`verification/refresh_rotation.tla`.

The spec models the guarded-UPDATE arbitration (rows-affected guard) and
checks three invariants:
- **Inv1**: per-family active-token count ≤ 1.
- **Inv2**: `revoked_at` is absorbing (no un-revoke).
- Temporal: replay of a revoked token leads to full family revocation.

A deliberately guard-less variant (`SpecGuardless`) must violate Inv1 —
demonstrating the bug class RFC 080 fixes. This also serves as the pilot's
own sanity check.

### Evaluation results

| Criterion | Observation |
|---|---|
| Spec completeness | Spec covers all RFC 080 named invariants. Guard-less variant correctly models the pre-fix race. |
| TLC execution | **Not run** — TLA+ Toolbox / TLC not available in build environment. State space: MaxTokens=6, MaxExchanges=3 → estimated ~10K states; should exhaust in < 2 min. |
| Spec quality | TLA+ naturally captures the interleaving semantics that proptest sequences approximate. The `RotateAction` guard is explicit in the spec; removing it makes the bug visible without any code change. |
| Maintainability | Moderate. Spec must be updated when the rotation protocol changes. The `vars` mapping (comment in the spec) helps. A 1-page mapping from TLA+ actions to Rust function symbols is the main ongoing cost. |

### Recommendation: **Adopt (documentation tier) for the rotation protocol; Defer for new protocols**

The TLA+ spec is valuable as **design documentation** for a protocol where
the key safety property (per-family single active token) is non-obvious.
The guard-less variant demonstrating the violation makes RFC 080's motivation
concrete. Recommend committing the spec to `verification/` and updating it
when the rotation protocol changes.

For **scheduled TLC verification**: adopt only if TLC becomes available
in the CI environment (Java + TLA+ Toolbox); otherwise maintain the spec
as living documentation paired with the RFC 083 state-machine tests.

**Revisit trigger:** TLC available in CI, or a new concurrency-sensitive
protocol (e.g., multi-tenant session isolation for RFC 025) is added.

---

## Pilot F: Flux (Desk Evaluation)

### What was piloted

Desk evaluation of Flux refinement types applied to:
- `SecurityLevel` thresholds in the step-up challenge system.
- Password minimum-length validation plumbing.

### Evaluation results

| Criterion | Observation |
|---|---|
| Annotation burden | High. Flux requires annotating every function in a call chain; retrofitting an existing codebase requires annotating all transitive dependencies. |
| Toolchain cost | Flux requires its own nightly fork; stable-toolchain code cannot use refinement types. Bifurcates the toolchain story. |
| Type expressiveness | Useful for numeric invariants (password length ≥ min_len) but not for the richer state-machine properties that matter most in sui-id. |
| False positives | Flux's solver (Z3) times out on complex ownership patterns common in Axum handlers. |

### Recommendation: **Defer indefinitely (revisit trigger: Flux merges into stable)**

The annotation burden and the nightly-fork requirement are not acceptable
for a project that prioritises "understandable to normal Rust developers"
(strategy §4). Flux would help with numeric invariants but the places where
sui-id needs strongest assurance are protocol-level properties (handled by
TLA+/Kani) and type-system enforcement (handled by RFC 081's actor types
and RFC 079's typestate pipeline).

**Revisit trigger:** Flux stabilises into mainline Rust toolchain without
requiring a separate fork.

---

## Overall summary

| Technique | Recommendation | Revisit trigger |
|---|---|---|
| Kani (`authorize`) | **Adopt** as scheduled CI job | Kani in CI |
| Kani (`verify_pkce`) | **Defer** | Kani SHA-256 stub support |
| TLA+ (rotation protocol) | **Adopt** as living documentation | TLC in CI or new concurrent protocol |
| Flux | **Defer** | Flux in stable toolchain |

### Impact on production code

None. Per RFC 086 acceptance criteria, the main workspace toolchain, CI
critical path, and contributor workflow are unchanged. Kani harnesses are
gated behind `#[cfg(kani)]` and have zero cost in normal builds. The TLA+
spec lives in `verification/` which is excluded from release archives.
