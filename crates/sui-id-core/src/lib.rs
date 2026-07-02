//! # sui-id-core
//!
//! Domain layer: password hashing, JWT signing/verification, OIDC Discovery,
//! JWKS, Authorization Code + PKCE, token issuance, sessions, and the setup
//! state machine.
//!
//! This crate has no knowledge of HTTP. It speaks in terms of the storage
//! layer and pure data; the wiring to Axum lives in `sui-id-bin`.

#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::items_after_test_module,
        clippy::panic,
        clippy::unwrap_used
    )
)]

#[path = "identity/actor.rs"]
pub mod actor;
#[path = "identity/admin.rs"]
pub mod admin;
pub mod audit_guard;
#[path = "oidc/authorize.rs"]
pub mod authorize;
#[path = "identity/authz.rs"]
pub mod authz;
pub mod cache;
pub mod dashboard;
#[path = "oidc/discovery.rs"]
pub mod discovery;
pub mod errors;
pub mod events;
#[path = "account/forgot_password.rs"]
pub mod forgot_password;
#[path = "authn/hibp.rs"]
pub mod hibp;
pub mod i18n;
#[path = "oidc/jwks.rs"]
pub mod jwks;
#[path = "oidc/jwt.rs"]
pub mod jwt;
#[path = "oidc/key_rotation.rs"]
pub mod key_rotation;
#[path = "communication/mail.rs"]
pub mod mail;
#[path = "account/me_security.rs"]
pub mod me_security;
#[path = "authn/mfa.rs"]
pub mod mfa;
#[path = "oidc/oauth_token.rs"]
pub mod oauth_token;
#[path = "authn/password.rs"]
pub mod password;
#[path = "settings/pending_change.rs"]
pub mod pending_change;
pub mod security;
#[path = "authn/session.rs"]
pub mod session;
pub mod setup;
#[path = "authn/step_up.rs"]
pub mod step_up;
pub mod time;
#[path = "oidc/tokens.rs"]
pub mod tokens;
#[path = "authn/totp.rs"]
pub mod totp;
#[path = "authn/webauthn.rs"]
pub mod webauthn;

pub use errors::{CoreError, CoreResult};
