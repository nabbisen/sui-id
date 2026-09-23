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

// This whole binary is integration-test code: unwrap()/expect()/panic!() on
// setup and assertion steps is normal here and a failure is exactly the
// panic we want (`cargo test` reports it as a test failure).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

mod acr_amr;
mod auth_flow_integrity;
mod backup;
mod backup_checks;
mod clients_edit;
mod csrf;
mod dashboard;
mod dev_mode;
mod email_forgot;
mod email_pwd_change;
mod federation_fail_closed;
mod hibp;
mod i18n_auth_flow;
mod i18n_basic;
mod i18n_me_security;
mod i18n_phase2;
mod introspection;
mod key_rotation;
mod ldap_returning_signin;
mod lockout;
mod logout_jwks;
mod me_security;
mod mfa;
mod oidc_flow;
mod password_change;
mod r102_stage1;
mod r102_stage2;
mod r102_stage3;
mod r102_stage4;
mod r102_stage6;
mod r102_stage7;
mod r103_reset_mfa;
mod r103_stage1;
mod r103_stage2;
mod r103_stage3;
mod r103_stage4;
mod r103_stage5;
mod r115_stage1;
mod r11_login_failure;
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
