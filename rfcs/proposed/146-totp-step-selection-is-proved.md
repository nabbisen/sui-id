# RFC 146 — TOTP step selection is proved, not only tested

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted, and M7 (the real-environment soak) complete. The build waits for the soak, with no feature-freeze exception: `@nabbisen`, 2026-11-14, on the [scoping note](../../verification/verus-scoping-note-2026-11-14.md)'s §6 recommendation: *"Authorized."*
**Closure prerequisites.** `totp::verify` decides its step only through `select_step`, which is pure and computes nothing but the step; the copy in `verification/verus/select_step.rs` proves §D2's three properties under the pinned Verus version, recorded with its output; a CI lane fails when the two bodies differ, and its negative fixture proves it does; every existing TOTP test passes unchanged, and a property test shows the old and new `verify` agree on every input it draws; and each of the scoping note's three planted bugs is refused by the proof, recorded.
**Tracks.** Security assurance, formal verification: the first target, and the only one until it has paid for itself.
**Touches.** `crates/sui-id-core/src/authn/totp.rs`, `crates/sui-id-core/src/authn/totp/tests.rs`, `verification/verus/select_step.rs` (new), `verification/README.md`, `xtask/` (the drift check), `.github/workflows/ci.yml` and `contracts/gate-inputs.toml` (its lane), `docs/src/contributing/` (how to re-prove).
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Handoff.** [`../handoffs/146-totp-step-selection-is-proved/README.md`](../handoffs/146-totp-step-selection-is-proved/README.md)

## Summary

Every TOTP decision sui-id makes goes through fourteen lines that choose the
time step a six-digit code is accepted for (`crates/sui-id-core/src/authn/totp.rs:59-72`).
They are reached from password sign-in's second factor (`authn/mfa.rs:113`,
`:248`), step-up (`authn/step_up.rs:173`) and federated sign-in's second
factor (`authn/federated_mfa.rs:478`).

This RFC proves, for every input, that **the step they accept is newer than
the last one used and within one step of now**. So a code is never accepted
twice and never outside the ±30 s window, and no arithmetic overflows. The
proof is written for Verus and was already run, on 2026-11-14, in 0.56 s
([scoping note](../../verification/verus-scoping-note-2026-11-14.md) §2).

What this RFC adds is what makes the proof about the real code:
- production calls the same pure function the proof is about;
- a CI lane fails the moment the two copies differ.

It is the first formal-verification target, chosen as the smallest. Its
security gain is modest, since the store's guarded `UPDATE` is a second
line of defence. **Its main purpose is to measure what a proof costs this
project, cheaply, before any second target is considered.**

## Motivation

Tests sample. The selection's inputs are two `i64`s and three outcomes of a
comparison, a space no test enumerates, and its failure modes are exactly
the kind sampling misses: an off-by-one at the replay boundary, or an
overflow at an extreme value. A proof covers them all.

**The owner's constraint governs the scope**, 2026-11-10: *"Too wide range
will cost insanely. It should start from minimal and be optimized every
time."* So this RFC proves one function and builds only the machinery that
function needs. RFC 086's pilot is the precedent for not doing that: it left
Kani harnesses (`crates/sui-id-core/src/identity/authz.rs:197`) that nothing
in `.github/`, `scripts/` or `xtask/` runs.

## Non-goals

- **No other target.** `same_site_return_path` and the issuer host rule are
  the likeliest second targets (scoping note §5). Each is judged separately,
  after this one has run through a real change.
- **Not the HMAC, the constant-time comparison, or the clock.** They are
  inputs to what is proved, not part of it.
- **Not the replay guard across requests.** Two concurrent sign-ins with one
  code are stopped by `advance_last_used_step_within_tx`'s guarded `UPDATE`
  inside the command's transaction (`sui-id-store/src/repos/user_totp.rs`),
  and its tests.
- **No Verus run in CI.** See §D4.
- **No change to what a user sees.** The same codes are accepted and refused.

## Design

### D1 — production calls a pure `select_step`

`totp::verify` becomes:
1. reject a code of more than six digits, as today;
2. compute `now_step = unix_time.div_euclid(30)`, as today;
3. **compute the comparison for all three candidate steps**, `now_step - 1`,
   `now_step` and `now_step + 1`, each in constant time as today, into
   `matches: [bool; 3]`;
4. return `select_step(now_step, last_used_step, matches)`.

`select_step` is a private, synchronous, pure function. Its body is the
proved body in §D2, character for character after whitespace
normalisation.

**This is the one behaviour change.** Today the loop skips steps at or
before `last_used_step` without computing their HMAC, and stops at the first
match. After it, all three HMACs are always computed. That costs two more
HMAC-SHA1 evaluations per attempt, which is negligible. In return, the time
an attempt takes no longer depends on which step matched or how many were
skipped. The result is unchanged: the first matching eligible step in
`-1, 0, +1` order, or none.

### D2 — the proof

`verification/verus/select_step.rs` holds the scoping note's §2 source.
Its `ensures` clause states three properties:
- **(P1)** a returned step is greater than `last_used_step`;
- **(P2)** it is within one step of `now_step`, and its `matches` entry is
  true;
- **(P3)** when nothing is returned, no eligible step matched.

The `requires` clause, `i64::MIN < now_step < i64::MAX`, always holds in
production: `div_euclid(30)` bounds `now_step` by `i64::MAX / 30`. Verus
also proves the absence of overflow and that the loop terminates.

The copy keeps **production's exact form**: `now_step + deltas[k]`, with
`deltas = [-1, 0, 1]`. The scoping note records why that matters. A
first draft written as `now_step + (k as i64) - 1` overflows at
`i64::MAX - 1`, and Verus refused it.

### D3 — the drift check

A new `[gates]` lane, numbered when it is added, runs an `xtask` check:
1. extract `select_step`'s body from `totp.rs` and from the verified copy;
2. strip comments and normalise whitespace;
3. fail if they differ, printing both.

Two negative fixtures: a production body with one token changed, and a
verified copy with one token changed. Each must make the check fail. The
check reads source text only and needs no Verus, so it costs CI nothing.

Its scope is deliberately narrow: it proves the two bodies are the same, not
that the proof still passes. §D4 covers that.

### D4 — where Verus runs

**On a maintainer's machine, not in CI**, until this target pays for itself.
- `verification/README.md` records the Verus version the proof was last run
  with and the command (`verus verification/verus/select_step.rs`).
- Any change to `select_step`'s body must change both copies (D3 enforces
  that), and its review package must include a fresh Verus run's output.

A CI job would need a pinned Verus release and its own toolchain on a job of
its own. That is the widening the owner's constraint asks to defer.

## Tests

- **Every existing TOTP test passes unchanged.**
- **A property test**: the old `verify` (kept as a test-only reference in
  `totp/tests.rs`) and the new one return the same result. It draws
  arbitrary `unix_time`, `last_used_step`, secret and code, including codes
  that match at each of the three steps, and the `i64` extremes.
- **The drift check's two negative fixtures** (§D3).
- **The proof's three planted bugs**, each refused (scoping note §2): replay
  allowed; bound dropped; window widened. Recorded in the review package
  with Verus's output.

## Risks

| Risk | Mitigation |
|---|---|
| The proved copy and production diverge | §D3's lane fails on any difference |
| The proof stops passing after a change, and nobody re-runs it | §D4: the review package for any change to the body must carry a fresh run |
| Verus's dialect or version changes | The version is recorded. The proof is fourteen lines and is re-written if it must be |
| The cost turns out higher than measured | That is the finding this RFC exists to produce, and no second target follows until it is known |

## Acceptance criteria

The closure prerequisites above.

## Open questions

None. Where Verus runs is settled by §D4: locally, until this target has
paid for itself.
