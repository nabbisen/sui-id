//! # sui-id
//!
//! Entry point: configuration loading, master-key resolution, tracing setup,
//! Axum routing, asset embedding, and graceful shutdown. The library half
//! exists so that integration tests in `tests/` can spin up a fully wired
//! server without going through `main`.

#![forbid(unsafe_code)]

pub mod assets;
pub mod backup;
pub mod config;
pub mod cors;
pub mod csrf;
pub mod errors;
pub mod gc;
pub mod handlers;
pub mod ipnet;
pub mod keyring;
pub mod ratelimit;
pub mod request_id;
pub mod router;
pub mod security_headers;
pub mod startup;
pub mod state;

pub use config::Config;
pub use router::build_router;
pub use startup::Startup;
pub use state::AppState;
