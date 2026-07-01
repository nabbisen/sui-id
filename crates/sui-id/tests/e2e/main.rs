//! End-to-end test of the full OIDC flow against the in-process router.
//!
//! Boots an `AppState` with an in-memory SQLite database, completes setup,
//! registers a client, drives an Authorization Code + PKCE flow, exchanges
//! the code, calls userinfo with the resulting Bearer token, and rotates a
//! refresh token. Negative cases verify that PKCE failure, redirect-uri
//! mismatch, and replayed codes are rejected.
//!
//! ## Layout
//!
//! Per-feature integration tests live under `tests/e2e/<theme>.rs` and
//! are wired in here as modules so they share a single integration test
//! binary (one `cargo test -p sui-id --test e2e` invocation runs all of
//! them) and a single set of helpers in `tests/e2e/common.rs`.
//!
//! Adding a new theme:
//!   1. Create `tests/e2e/<your_theme>.rs`.
//!   2. Add a `mod <your_theme>;` line below.
//!   3. Use helpers from `super::common::*` rather than duplicating them.

mod common;

mod acr_amr;
mod auth_flow_integrity;
mod backup;
mod clients_edit;
mod csrf;
mod dashboard;
mod dev_mode;
mod email_forgot;
mod email_pwd_change;
mod hibp;
mod i18n_auth_flow;
mod i18n_basic;
mod i18n_me_security;
mod i18n_phase2;
mod introspection;
mod key_rotation;
mod lockout;
mod logout_jwks;
mod me_security;
mod mfa;
mod oidc_flow;
mod password_change;
mod refresh_theft;
mod request_id;
mod rfc030_033_035;
mod rfc6749_error_format;
mod scope_logout;
mod sec_headers;
mod session_limits;
mod settings;
mod setup_wizard;
mod step_up_totp;
mod step_up_webauthn;
mod user_identity_invariants;
