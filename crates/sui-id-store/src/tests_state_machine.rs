//! RFC 083 — Security state-machine property tests.
//!
//! Three harnesses covering the three core security lifecycles:
//! - `auth_codes` — authorization code issue / consume / expiry / purge.
//! - `refresh_tokens` — rotation family arbitration / reuse detection / purge.
//! - `sessions` — create / revoke / revoke-all-except / expiry / purge.
//!
//! Each harness drives a proptest sequence generator against both a trivial
//! in-memory oracle model and the real `Database`-backed implementation, then
//! asserts the named invariants (`INV_*`) after every step.

mod auth_codes;
mod refresh_tokens;
mod sessions;
