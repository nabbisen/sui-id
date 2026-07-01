//! # sui-id-core
//!
//! Domain layer: password hashing, JWT signing/verification, OIDC Discovery,
//! JWKS, Authorization Code + PKCE, token issuance, sessions, and the setup
//! state machine.
//!
//! This crate has no knowledge of HTTP. It speaks in terms of the storage
//! layer and pure data; the wiring to Axum lives in `sui-id-bin`.

#![forbid(unsafe_code)]

pub mod actor;
pub mod admin;
pub mod audit_guard;
pub mod authorize;
pub mod authz;
pub mod cache;
pub mod dashboard;
pub mod discovery;
pub mod errors;
pub mod events;
pub mod forgot_password;
pub mod hibp;
pub mod i18n;
pub mod jwks;
pub mod jwt;
pub mod key_rotation;
pub mod mail;
pub mod me_security;
pub mod mfa;
pub mod oauth_token;
pub mod password;
pub mod pending_change;
pub mod security;
pub mod session;
pub mod setup;
pub mod step_up;
pub mod time;
pub mod tokens;
pub mod totp;
pub mod webauthn;

pub use errors::{CoreError, CoreResult};
