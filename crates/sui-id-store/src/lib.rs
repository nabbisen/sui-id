//! # sui-id-store
//!
//! Persistence layer for sui-id. Owns the SQLite connection, runs schema
//! migrations on startup, and exposes thin repository functions for the
//! domain layer in `sui-id-core`.
//!
//! ## Encryption model
//!
//! Rather than relying on a SQLCipher-style file-level encryption (which would
//! pull in a heavy native dependency tree and complicate single-binary
//! distribution), sui-id encrypts *individual sensitive columns* at the
//! application layer using XChaCha20-Poly1305 with an authenticated tag. The
//! master key lives outside the database (env var or a separate key file with
//! strict permissions) and is never persisted next to the data it protects.

#![forbid(unsafe_code)]

pub mod backend;
pub mod crypto;
#[cfg(feature = "ldap")]
pub mod ldap_source;
pub mod user_source;

// ── Global metrics handle (RFC 006) ─────────────────────────────────────────
//
// Set once at startup by the binary crate via `set_global_metrics()`. Store
// internals (e.g. `audit::append`) can then call the metrics registry without
// any signature change at their 40+ call sites.

use std::sync::OnceLock;
static GLOBAL_METRICS: OnceLock<std::sync::Arc<metrics::Metrics>> = OnceLock::new();

/// Install the global metrics registry.  Safe to call once; subsequent calls
/// are no-ops (the `OnceLock` ignores them).  Called by the binary crate
/// immediately after constructing `AppState` when `metrics_enabled = true`.
pub fn set_global_metrics(m: std::sync::Arc<metrics::Metrics>) {
    let _ = GLOBAL_METRICS.set(m);
}

/// Borrow the global metrics registry, or `None` when metrics are disabled.
#[inline]
pub fn global_metrics() -> Option<&'static metrics::Metrics> {
    GLOBAL_METRICS.get().map(|m| m.as_ref())
}

pub mod db;
pub mod errors;
pub mod metrics;
pub mod migrations;
pub mod models;
pub mod repos;

pub use db::Database;
pub use errors::{StoreError, StoreResult};

#[cfg(test)]
mod tests_rfc021;

#[cfg(test)]
mod tests_state_machine;
