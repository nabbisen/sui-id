# RFC 126 — independent design review request

**RFC.** [RFC 126 — Password hashing must not block the request runtime](../../done/126-password-hashing-must-not-block-the-runtime.md). **Accepted 2026-09-30 — and its design is unreviewed**, which its header states.
**Reviewer.** Mid-capability model, implementation role. **It found this problem**, while reviewing RFC 123; it did not author this RFC. That is a closer involvement than usual and is disclosed rather than left to be noticed.
**Baseline.** `7262354` or later.
**Scope.** Read-only. Report findings.
**Why a review of an Accepted RFC.** `@nabbisen` accepted it on the strength of
the finding. The finding is not the design, and implementation stays undispatched
until the four questions below are settled.

## What to settle

1. **D2 — where the boundary goes.** At `password.rs`'s own API, or at each of
   the ~20 call sites? Only the first cannot be forgotten by a future caller.
   **Establish which is achievable from the actual call sites**, not from
   preference — in particular whether any caller needs a borrowed value across
   the boundary, which is what would force the worse option.
2. **D4 — the pool's bound.** Tokio's blocking pool defaults to 512 threads. At
   64 MiB per concurrent hash that is not a bound anyone wants to meet under
   load. Recommend a number or a derivation, and say what happens to a caller
   when the bound is reached — a queue, a rejection, or a stall — because that
   choice is the availability behaviour, not an implementation detail.
3. **D3 — does the timing equalisation survive?** `DUMMY_PHC` exists so a failed
   lookup costs what a real verification costs, and RFC 123 has just added the
   same to `authenticate_client`. A thread hop adds its own variable cost.
   **Establish that it adds it to both paths and not only one** — and say how a
   test would show that, given the measurement noise a thread boundary
   introduces.
4. **Is Argon2 the only synchronous work on the request path?** It is the
   expensive one measured. **Look rather than assume.**

## Also worth your view

5. **Is `spawn_blocking` the right instrument**, or a dedicated pool? The
   codebase already uses `spawn_blocking` for database work in
   `sui-id-store/src/backend.rs`, so the same pool would then carry both
   microsecond SQLite reads and 34 ms hashes. Say whether that matters.
6. **Anything else**, including whether this RFC should be narrower or wider than
   it is.

## What to return

The usual package: your answers to 1–6 each citing what you read, findings
ranked, and a recommendation — including "the RFC should say something different"
if that is what you conclude.
