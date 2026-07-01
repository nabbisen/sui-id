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

// Status badge primitives (RFC 052) — exposed for handlers and tests
// that want to compose a badge outside the page render functions.
pub use components::{StatusKind, status_badge};

pub use pages::{
    ClientEditData,
    ConfirmDeleteClientData,
    ConfirmDeleteSigningKeyData,
    ConfirmDeleteUserData,
    ConfirmDisableData,
    ConfirmResetMfaData,
    // RFC 059 — shared confirm-screen template
    ConfirmScreenData,
    ConsentData,
    DashboardData,
    DashboardEventRow,
    DashboardSparkBucket,
    DashboardSparkline,
    EmptyStateAction,
    // RFC 064 — empty-state primitives
    EmptyStateData,
    Flash,
    FlashKind,
    MeAuditEntry,
    MeLanguageData,
    MeMfaData,
    MeOverviewData,
    MePasskeyData,
    MeSecurityData,
    MeSessionDescriptor,
    MeSessionsData,
    MeShellData,
    MeTab,
    MfaSetupData,
    PasskeyDescriptor,
    PasswordChangeData,
    ReversibilityKind,
    SettingsAuthenticationData,
    SettingsBasicData,
    SettingsChainStatus,
    SettingsEmailData,
    SettingsLogsData,
    SettingsOtherData,
    SettingsSecurityData,
    SettingsTab,
    UserDetailData,
    UserDetailSession,
    confirm_screen,
    empty_state,
    render_audit,
    render_client_edit,
    render_clients,
    render_clients_new,
    render_confirm_delete_client,
    render_confirm_delete_signing_key,
    render_confirm_delete_user,
    render_confirm_disable_user,
    render_confirm_reset_mfa,
    render_consent,
    render_dashboard,
    render_error,
    render_forgot_password,
    render_forgot_password_sent,
    render_login,
    render_me_apps, // RFC 072
    render_me_language,
    render_me_mfa,
    render_me_overview,
    render_me_passkey,
    render_me_security,
    render_me_sessions,
    render_mfa_challenge,
    render_mfa_setup,
    render_password_change,
    render_reset_password,
    render_reset_password_invalid,
    render_settings_authentication,
    render_settings_basic,
    render_settings_email,
    render_settings_logs,
    render_settings_other,
    render_settings_security,
    render_setup_admin,
    render_setup_done,
    render_setup_hibp,
    render_setup_lang,
    render_setup_welcome,
    render_signing_keys,
    render_step_up,
    render_user_detail,
    render_users,
    render_users_new,
    table_empty_row,
};
