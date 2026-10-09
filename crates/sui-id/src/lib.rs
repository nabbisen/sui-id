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
#[path = "http/cache_freshness.rs"]
pub mod cache_freshness;
#[path = "runtime/config.rs"]
pub mod config;
#[path = "http/consent_state.rs"]
pub mod consent_state;
#[path = "http/cors.rs"]
pub mod cors;
#[path = "http/csrf.rs"]
pub mod csrf;
#[path = "runtime/database.rs"]
pub mod database;
#[path = "runtime/dev_mode.rs"]
pub mod dev_mode;
#[path = "http/discovery.rs"]
pub mod discovery;
#[path = "http/dynamic_registration_validation.rs"]
pub mod dynamic_registration_validation;
#[path = "runtime/egress.rs"]
pub mod egress;
#[path = "http/errors.rs"]
pub mod errors;
#[path = "http/federation_cache.rs"]
pub mod federation_cache;
#[path = "http/federation_identity.rs"]
pub mod federation_identity;
#[path = "http/federation_state.rs"]
pub mod federation_state;
#[path = "runtime/gc.rs"]
pub mod gc;
#[path = "http/handlers.rs"]
pub mod handlers;
#[path = "http/id_token.rs"]
pub mod id_token;
#[path = "http/identity_capability.rs"]
pub mod identity_capability;
#[path = "http/identity_claims.rs"]
pub mod identity_claims;
#[path = "runtime/ipnet.rs"]
pub mod ipnet;
#[path = "http/jwks.rs"]
pub mod jwks;
#[path = "runtime/keyring.rs"]
pub mod keyring;
#[path = "http/optional_claims.rs"]
pub mod optional_claims;
#[path = "runtime/ratelimit.rs"]
pub mod ratelimit;
#[path = "http/request_id.rs"]
pub mod request_id;
#[path = "runtime/resolver.rs"]
pub mod resolver;
#[path = "http/response_bounds.rs"]
pub mod response_bounds;
#[path = "http/router.rs"]
pub mod router;
#[path = "http/security_headers.rs"]
pub mod security_headers;
#[path = "runtime/startup.rs"]
pub mod startup;
#[path = "runtime/state.rs"]
pub mod state;
#[path = "http/time_claims.rs"]
pub mod time_claims;

pub use config::Config;
pub use router::build_router;
pub use startup::Startup;
pub use state::AppState;
