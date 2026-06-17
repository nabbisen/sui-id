# RFC 086 — Lightweight Formal / Model-Checking Pilot

**Status.** Proposed
**Tracks.** Strategy theme 9. Category C — pilot only; explicitly
time-boxed; outcome is a recommendation, not adoption.
**Touches.** New `verification/` directory (excluded from release
archives and the cargo workspace), possibly
`sui-id-core/src/authz.rs` and `tokens.rs` attribute annotations
behind `cfg(kani)`.

## Summary

Run three bounded, time-boxed pilots — **Kani** on two pure
security functions, **TLA+** on the refresh-token rotation
protocol, and a short **Flux** desk evaluation — and produce a
written adopt / defer / reject recommendation for each. Total
budget: ~5 working days. No pilot result changes production code
paths; the deliverable is evidence.

## Motivation

Strategy §8 (RFC 9 theme) and §6 Category C: formal techniques
may be valuable for sui-id's small pure cores, but only a pilot
can show whether they pay for their maintenance inside a project
that must stay "understandable to normal Rust developers"
(strategy §4). The audit identified unusually good pilot
substrates: `verify_pkce` and `is_redirect_uri_registered` are
small, pure, total functions; RFC 082's `authorize` is a finite
decision table; RFC 080's `RotationLookup` protocol is a textbook
concurrency state machine.

## Background

Pre-classified by the strategy: Verus — reject for now (broad
adoption); Flux — keep under consideration, no immediate
adoption; Kani / TLA+ — pilot candidates. This RFC operationalizes
exactly that classification.

## Target code areas / pilot definitions

### Pilot K (Kani, 2 days)

Harnesses (under `cfg(kani)`, in-crate):

- `authorize` (RFC 082): exhaustively prove P1–P5 of RFC 082 for
  the *entire* finite input space — deny-by-default, auditor
  write-impossibility, last-admin denial. Finite enums make this
  a complete proof, strictly stronger than the property tests.
- `verify_pkce`: prove "returns Ok ⇒ method == S256 ∧
  base64url(SHA-256(verifier)) == challenge" for bounded input
  lengths (e.g. ≤ 128 bytes), and panic-freedom. SHA-256 inside
  the harness may need stubbing per Kani practice; if stubbing
  erodes the claim's value, record that as a finding.

Evaluation criteria: wall-clock proof time (< 10 min target),
harness LOC vs property-test LOC, CI integrability
(`kani --tests` in a scheduled job), and the "could a normal
Rust developer maintain this after the author leaves" judgment.

### Pilot T (TLA+, 2 days)

Model the rotation protocol of RFC 080: variables = token rows
(active/revoked, family), in-flight exchanges; actions = begin
rotation (with the rows-affected guard), insert successor,
replay, crash between revoke-commit and insert. Check:

- **Inv1:** per family, ≤ 1 active token.
- **Inv2:** revoked is absorbing.
- **Inv3:** any replay of a revoked token leads to whole-family
  revocation (theft response) in all behaviours.
- A deliberately *guard-less* variant must violate Inv1 — the
  model demonstrating the bug class RFC 080 fixes, which doubles
  as design documentation.

Small constants (≤ 3 concurrent exchanges, ≤ 6 tokens); TLC must
exhaust the state space in minutes. Spec lives in
`verification/refresh_rotation.tla` with a README mapping model
actions to code symbols.

### Pilot F (Flux, 1 day, desk-scope)

Evaluate refinement types on `SecurityLevel` thresholds and
length-validated inputs (password min-length plumbing) on a
scratch branch only. Expected outcome per strategy lean: defer —
record concretely *why* (toolchain pinning cost, annotation
burden) or be surprised.

## Security properties / invariants

The pilots prove/check properties already specified in RFCs 080
and 082 (referenced, not duplicated). New guarantees ship only if
a pilot is adopted by follow-up RFC.

## Non-goals

- No broad Verus/Flux adoption; no proof obligations on routine
  development; no PR-gating on any prover; no verification of
  repository/DB code (Category D items stay rejected).

## Proposed design

`verification/` holds TLA+ artifacts and pilot reports; Kani
harnesses live with the code under `cfg(kani)` so they can't rot
silently if adopted. Each pilot ends with a one-page report:
effort spent, what was proven, what broke, maintenance cost
estimate, recommendation (adopt as scheduled-CI / defer with
revisit-trigger / reject with reason). The three reports plus a
summary land in `docs/` and the recommendation is decided by the
project owner — adoption itself would be a new RFC.

## Data model impact / API impact

None / none.

## Testing strategy

Not applicable in the usual sense; the pilots are themselves
verification artifacts. The guard-less TLA+ variant violating
Inv1 serves as the pilot's own sanity check, as does a seeded
bug for Kani (flip one `authorize` table arm; proof must fail).

## Migration strategy

None.

## Rollout plan

After RFCs 080 and 082 are implemented (their artifacts are the
substrate). Time-boxed: if a pilot exceeds its budget, that fact
*is* the finding — stop and write it down. No release tag
depends on this RFC; reports may ship in any docs release.

## Risks and mitigations

- *Risk:* sunk-cost adoption pressure. Mitigation: decision
  criteria written above, *before* the pilots run.
- *Risk:* toolchain weight (Kani install, TLC/Java). Mitigation:
  everything is opt-in and scheduled-CI at most; nothing enters
  the contributor's default loop.

## Acceptance criteria

- Three reports + summary delivered with explicit
  adopt/defer/reject per technique and revisit triggers.
- TLA+ spec committed with both guarded (invariants hold) and
  unguarded (Inv1 violated) results recorded.
- Main workspace toolchain, CI critical path, and contributor
  workflow unchanged.

## Open questions

- If Kani's `authorize` proof is cheap and total, should it
  *replace* the exhaustive table test or run alongside it?
  (Default: alongside — stable-toolchain tests remain the floor.)
