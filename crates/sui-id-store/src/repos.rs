//! Thin SQL → typed-row repositories.
//!
//! Each submodule covers one entity. Functions take a `Database` and operate
//! on a short-lived locked connection. Errors are surfaced as `StoreError` so
//! the caller can distinguish "not found" from real failures.

pub mod audit;
pub mod auth_codes;
pub mod clients;
pub mod credentials;
pub mod email_outbox;
pub mod json_util;
pub mod login_pending_mfa;
pub mod password_reset_tokens;
pub mod pending_settings_change;
pub mod refresh_tokens;
pub mod revoked_access_tokens;
pub mod server_settings;
pub mod sessions;
pub mod signing_keys;
pub mod smtp_config;
pub mod state;
pub mod user_consent;
pub mod user_totp;
pub mod user_webauthn_credentials;
pub mod users;
pub mod webauthn_pending;
