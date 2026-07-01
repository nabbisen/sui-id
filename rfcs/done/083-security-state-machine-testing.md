# RFC 083 — Security State-Machine Testing with proptest

**Status.** Implemented (v0.68.0)
**Tracks.** Strategy theme 6 (audit gap G6). Category B.
**Touches.** New test-only modules:
`sui-id-store/src/repos/tests_state_machine/` (or integration
test binaries per the project's `[[test]]` convention),
`sui-id-core` lifecycle tests; `proptest` is already a
workspace dev-dependency.

## Summary

Add model-based ("state-machine") property tests for the three
security lifecycles — authorization codes, refresh-token
families, and sessions — in which proptest generates random
*operation sequences*, a trivial in-memory oracle model computes
the expected state, and the real `Database`-backed implementation
must agree after every step. This is the test style that finds
ordering and interleaving bugs (the audit's G1 class) that
example-based tests structurally miss.

## Motivation

Strategy §5.1–§5.3 list lifecycle invariants ("an expired code
must not be accepted", "a revoked family must not produce new
active tokens", "logout must be durable") that hold *across
sequences of operations*, not at single call sites. v0.63.1's
125 core tests are example-based: they verify chosen sequences.
RFCs 079/080 add focused precursor properties; this RFC builds
the reusable harness and extends generated coverage to all three
lifecycles, turning the documented invariants into executable
properties — the strategy's stated expected outcome.

## Background

proptest is established in the codebase (redirect-URI comparator
properties in `authorize.rs` are a good in-repo exemplar,
including the "if anyone adds normalisation, a property fails
loudly" documentation style). `Database::open_in_memory` gives
fast per-case stores. The clock is injectable (`SharedClock`),
which the harness exploits for expiry transitions.

## Target code areas

Harness shape (one per lifecycle, sharing a tiny support module):

```rust
#[derive(Debug, Clone)]
enum RefreshOp {
    Rotate { token_ref: TokenRef },     // present a (model-tracked) token
    ReplayOld { token_ref: TokenRef },  // present a known-rotated token
    RevokeFamily { family_ref: FamilyRef },
    AdvanceClock { secs: u32 },
    PurgeExpired,
}
```

- `TokenRef`/`FamilyRef` are indices into the model's history so
  the generator can name "an old token" without knowing values.
- The oracle model is a few `Vec`s/`HashMap`s and < 100 lines —
  deliberately too simple to be wrong.
- After each op: run against real repo + model; compare observable
  outcomes (`RotationLookup` variant, active-row sets queried by
  SQL) and assert global invariants (per-family active ≤ 1;
  revoked stays revoked; expired never accepted; consumed code
  never re-consumed; revoked session never resolves).
- Sequence length 1–40, cases ~256 by default; a `PROPTEST_CASES`
  env override documented for soak runs.
- Failures persist via proptest's regression files, committed
  under the test dir per proptest convention.

Session lifecycle ops: create, revoke-one, revoke-all-except,
idle-timeout via clock, concurrent-cap insert (FIFO expiry),
resolve. Auth-code ops: issue, consume, replay, expire, purge.

Concurrency note: proptest sequences are sequential by design;
true-parallel arbitration is covered by RFC 080's dedicated
multi-task tests. This harness instead covers *orderings* —
which is where SQLite-serialized systems actually fail.

## Security properties / invariants (asserted globally)

- **Auth code:** at most one successful consume per code; no
  success after expiry; purge never resurrects.
- **Refresh:** per-family non-revoked count ≤ 1; replay of any
  rotated token yields reuse detection and an all-revoked family;
  no operation un-revokes; expired tokens never rotate.
- **Session:** revoked or expired sessions never resolve;
  revoke-all-except leaves exactly one; cap is never exceeded;
  logout durability across `purge_expired`.

## Non-goals

- Not a replacement for example-based tests or the binary crate's
  endpoint integration tests (strategy §9 explicitly warns
  against that substitution).
- No multi-process/multi-connection simulation.
- No proptest on UI/rendering.

## Proposed design

(Above.) Placement follows the project's test-layout policy:
state-machine suites live as integration-test entry points
(`tests/state_machine/main.rs` style) in the store crate, since
they exercise repo + core together but need no HTTP.

## Data model impact / API impact

None / none (test-only; possibly a few `#[cfg(test)]` helpers).

## Testing strategy

The RFC *is* test strategy; its own acceptance is mutation-style:
re-introduce two known-fixed bugs in a scratch branch — the
pre-fix `purge_expired` that deleted revoked rows, and a
predicate-less consume under a simulated reordering — and confirm
the harness catches both within default cases.

## Migration strategy

None.

## Rollout plan

Suggested v0.67.0, after RFCs 079/080 land (the harness asserts
their semantics, e.g. `RotationLookup`). CI: default case counts
on every PR (runtime budget ≤ ~30 s for the suite); a scheduled
weekly job at 10× cases.

## Risks and mitigations

- *Risk:* flaky time-dependent cases. Mitigation: all time flows
  through the injected clock; wall-clock is never read.
- *Risk:* oracle drifts from spec. Mitigation: oracle asserts
  *invariants*, not implementation equality, wherever the spec
  speaks; model-vs-real comparison is limited to observable
  outcomes.
- *Risk:* CI time creep. Mitigation: case budget in CI config,
  heavier runs scheduled.

## Acceptance criteria

- Three harnesses exist and run in CI within budget.
- Mutation check above demonstrably fails the suite.
- All documented invariants appear as named assertions
  (greppable, e.g. `INV_FAMILY_SINGLE_ACTIVE`).

## Open questions

- Whether to also model the login→MFA-pending→session creation
  funnel. Valuable but larger; propose as follow-up after the
  three core lifecycles prove the harness.
