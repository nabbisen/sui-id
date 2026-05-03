//! # sui-id-web
//!
//! Server-rendered HTML for the setup wizard, login screen, and admin
//! dashboard. Each page is produced by a Leptos component rendered with
//! `leptos::prelude::ssr::render_to_string`, which yields a complete HTML
//! string the binary serves directly. No client-side JavaScript or WASM is
//! shipped — that decision keeps the runtime artefact a single static
//! binary, in line with the project's minimalism.
//!
//! Pages do progressive enhancement only: ordinary `<form>` POSTs go back to
//! Axum handlers, which redirect on success.

#![forbid(unsafe_code)]

pub mod components;
pub mod layout;
pub mod pages;
pub mod tokens;

pub use pages::{
    render_audit, render_client_edit, render_clients, render_dashboard, render_error,
    render_login, render_me_security, render_mfa_challenge, render_mfa_setup,
    render_password_change, render_profile, render_settings_authentication,
    render_settings_basic, render_settings_logs, render_settings_other, render_settings_security,
    render_setup_admin, render_setup_done, render_setup_welcome, render_signing_keys,
    render_users, ClientEditData, DashboardData, DashboardSparkBucket, DashboardSparkline, Flash,
    FlashKind, MeAuditEntry, MeSecurityData, MeSessionDescriptor, MfaSetupData, PasskeyDescriptor,
    PasswordChangeData, ProfileData, SettingsAuthenticationData, SettingsBasicData,
    SettingsChainStatus, SettingsLogsData, SettingsOtherData, SettingsSecurityData, SettingsTab,
};
