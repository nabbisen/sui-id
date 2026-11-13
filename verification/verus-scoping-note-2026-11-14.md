# Verus: a scoping note, and one first target

> ## Whose turn it is: **yours**, for one decision (§6).
> Nothing here changes code. Whether the target becomes an RFC, and when
> it is built, is decided on this note (ROADMAP, *"After K: a scoping note
> on formal verification"*).

**Written.** 2026-11-14, by the architect.
**Verus.** `0.2026.11.02.8bf68ee` (`verus --version`, re-read today; it has
been upgraded since 2026-11-09).
**The owner's guidance**, 2026-11-10: *"Too wide range will cost insanely.
It should start from minimal and be optimized every time."* So this note
proposes **one** target and prices it. It is not a catalogue to adopt.

## 1 — the target: TOTP step selection

`sui_id_core::authn::totp::verify` (`crates/sui-id-core/src/authn/totp.rs:50-73`)
decides which time step a six-digit code is accepted for. Its selection
logic (`:59-72`) is fourteen lines of integer arithmetic. **Every TOTP
decision goes through it**, at four production call sites:
- password sign-in's second factor (`authn/mfa.rs:113` and `:248`);
- step-up (`authn/step_up.rs:173`);
- federated sign-in's second factor (`authn/federated_mfa.rs:478`).

**The property, for every input**: the step it accepts is greater than
`last_used_step` and within one step of `now_step`, so a code is never
accepted twice and never outside the ±30 s window. When it refuses, no
eligible step matched. And no arithmetic overflows.

Why this one first: it is the smallest kernel whose property is
security-critical and holds over an input space no test can enumerate
(two `i64`s and three booleans). It is also pure integers, which is where
Verus is strongest, so it prices the machinery (§4) at its cheapest. Its
security gain is real but modest: the code is short and the store's
`UPDATE … WHERE last_used_step < ?1` (`sui-id-store/src/repos/user_totp.rs`,
`advance_last_used_step_within_tx`) is a second guard behind it. **The
first target's main value is learning what a proof costs here, cheaply.**

## 2 — the evidence: it was built and run, not estimated

RFC 086's pilot (`verification/pilot-reports.md`) estimated every proof
time it reported, because no tool was installed. Verus is installed now, so
this target was written and run on 2026-11-14.

```rust
use vstd::prelude::*;

verus! {

/// The step-selection half of `sui_id_core::authn::totp::verify`
/// (`crates/sui-id-core/src/authn/totp.rs:59-72`), with the HMAC comparison
/// supplied as input: `matches[k]` is "the supplied code equals the HMAC for
/// step `now_step + k - 1`". Candidates are tried in production's order.
pub fn select_step(now_step: i64, last_used_step: i64, matches: [bool; 3]) -> (r: Option<i64>)
    requires
        i64::MIN < now_step < i64::MAX,
    ensures
        match r {
            Some(s) => s > last_used_step
                && now_step - 1 <= s <= now_step + 1
                && matches[s - now_step + 1],
            None => forall|k: int| 0 <= k < 3 && now_step + k - 1 > last_used_step ==> !matches[k],
        },
{
    let deltas: [i64; 3] = [-1, 0, 1];
    let mut k: usize = 0;
    while k < 3
        invariant
            0 <= k <= 3,
            i64::MIN < now_step < i64::MAX,
            deltas[0] == -1, deltas[1] == 0, deltas[2] == 1,
            forall|j: int| 0 <= j < k && now_step + j - 1 > last_used_step ==> !matches[j],
        decreases 3 - k,
    {
        let step = now_step + deltas[k];
        if step > last_used_step && matches[k] {
            return Some(step);
        }
        k += 1;
    }
    None
}

fn main() {}

} // verus!
```

| Run | Result |
|---|---|
| As above | **3 verified, 0 errors, 0.56 s** wall |
| Replay allowed (`step > last_used_step` → `>=`) | refused: *postcondition not satisfied* |
| Bound dropped (no `requires`) | refused: *invariant not satisfied before loop* |
| Window widened (`deltas` `[-1, 0, 2]`) | refused: *postcondition not satisfied*, and an overflow |

**It caught a real bug, in my first draft.** I wrote the step as `now_step +
(k as i64) - 1`. Verus refused it: *"possible arithmetic underflow/overflow"*,
because `now_step + 2` overflows at `i64::MAX - 1` before the `- 1`.
Production writes `now_step + delta` and has no such intermediate. The
lesson prices the sync cost: **the verified copy must keep production's
exact form**, or it proves something else.

The precondition always holds in production: `now_step` is
`unix_time.div_euclid(30)`, which is at most `i64::MAX / 30` in magnitude.

## 3 — what the proof does not cover

- **The HMAC.** Whether `matches[k]` is computed correctly, from the right
  secret, is assumed. So is the constant-time comparison (`ct_eq`).
- **The replay guard across requests.** Two concurrent sign-ins with one
  code are stopped by the store's guarded `UPDATE` inside the command's
  transaction, not by this function.
- **The clock.** `unix_time` is trusted.
- **The production code itself**, until §4's sync check exists. The proof
  is about the copy.

## 4 — what adopting it costs

1. **A small production refactor.** `verify` would call a pure
   `select_step(now_step, last_used_step, matches)`, with all three
   HMACs computed first. That is a behaviour change. Computing all three
   always, rather than stopping at the first match, makes the time a
   request takes independent of which step matched, which is a small
   improvement. So it needs an RFC and, under the freeze, your exception.
2. **Keeping the copy in step.** Two parts, of different cost:
   - **A drift check in CI: cheap.** An `xtask` lane that compares
     `select_step`'s body in `totp.rs` with the verified copy's,
     whitespace-normalised, and fails if they differ. This is what makes
     the proof about production rather than about a copy.
   - **Running Verus in CI: deferred.** A pinned Verus release and its own
     toolchain (Verus pins one, separate from the project's), on a job of
     its own. Until it pays for itself, Verus runs on the owner's machine
     only, and the drift check makes any edit to the function need a local
     re-proof (0.56 s).
3. **Per later change:** edit both copies, re-prove, and the drift check
   confirms they match. For a fourteen-line function that changes rarely
   (`git log -L59,72` on `totp.rs`: last changed 2026-07-02, in
   housekeeping, and before that 2026-06-05), that is minutes.

## 5 — considered, and not first

| Candidate | Why not first |
|---|---|
| `lockout_backoff` (`authn/session.rs:33`) | A nine-arm table and a `min`, no arithmetic to overflow, and `authn/session/lockout_tests.rs` already loops over it. A proof would add little |
| `authorize` (`identity/authz.rs`) | Finite: 3 roles × 23 action values (20 `Action` variants, three carrying a `bool`; counted 2026-11-14), and an exhaustive table test exists. **RFC 086 left Kani harnesses at `authz.rs:197`, but nothing in `.github/`, `scripts/` or `xtask/` runs them, and Kani is not installed here.** A proof nobody runs is not evidence; whether to wire it or remove it is a separate, small question |
| `same_site_return_path` (open-redirect guard), and the issuer host rule | **The best second target**: higher value, but string-shaped, where Verus costs more. Judged only after the first has paid for itself |

## 6 — the decision

**Recommendation: write the RFC for this one target now, design-only, and
build it after the soak**, with no freeze exception. It is not on the path
to the soak, and the freeze exists for that path.

The alternative is a freeze exception to build it in `0.91.0`. That is
small (one function, one `xtask` lane). But `0.91.0` is where M2c, RFC
100's build, goes if RFC 100 is accepted (cycle L's record, Track 2), and
M2c is on the path to the soak. This would compete with it for a modest
security gain.

Either way, **no widening** until this target has run through one real
change and the cost in §4 has been measured, not estimated.
