# Password hashing must not block the request runtime

**RFC.** [RFC 126](../../accepted/126-password-hashing-must-not-block-the-runtime.md), **Proposed** — not yet accepted, and nothing is dispatched for implementation.
**Author.** High-capability model, requirements-architect role.

## Dispatched for implementation 2026-09-30

RFC 126 is **Accepted**, and its design is now reviewed — the four questions this
section used to pose are settled in the RFC as D2–D4 and D6. **Build against the
RFC.** They are not reopened here, and this section no longer asks them.

## What to build

**One async boundary, inside `password.rs` (D2).** `verify_password` and
`hash_password` become `async fn`, wrapping `tokio::task::spawn_blocking`
internally, with the password and stored hash cloned to owned `String`s before
the closure. **23 of the 24 call sites then need only `.await`.**

**The 24th needs a rewrite, and it is the only one (D2).** `authn/mfa.rs:363`
passes a *synchronous* closure to `Iterator::position`, which cannot host an
`.await` however ownership is arranged. Replace it with an explicit loop. Do
**not** keep a synchronous wrapper for this one site: that would reintroduce
exactly what the boundary exists to prevent, and it would do so at the call site
with the worst blocking cost in the codebase.

**A semaphore inside `password.rs` (D4).** `tokio::sync::Semaphore`, acquired
before `spawn_blocking` and released by a guard dropped in the async wrapper, so
no call site sees it. Bound derived, not picked:
`(available_parallelism() * 2).min(16)`. **A caller that meets the bound queues**
— `acquire().await` does not hold a worker thread — and that is the architect's
decision about user-visible behaviour, not a property of the primitive. Do not
substitute a rejection.

**Leave the recovery-code loops sequential (D6).** `mfa.rs`'s enrollment and
regeneration hash eight codes each. Once `hash_password` is async it becomes
possible to run them concurrently; do not. Eight permits at once for one
enrollment would compete with unrelated sign-ins to save latency nobody asked to
improve.

## Evidence

**D5 is the closure requirement and is not optional.** A test issues concurrent
*real* (non-dummy) authentication attempts and shows an unrelated,
health-check-shaped request served promptly during the burst — **failing before
this change and passing after**. Run it under
`#[tokio::test(flavor = "multi_thread", worker_threads = 1)]` so the result is
deterministic rather than machine-dependent.

**D3's timing test** compares a real verification against a `DUMMY_PHC` one after
the wrap exists: same-order medians over many samples, not equality, which would
be flaky. The assertion is that the thread hop's cost lands on **both** branches
— not that it is zero.

**Mutations:** remove the semaphore; move the wrap to one call site instead of
the API; revert `mfa.rs:363` to a synchronous wrapper. Name the test that catches
each.

Plus fmt, both clippy scopes, the workspace count before and after, MSRV, and
every doc gate.

## Say, in the package

Whether **anything else on the request path blocks**. The design review found
Argon2 is the only comparable cost and checked WebAuthn, JWT signing and the
AES-GCM operations. If you find another while you are in here, report it rather
than fixing it — that is the escalation this project runs on.

## What is already known, so it is not re-derived

- Argon2: `m_cost = 64 MiB`, `t_cost = 2`, `p_cost = 1` — `authn/password.rs:11-13`.
- One verification: **~34 ms**, measured over 300 samples (RFC 123's design
  review evidence).
- `#[tokio::main]` with no `worker_threads` override — `crates/sui-id/src/main.rs:22`.
- `spawn_blocking` is **already used** in `sui-id-store/src/backend.rs` and
  `authn/hibp.rs`. The pattern exists; it is simply not applied here.
- Roughly 20 non-test call sites across `sui-id-core` and `sui-id`.

## Reverted 2026-09-30, and re-dispatched: the D5 deadline does not discriminate

The implementation landed as `ff19f05` and was **reverted** as `7813b35`. The
production change was verified and is not in question. **Both D5 tests failed in
CI**, on `G06` (stable, all-features) and `G02` (1.95, default), with the same
assertion:

```
a probe iteration took 52.837674ms (deadline 30ms) while 8 real login
attempts were in flight — a single worker thread was busy with Argon2
```

**The design of the test is right; the threshold is wrong.** Measuring elapsed
wall-clock across the whole iteration — the third design, the one that survived
mutation testing — is the correct shape and should be kept. What fails is the
**30 ms deadline**, which is tight rather than discriminating: on a
GitHub-hosted runner inside a 1300-second suite, 52 ms of ordinary scheduling
delay clears it with no Argon2 blocking involved at all.

**Choose the threshold to separate the two populations, not to be small.**

- **Blocked** (what the test must catch): a probe stuck behind Argon2 on the one
  worker waits on the order of one hash per queued caller — RFC 126's own
  numbers make that **~34 ms each, and up to ~270 ms** for the recovery-code
  path, with eight in flight.
- **Not blocked, just a slow runner**: tens of milliseconds, as measured — 52 ms
  here.

A deadline in the low hundreds of milliseconds separates those cleanly; 30 ms
separates neither. **Derive it from the RFC's own measured hash cost rather than
picking a number**, and say in the package what margin you chose and against
which of the two populations.

**Also required, because this is the second time a test in this suite has been
environment-dependent:** state how the test behaves on a runner slower still.
A threshold that merely moves the failure to a slower machine has not fixed it.
If the honest answer is that no wall-clock threshold is safe in CI, say so — the
alternative is to assert the *relationship* (a probe during the burst is not
dramatically slower than a probe outside it, measured in the same run) rather
than an absolute, which is immune to machine speed.

**The reviewer's error, recorded because it caused this.** The architect landed
`ff19f05` on twenty clean local runs, having argued that the r126 tests were not
implicated in an intermittent failure. Local runs on a quiet machine cannot clear
a wall-clock promptness test for a contended runner. The evidence needed was a
CI run, and it existed twenty-two minutes later.
