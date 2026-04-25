//! Thin SQL → typed-row repositories.
//!
//! Each submodule covers one entity. Functions take a `Database` and operate
//! on a short-lived locked connection. Errors are surfaced as `StoreError` so
//! the caller can distinguish "not found" from real failures.

pub mod audit;
pub mod auth_codes;
pub mod clients;
pub mod credentials;
pub mod refresh_tokens;
pub mod sessions;
pub mod signing_keys;
pub mod state;
pub mod users;
