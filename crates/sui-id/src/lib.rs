//! # sui-id
//!
//! Entry point: configuration loading, master-key resolution, tracing setup,
//! Axum routing, asset embedding, and graceful shutdown. The library half
//! exists so that integration tests in `tests/` can spin up a fully wired
//! server without going through `main`.

#![forbid(unsafe_code)]

#[path = "http/assets.rs"]
pub mod assets;
pub mod backup;
#[path = "runtime/config.rs"]
pub mod config;
#[path = "http/cors.rs"]
pub mod cors;
#[path = "http/csrf.rs"]
pub mod csrf;
#[path = "runtime/dev_mode.rs"]
pub mod dev_mode;
#[path = "http/errors.rs"]
pub mod errors;
#[path = "runtime/gc.rs"]
pub mod gc;
#[path = "http/handlers.rs"]
pub mod handlers;
#[path = "runtime/ipnet.rs"]
pub mod ipnet;
#[path = "runtime/keyring.rs"]
pub mod keyring;
#[path = "runtime/ratelimit.rs"]
pub mod ratelimit;
#[path = "http/request_id.rs"]
pub mod request_id;
#[path = "http/router.rs"]
pub mod router;
#[path = "http/security_headers.rs"]
pub mod security_headers;
#[path = "runtime/startup.rs"]
pub mod startup;
#[path = "runtime/state.rs"]
pub mod state;

pub use config::Config;
pub use router::build_router;
pub use startup::Startup;
pub use state::AppState;
