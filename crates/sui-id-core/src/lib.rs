//! # sui-id-core
//!
//! Domain layer: password hashing, JWT signing/verification, OIDC Discovery,
//! JWKS, Authorization Code + PKCE, token issuance, sessions, and the setup
//! state machine.
//!
//! This crate has no knowledge of HTTP. It speaks in terms of the storage
//! layer and pure data; the wiring to Axum lives in `sui-id-bin`.

#![forbid(unsafe_code)]

pub mod errors;
pub mod password;
pub mod tokens;
pub mod jwt;
pub mod jwks;
pub mod discovery;
pub mod authorize;
pub mod session;
pub mod setup;
pub mod admin;
pub mod time;
pub mod totp;
pub mod mfa;
pub mod webauthn;
pub mod oauth_token;

pub use errors::{CoreError, CoreResult};
