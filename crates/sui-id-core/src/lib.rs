//! # sui-id-core
//!
//! Domain layer: password hashing, JWT signing/verification, OIDC Discovery,
//! JWKS, Authorization Code + PKCE, token issuance, sessions, and the setup
//! state machine.
//!
//! This crate has no knowledge of HTTP. It speaks in terms of the storage
//! layer and pure data; the wiring to Axum lives in `sui-id-bin`.

#![forbid(unsafe_code)]

pub mod cache;
pub mod errors;
pub mod password;
pub mod security;
pub mod tokens;
pub mod jwt;
pub mod jwks;
pub mod discovery;
pub mod authorize;
pub mod authz;
pub mod actor;
pub mod session;
pub mod setup;
pub mod admin;
pub mod dashboard;
pub mod me_security;
pub mod step_up;
pub mod time;
pub mod totp;
pub mod mfa;
pub mod webauthn;
pub mod oauth_token;
pub mod events;
pub mod mail;
pub mod forgot_password;
pub mod i18n;
pub mod hibp;
pub mod key_rotation;

pub use errors::{CoreError, CoreResult};
