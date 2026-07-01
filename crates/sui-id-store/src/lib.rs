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

pub mod crypto;
pub mod db;
pub mod errors;
pub mod migrations;
pub mod models;
pub mod repos;

pub use db::Database;
pub use errors::{StoreError, StoreResult};

#[cfg(test)]
mod tests_rfc021;

#[cfg(test)]
mod tests_state_machine;
