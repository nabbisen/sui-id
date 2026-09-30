# Password hashing must not block the request runtime

**RFC.** [RFC 126](../../accepted/126-password-hashing-must-not-block-the-runtime.md), **Proposed** — not yet accepted, and nothing is dispatched for implementation.
**Author.** High-capability model, requirements-architect role.

## Status

Written 2026-09-30, the same day the RFC 123 design review found it. It needs its
own independent design review before it can be accepted, and that review is not
dispatched yet: the dev team holds RFC 123's implementation next.

## What a reviewer will need to settle

1. **Where the boundary goes (D2).** At the `password.rs` API, or at each call
   site? The first is the only one that cannot be forgotten by a new caller; it
   may not be achievable if a caller needs a borrowed value across the boundary.
   Whoever reviews this should establish which, from the actual call sites, not
   from preference.
2. **The pool's bound (D4).** Tokio's blocking pool defaults to 512 threads.
   At 64 MiB per concurrent hash that is not a bound anyone wants to discover
   under load. What is the right number, and is it a fixed cap or derived?
3. **Whether the timing equalisation survives (D3).** The `DUMMY_PHC` calls
   exist to make a failed lookup cost what a real verification costs. Crossing a
   thread boundary adds its own variable cost to both — establish that it adds
   it to both, and not only to one.
4. **Whether this is the whole exposure.** Argon2 is the expensive one measured,
   but it may not be the only synchronous work on the request path. A reviewer
   should look rather than assume.

## What is already known, so it is not re-derived

- Argon2: `m_cost = 64 MiB`, `t_cost = 2`, `p_cost = 1` — `authn/password.rs:11-13`.
- One verification: **~34 ms**, measured over 300 samples (RFC 123's design
  review evidence).
- `#[tokio::main]` with no `worker_threads` override — `crates/sui-id/src/main.rs:22`.
- `spawn_blocking` is **already used** in `sui-id-store/src/backend.rs` and
  `authn/hibp.rs`. The pattern exists; it is simply not applied here.
- Roughly 20 non-test call sites across `sui-id-core` and `sui-id`.
