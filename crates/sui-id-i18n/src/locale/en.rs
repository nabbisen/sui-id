//! English (`en`) translation table and date/number formatters.
//!
//! **Translator guide:**
//! Edit the string values between `\"…\"` only. Do not rename field names.
//! Every field must be present — the compiler enforces completeness.
//! After editing, run `cargo test -p sui-id-i18n` to confirm all tests pass.

use crate::formatters::{Formatters, fmt_count_shared, fmt_time_shared};
use crate::strings::Strings;
use chrono::{DateTime, Datelike, Utc};

// ── Strings ──────────────────────────────────────────────────────────────────

pub static STRINGS_EN: Strings = Strings {
    // Generic UI
    button_save: "Save",
    button_cancel: "Cancel",
    button_back: "Back",
    button_continue: "Continue",
    button_delete: "Delete",
    danger_zone_title: "Danger Zone",
    button_create: "Create",
    button_confirm: "Confirm",
    button_test: "Test",
    badge_enabled: "Enabled",
    badge_disabled: "Disabled",
    badge_ok: "OK",
    badge_warn: "Warning",
    badge_error: "Error",
    label_optional: "(optional)",
    label_required: "(required)",
    muted_none: "(none)",

    // Language native names (RFC 051) — identical across all locales.
    locale_native_ja: "日本語",
    locale_native_en: "English",
    locale_native_zh_hans: "中文（简体）",
    locale_native_zh_hant: "中文（繁體）",

    // Lifetime formatting (RFC 051)
    fmt_lifetime_days: |n, secs| format!("{n} d ({secs}s)"),
    fmt_lifetime_hours: |n, secs| format!("{n} h ({secs}s)"),
    fmt_lifetime_minutes: |n, secs| format!("{n} m ({secs}s)"),

    // Settings: Auth extensions (RFC 051)
    settings_auth_min_length_value: |n| format!("{n} characters"),
    settings_auth_recovery_codes_label: "Recovery codes (per enrollment)",
    settings_auth_recovery_codes_value: |n| format!("{n} codes"),
    settings_auth_mfa_note_prefix: "Enabled per user. See ",
    settings_auth_mfa_note_suffix: " for details.",

    // Settings: Logs extensions (RFC 051)
    settings_logs_lede: "Log output settings and audit log status.",
    settings_logs_kv_format: "Format",
    settings_logs_kv_filter: "Filter",
    settings_logs_audit_link_prefix: "For detailed history see ",
    settings_logs_audit_link_suffix: ".",

    // Settings: Advanced/Other extensions (RFC 051)
    settings_advanced_lede: "Build, schema, and storage details.",
    settings_advanced_storage_note_prefix: "The database is a single SQLite file. The master key is read from the key file only when the environment variable ",
    settings_advanced_storage_note_suffix: " is not set.",
    settings_advanced_users_count: |n| format!("{n} users "),
    settings_advanced_clients_count: |n| format!("{n} clients "),

    // Settings: Email extensions (RFC 051)
    settings_email_enable_checkbox: "Enable email",
    settings_email_enable_hint: "When disabled, forgot-password and other mail-sending endpoints are unavailable.",
    settings_email_password_placeholder_change: "(only enter to change)",
    settings_email_password_placeholder_none: "(leave blank if no authentication)",
    settings_email_password_hint: "Enter only to change the stored password. Leave blank to keep the existing value.",
    settings_email_base_url_hint: "Base URL used in reset emails. May differ from the issuer URL.",
    settings_email_save_button: "Save settings",
    settings_email_test_section: "Connection test",
    settings_email_test_lede: "Attempts a connection and authentication to the SMTP server using current settings. No email is sent.",

    // Navigation
    nav_dashboard: "Dashboard",
    nav_users: "Users",
    nav_clients: "Clients",
    nav_signing_keys: "Signing keys",
    nav_audit: "Audit log",
    nav_settings: "Settings",
    nav_profile: "Profile",
    nav_apps: "Apps",
    nav_my_account: "My account",
    settings_tab_general: "General",
    settings_tab_advanced: "Advanced",
    me_overview_last_login: "You last signed in on {date}.",
    me_overview_first_login: "Welcome — this is your first sign-in.",
    nav_security: "Security",
    nav_logout: "Sign out",
    a11y_skip_to_main: "Skip to main content",
    nav_aria_main: "Main navigation",
    nav_aria_signout: "Sign out",

    // Footer (RFC 050)
    footer_tagline: "🌱 sui-id · A quiet, dependable identity foundation.",
    footer_a11y_group_label: "Accessibility features",
    a11y_keyboard: "Keyboard accessible",
    a11y_screen_reader: "Screen reader friendly",
    a11y_contrast: "High contrast support",

    // Theme toggle (RFC 050)
    theme_toggle_group: "Theme",
    theme_toggle_light: "Light",
    theme_toggle_auto: "Auto",
    theme_toggle_dark: "Dark",
    theme_toggle_light_title: "Light theme",
    theme_toggle_auto_title: "Follow system",
    theme_toggle_dark_title: "Dark theme",

    // Status words (RFC 052)
    status_active: "Active",
    status_disabled: "Disabled",
    status_deleted: "Deleted",
    status_admin: "Admin",
    status_on: "On",
    status_off: "Off",
    status_in_use: "In use",
    status_retired: "Retired",
    status_published: "Published",
    status_pending: "Pending",
    status_healthy: "Healthy",
    status_unhealthy: "Unhealthy",

    // Empty placeholders (RFC 052)
    empty_dash: "—",
    empty_any: "(any)",
    empty_none: "(none)",
    empty_falls_back_redirect_uris: "(falls back to redirect_uris)",
    empty_no_email: "(no email)",
    empty_not_set: "(not set)",

    // Copy-to-clipboard button (RFC 053)
    copy_button_label: "📋 Copy",
    copy_button_label_done: "✓ Copied",
    copy_button_aria_template: "Copy {noun}",
    copy_noun_client_id: "Client ID",
    copy_noun_client_secret: "client secret",
    copy_noun_jwks_uri: "JWKS URI",
    copy_noun_redirect_uri: "redirect URI",
    copy_noun_audit_row_id: "audit row ID",
    copy_noun_setup_token: "setup token",
    copy_noun_recovery_code: "recovery code",
    copy_noun_passkey_id: "passkey ID",

    // Login
    signed_out_flash: "Signed out.",
    login_title: "Sign in",
    login_username_label: "Username",
    login_password_label: "Password",
    login_submit: "Sign in",
    login_passkey_button: "Sign in with passkey",
    login_no_admin_access: "This account does not have access to the admin panel.",
    login_passkey_primary: "Sign in with passkey",
    login_forgot_password_link: "Forgot your password?",
    login_invalid_credentials: "The username or password is incorrect.",
    login_account_locked: "Your account is temporarily locked. Please try again later.",
    login_reset_ok_banner: "Your password has been reset. Please sign in with the new password.",

    // Setup wizard
    setup_step_welcome: "Welcome",
    setup_step_admin: "Create admin",
    setup_step_done: "Done",
    setup_welcome_title: "Welcome to sui-id",
    setup_welcome_lede: "This server has not been initialized yet. Setup will only take a few minutes.",
    setup_welcome_lede2: "On the next screen you'll create the first administrator account. Please have the setup token (printed when the server started) ready.",
    setup_welcome_begin: "Begin setup",
    setup_admin_title: "Create the administrator account",
    setup_admin_lede: "Enter the setup token printed at server startup, plus the details for the new administrator account.",
    setup_admin_token_label: "Setup token",
    setup_admin_token_hint: "Printed once in the startup log",
    setup_admin_username_label: "Username",
    setup_admin_email_label: "Email address (optional)",
    setup_admin_email_hint: "Used for notifications and password resets. You can change it later.",
    setup_admin_display_label: "Display name (optional)",
    setup_admin_password_label: "Password",
    setup_admin_password_hint: "12 characters or more",
    setup_admin_confirm_label: "Password (confirm)",
    setup_admin_submit: "Create administrator",
    setup_done_title: "Setup complete",
    setup_done_lede: "The administrator account has been created and the initial signing key issued. You can now review system configuration in the admin console.",
    setup_done_next_steps_title: "Next steps",
    setup_done_next_step_register_clients: "Register an OIDC client.",
    setup_done_next_step_enable_mfa: "Enable two-factor authentication with a passkey or authenticator app.",
    setup_done_next_step_review_settings: "Review the current effective configuration on the Settings tab.",
    setup_done_enter_admin: "Go to admin console",
    setup_not_complete_title: "Setup is not complete",
    setup_not_complete_lede: "The administrator account has not been created yet. Please start setup from the beginning.",
    setup_password_mismatch: "The password and the confirmation do not match.",
    setup_already_initialized: "The server is already initialized.",
    setup_invalid_token: "The setup token is incorrect.",
    setup_hibp_blocked: "This password appears in a known breach. Please choose a different one.",
    setup_generic_failure: "Setup failed. Please review the form and try again.",

    // RFC 012: Setup wizard — Language step
    setup_step_lang: "Language",
    setup_lang_title: "Display Language",
    setup_lang_lede: "Choose the language for the admin panel and login screens. You can change this later in the settings.",
    setup_lang_field_label: "Display language",
    setup_lang_default_note: "You can change this later in the admin panel settings.",
    setup_lang_submit: "Next",

    // RFC 012: Setup wizard — HIBP step
    setup_step_hibp: "Security",
    setup_hibp_step_title: "Password Security Policy",
    setup_hibp_step_lede: "Use Have I Been Pwned to detect known breached passwords. Only the first 5 characters of the SHA-1 hash are sent, so your privacy is preserved.",
    setup_hibp_option_off: "Off",
    setup_hibp_option_off_desc: "No breach checking.",
    setup_hibp_option_warn: "Warn (recommended)",
    setup_hibp_option_warn_desc: "Warn about breached passwords but still allow them.",
    setup_hibp_option_block: "Block",
    setup_hibp_option_block_desc: "Reject passwords found in known breach data.",
    setup_hibp_step_default_note: "You can change this later in the admin panel settings.",
    setup_hibp_step_submit: "Next",

    // Step-up auth
    step_up_title: "Re-authenticate",
    step_up_lede: "Before performing this security-sensitive action, please confirm your identity with a code from your authenticator app. Valid for 5 minutes.",
    step_up_code_label: "Confirmation code",
    step_up_code_hint: "Six-digit code from your authenticator app, or a recovery code.",
    step_up_code_invalid: "That code is not valid. Please try again.",
    step_up_passkey_alt: "Or re-authenticate with a passkey:",
    step_up_passkey_button: "Re-authenticate with passkey",

    // MFA challenge (login flow)
    mfa_challenge_shell_title: "Verification required",
    mfa_challenge_title: "Verification code",
    mfa_challenge_lede: "Enter the 6-digit code from your authenticator app, or one of your recovery codes.",
    mfa_challenge_code_label: "Code",
    mfa_challenge_submit: "Verify",
    mfa_challenge_passkey_alt: "Or sign in with a passkey:",
    mfa_challenge_passkey_button: "Sign in with passkey",
    mfa_challenge_failed_flash: "Verification failed. Try again, or use a recovery code.",

    // Profile / self-service security (v0.29.1)
    profile_subtitle_template: "Security settings for {username}",
    profile_recovery_save_now: "Save these recovery codes now. They will not be shown again.",
    profile_recovery_save_lede: "Each code can be used once. Keep them somewhere safe. If you lose access to your authenticator app, you can enter any one of these codes in place of the 6-digit code to sign in.",
    profile_mfa_section: "Two-factor authentication",
    profile_mfa_totp_card_title: "Authenticator app (TOTP)",
    profile_mfa_status_label: "Status:",
    profile_mfa_status_enabled: "Enabled",
    profile_mfa_status_not_configured: "Not configured",
    profile_mfa_regenerate_codes: "Regenerate recovery codes",
    profile_mfa_disable_confirm: "Disable authenticator-app two-factor authentication?",
    profile_mfa_disable_button: "Disable TOTP",
    profile_mfa_enroll_lede: "When enabled, sign-in will additionally require a 6-digit code from your authenticator app.",
    profile_mfa_enroll_apps_note: "Any standards-compliant TOTP app will work (Aegis / FreeOTP / Google Authenticator / 1Password and others).",
    profile_mfa_enroll_button: "Set up TOTP",
    profile_passkeys_section: "Passkeys",
    profile_passkeys_lede: "Passkeys are hardware-backed credentials stored on your phone, PC, security key, or password manager. They never leave the device. You can register more than one — we recommend at least two as a backup.",
    profile_passkeys_th_name: "Name",
    profile_passkeys_th_registered: "Registered",
    profile_passkeys_th_last_used: "Last used",
    profile_passkeys_last_used_never: "(never used)",
    profile_passkeys_delete_confirm: "Delete this passkey? You will no longer be able to sign in with it.",
    profile_passkeys_delete_button: "Delete",
    profile_passkeys_empty: "No passkeys registered.",
    profile_passkeys_register_section: "Register a new passkey",
    profile_passkeys_nickname_label: "Nickname",
    profile_passkeys_nickname_hint: "e.g. YubiKey 5C / MacBook Touch ID",
    profile_passkeys_register_button: "Register passkey",
    profile_lang_section: "Display language",
    profile_lang_lede: "The language sui-id uses for screens after sign-in. \"Browser default\" follows your browser's Accept-Language setting.",
    profile_lang_field_label: "Language",
    profile_mfa_enrolled_flash: "Two-factor authentication is now enabled.",
    profile_recovery_regenerated_flash: "Recovery codes regenerated. Save the new ones — the old ones no longer work.",

    // MFA setup (TOTP enrollment)
    mfa_setup_shell_title: "Set up two-factor authentication",
    mfa_setup_title: "Set up two-factor authentication",
    mfa_setup_lede: "Link an authenticator app to sui-id.",
    mfa_setup_steps_title: "Steps",
    mfa_setup_step1: "Open your authenticator app and scan the QR code below. To type the secret instead, copy it from the field below.",
    mfa_setup_step2: "Enter the 6-digit code shown by the app in the form below to verify the link.",
    mfa_setup_step3: "After confirmation, sui-id will issue 8 single-use recovery codes. Save them somewhere safe.",
    mfa_setup_qr_card_title: "QR code and secret",
    mfa_setup_secret_label: "Secret:",
    mfa_setup_otpauth_summary: "otpauth URI (advanced)",
    mfa_setup_verify_card_title: "Verify",
    mfa_setup_code_label: "Verification code",
    mfa_setup_code_hint: "The 6-digit code shown by your app",
    mfa_setup_confirm_button: "Confirm and enable",

    // Forgot password
    forgot_password_title: "Forgot your password",
    forgot_password_lede: "Enter your registered email address. If an account exists, we'll send you a reset link.",
    forgot_password_email_label: "Email address",
    forgot_password_submit: "Send reset link",
    forgot_password_sent_title: "Email sent",
    forgot_password_sent_lede: "Request received. If an account exists for the address you provided, we have sent a reset link.",
    forgot_password_sent_lede2: "The link is valid for 30 minutes. If you don't see it, check your spam folder.",
    reset_password_title: "Reset your password",
    reset_password_lede: "Enter your new password twice.",
    reset_password_new_label: "New password",
    reset_password_new_hint: "12 characters or more",
    reset_password_confirm_label: "Confirm again",
    reset_password_submit: "Change password",
    reset_password_invalid_title: "This link is no longer valid",
    reset_password_invalid_lede: "The reset link has expired, has already been used, or is invalid.",
    reset_password_invalid_request_again: "Request a new link",
    password_mismatch_flash: "The password and the confirmation do not match.",
    reset_password_failed_flash: "Password reset failed. Please try again.",
    back_to_login: "Back to log in",

    // Settings hub
    settings_title: "Settings",
    settings_lede: "Review the current effective configuration.",
    settings_tab_basic: "Basic",
    settings_tab_security: "Security",
    settings_tab_authentication: "Authentication",
    settings_tab_logs: "Logs",
    settings_tab_email: "Email",
    settings_tab_other: "Other",
    settings_basic_section: "Basic",
    settings_basic_oidc_section: "OIDC public endpoints",
    settings_basic_issuer: "Issuer",
    settings_basic_listen_addr: "Listen address",
    settings_basic_cookie_secure: "Cookie Secure flag",
    settings_basic_trusted_proxies: "Trusted proxies",
    settings_basic_trusted_proxies_none: "(none — peer IP is trusted directly)",
    settings_basic_default_lang: "Server default language",
    settings_basic_save: "Save",
    settings_basic_saved: "Server settings updated.",

    // Profile
    profile_title: "Profile",
    profile_username_label: "Username",
    profile_email_label: "Email address",
    profile_display_name_label: "Display name",
    profile_lang_label: "Display language",
    profile_lang_hint: "Choose \"Browser default\" to follow your browser's language setting.",
    profile_lang_browser_default: "Browser default",
    profile_save: "Save",
    profile_saved: "Profile updated.",

    // Password change
    password_change_title: "Change password",
    password_change_lede: "Enter your current password, and the new password twice.",
    password_change_current_label: "Current password",
    password_change_new_label: "New password",
    password_change_new_hint: "12 characters or more. A long random passphrase is safer than a short complicated one.",
    password_change_confirm_label: "New password (confirm)",
    password_change_revoke_others_label: "Sign out other browsers / apps after changing the password",
    password_change_revoke_others_hint: "Recommended. Existing sessions and refresh tokens are invalidated; you will need to sign in again with the new password.",
    password_change_submit: "Change password",
    password_change_wrong_current: "Current password is incorrect.",
    password_change_done_flash: "Password changed.",

    // /me/security
    me_security_title: "Account security",
    me_security_signed_in_as_suffix: " is currently signed in.",
    me_security_admin_link: "Open admin console →",
    me_security_mfa_section: "Two-factor authentication",
    me_security_mfa_status_label: "Status:",
    me_security_mfa_status_enabled: "enabled",
    me_security_mfa_factor_totp: "authenticator app",
    me_security_mfa_factor_passkey_n: "{n} passkey(s)",
    me_security_mfa_disabled_title: "Two-factor authentication is disabled.",
    me_security_mfa_disabled_lede: "This account is currently protected by a password alone. Registering a passkey or an authenticator app is strongly recommended.",
    me_security_mfa_manage: "Manage factors",
    // Self-service MFA tab extensions (RFC 055, 056)
    me_security_mfa_recovery_section_label: "Recovery codes",
    me_security_mfa_recovery_codes_remaining: |n| format!("{n} remaining"),
    me_security_language_saved_banner: "Language preference saved.",
    setup_steps_aria: "Setup steps",
    me_security_tabs_aria: "Security sections",
    settings_tabs_aria: "Settings tabs",
    me_security_password_change_link: "Change password",
    me_security_sessions_section: "Where you are signed in",
    me_security_sessions_lede: "Each row is one browser session. Pressing Revoke immediately signs that browser out. The row marked \"current session\" is the one you are using now.",
    me_security_sessions_th_started: "Started",
    me_security_sessions_th_expires: "Expires",
    me_security_sessions_th_factors: "Factors",
    me_security_sessions_current_badge: "current session",
    me_security_sessions_revoke: "Revoke",
    me_security_sessions_revoke_confirm: "Sign out this session?",
    me_security_sessions_revoke_all_others: "Sign out all other sessions",
    me_security_sessions_revoke_all_others_confirm: "Sign out all sessions except the current one?",
    me_security_activity_section: "Recent activity",
    me_security_activity_lede: "Authentication and administrative events for your account. If you don't recognise something here, change your password and sign out other sessions immediately.",
    me_security_activity_th_when: "When",
    me_security_activity_th_event: "Event",
    me_security_activity_th_result: "Result",
    me_security_activity_th_note: "Note",

    // Email subjects/bodies
    email_subject_password_reset: "Reset your password — sui-id",
    email_subject_password_changed: "Your password was changed — sui-id",
    email_greeting_suffix: "",
    email_password_reset_intro: "We received a password-reset request for your sui-id account. Use the link below within 30 minutes to set a new password.",
    email_password_reset_link_label: "Reset your password",
    email_password_reset_disregard: "If you didn't request this, you can ignore this email.",
    email_password_changed_intro: "The password for your sui-id account was changed.",
    email_password_changed_security_warning: "If you didn't do this, please revoke other sessions immediately and contact support.",
    email_password_changed_link_security: "security settings",

    // Errors
    error_generic_title: "Error",
    error_not_found_title: "Page not found",
    error_not_found_lede: "The page you were looking for doesn't exist or has moved.",
    error_internal: "An unexpected error occurred.",
    error_too_many_requests_label: "Too many requests. Please wait a moment before trying again.",

    // Audit
    audit_title: "Audit log",
    audit_col_when: "When",
    audit_col_actor: "Actor",
    audit_col_action: "Action",
    audit_col_target: "Target",
    audit_col_outcome: "Outcome",
    audit_col_note: "Note",

    // Settings tab (RFC 023: "Other" renamed to "Advanced")

    // Audit event labels (RFC 002 § D)
    audit_event_auth_login_success: "Login",
    audit_event_auth_login_failure: "Login failed",
    audit_event_auth_login_locked: "Account locked",
    audit_event_auth_login_mfa_required: "MFA required",
    audit_event_auth_logout: "Logout",
    audit_event_auth_mfa_success: "MFA verified",
    audit_event_auth_mfa_failure: "MFA failed",
    audit_event_auth_password_changed_self: "Password changed",
    audit_event_auth_password_reset_requested: "Password reset requested",
    audit_event_auth_password_reset_email_sent: "Reset email sent",
    audit_event_auth_password_reset_email_failed: "Reset email failed",
    audit_event_auth_password_reset_throttled: "Reset throttled",
    audit_event_auth_password_reset_completed: "Password reset",
    audit_event_auth_refresh_theft_detected: "Token theft detected",
    audit_event_auth_session_revoked: "Session revoked",
    audit_event_auth_sessions_bulk_revoke_self: "All other sessions revoked",
    audit_event_auth_smtp_config_changed: "SMTP config changed",
    audit_event_user_create: "User created",
    audit_event_user_delete: "User deleted",
    audit_event_user_reset_password: "Password reset (admin)",
    audit_event_admin_user_unlock: "Account unlocked",
    audit_event_client_create: "Client created",
    audit_event_client_update: "Client updated",
    audit_event_client_delete: "Client deleted",
    audit_event_client_set_allowed_scopes: "Client scopes updated",
    audit_event_signing_key_rotate: "Signing key rotated",
    audit_event_signing_key_delete: "Signing key deleted",
    audit_event_admin_master_key_rotated: "Master key rotated",
    audit_event_setup_create_initial_admin: "Initial admin created",

    // Admin: Dashboard (RFC 029)
    dashboard_title: "Dashboard",
    dashboard_lede: "System overview and recent activity.",
    dashboard_stat_users: "Users",
    dashboard_stat_clients: "Clients",
    dashboard_stat_sessions: "Active sessions",
    dashboard_stat_service_status: "Service status",
    dashboard_stat_service_ok: "Running",
    dashboard_activity_title: "Sign-in activity",
    dashboard_activity_period: "Period",
    // Dashboard extensions (RFC 051)
    dashboard_greeting: |u| format!("Hello, {u}."),
    dashboard_aria_stats: "Statistics",
    dashboard_aria_action_required: "Operator action required",
    dashboard_action_required_title: "Action required",
    dashboard_activity_success: "Success",
    dashboard_activity_failure: "Failure",
    dashboard_activity_hover_hint: "Hover a bucket to see its details.",
    dashboard_oidc_endpoint_discovery: "Discovery",
    dashboard_oidc_endpoint_jwks: "JWKS",
    dashboard_sparkline_aria: "Sign-in activity sparkline",
    dashboard_sparkline_tooltip: |label, success, failure| {
        format!("{label} : success {success} / failure {failure}")
    },

    // Dashboard operator prompts (RFC 031)
    dashboard_warn_smtp: "Forgot-password email is disabled. Configure SMTP in Settings → Email to enable it.",
    dashboard_warn_hibp: "Password breach checking is off. Enable it in Settings → Authentication.",
    dashboard_warn_cookie_insecure: "Cookie Secure flag is off. Set cookie_secure = true in production (Settings → Security).",
    dashboard_warn_admins_no_mfa: |n| format!("{n} admin account(s) without MFA."),
    dashboard_warn_old_signing_key: |age| {
        format!("Oldest signing key is {age} days old — rotation recommended.")
    },
    dashboard_warn_outbox_stuck: |n| format!("{n} email(s) have been queued for over an hour."),
    dashboard_warn_pending_resets: |n| format!("{n} password-reset link(s) outstanding."),
    dashboard_getting_started_title: "Getting Started",
    dashboard_getting_started_smtp: "Configure SMTP so users can receive password-reset emails",
    dashboard_getting_started_first_app: "Add your first OIDC application",
    dashboard_getting_started_admin_mfa: "Enable MFA on your admin account",

    // Admin: Users (RFC 029)
    users_title: "Users",
    users_lede: "Create and manage user accounts.",
    users_create_section: "Add a new user",
    users_create_button: "Create user",
    users_table_section: "User list",
    users_table_th_status: "Status",
    users_table_th_mfa: "MFA",
    users_is_admin_label: "Grant admin privileges",
    // Users extensions (RFC 051)
    users_count_caption: |n| format!("{n} registered."),
    users_label_username: "Username",
    users_label_display_name: "Display name (optional)",
    users_label_email: "Email address (optional)",
    users_label_password: "Password (12 or more characters)",

    // Admin: Clients (RFC 029)
    // Admin: Client edit (RFC 051)
    client_edit_title: "Edit client",
    client_edit_basic_section: "Basic information",
    client_edit_basic_note: "Client ID, kind (confidential/public) and client secret are fixed at creation. To change these, delete the client and register a new one.",
    client_edit_new_secret_label: "New client secret (shown once):",
    client_edit_label_client_id: "Client ID",
    client_edit_label_kind: "Kind",
    client_edit_label_status: "Status",
    client_edit_post_logout_hint: "One per line. Leave blank to reuse Redirect URIs.",

    // Admin: Clients (RFC 029)
    clients_title: "Clients",
    clients_lede: "Register and manage OIDC clients.",
    clients_create_section: "Register a new client",
    clients_table_section: "Registered clients",
    clients_secret_once_banner: "The client secret is shown only once. Save it somewhere safe.",
    clients_table_th_name: "Name",
    clients_table_th_kind: "Type",
    clients_table_th_status: "Status",
    clients_single_realm_note: "sui-id is a single-realm IdP. All users can access all clients. Scopes restrict what information a client can request, not which users can log in.",
    // Clients page extensions (RFC 051)
    clients_table_th_client_id: "Client ID",
    clients_count_caption: |n| format!("{n} registered."),
    clients_label_app_name: "Application name",
    clients_label_redirect_uris: "Redirect URIs",
    clients_hint_redirect_uris: "One per line. https or loopback http only.",
    clients_label_allowed_scopes: "Allowed scopes",
    clients_hint_scopes_intro: "Space-separated. Known scopes: ",
    clients_hint_scopes_openid_note: " (required) · ",
    clients_hint_scopes_profile_note: " (name, language) · ",
    clients_hint_scopes_email_note: " (email) · ",
    clients_hint_scopes_offline_note: " (refresh tokens).",
    clients_hint_scopes_default: " Leaving this blank defaults to openid profile email.",
    clients_label_post_logout_uris: "Post-logout redirect URIs (optional)",
    clients_hint_one_per_line: "One per line.",
    clients_label_confidential_checkbox: "Confidential client (issues a client secret)",
    clients_button_register: "Register",

    // Admin: Audit log (RFC 029)
    audit_lede: "Administrative operation history (newest first).",

    // Audit log enhancements (RFC 033)
    audit_chain_ok: "Audit chain verified.",
    audit_chain_broken: "Audit chain integrity check failed — investigate immediately.",
    audit_filter_label: "Filter by event",
    audit_filter_placeholder: "e.g. auth.login",
    audit_export_csv: "Export CSV",
    // Audit log extensions (RFC 051)
    audit_entry_count_caption: |n| format!("({n})"),
    audit_filter_button: "Filter",
    audit_chain_broken_note: |seq| {
        format!("Mismatch detected at seq={seq}. Investigate immediately.")
    },
    audit_chain_ok_note: |checked, legacy| {
        format!("Last {checked} rows inspected. Legacy unhashed rows (pre-v0.17): {legacy}")
    },

    // Admin: Signing keys (RFC 029)
    signing_keys_title: "Signing keys",
    signing_keys_lede: "Ed25519 signing keys for JWT issuance.",
    signing_keys_rotate_section: "Key rotation",
    signing_keys_rotate_button: "Rotate signing key",
    signing_keys_table_section: "All keys",
    signing_keys_th_algorithm: "Algorithm",
    signing_keys_th_status: "Status",
    signing_keys_th_created: "Created",
    signing_keys_th_retired: "Retired",
    signing_keys_in_use_badge: "(active)",
    // Signing keys extensions (RFC 051)
    signing_keys_count_caption: |n| format!("{n} registered."),
    signing_keys_th_key_id: "Key ID",
    signing_keys_rotate_explanation_1: "Rotating issues a new signing key and moves the current one to the retired state.",
    signing_keys_rotate_explanation_2: "Retired keys stay in JWKS so tokens issued before rotation remain verifiable until they expire.",
    signing_keys_rotate_explanation_3: "Once those tokens have expired, you can safely delete the retired key from this page.",

    // Dangerous operation confirmation screens (RFC 030)
    confirm_cancel: "Cancel",
    badge_recoverable: "Recoverable",
    badge_not_recoverable: "Not recoverable",
    confirm_disable_title: "Disable user?",
    confirm_disable_impact: "This user will be unable to sign in until re-enabled.",
    confirm_disable_reversibility: "This can be undone from the user list.",
    confirm_disable_button: "Disable user",
    confirm_enable_title: "Re-enable user?",
    confirm_enable_button: "Enable user",
    confirm_delete_user_title: "Delete user?",
    confirm_delete_user_impact: "This user will be permanently removed from the user list. Their audit history is preserved.",
    confirm_delete_user_reversibility: "This cannot be undone from the admin panel.",
    confirm_delete_user_button: "Delete user",
    confirm_reset_mfa_title: "Reset two-factor authentication?",
    confirm_reset_mfa_impact: "The user's TOTP authenticator and all passkeys will be removed. They will need to re-enrol at next sign-in.",
    confirm_reset_mfa_reversibility: "The user can re-enrol after this action.",
    confirm_reset_mfa_button: "Reset MFA",
    confirm_delete_client_title: "Delete client?",
    confirm_delete_client_impact: "This OIDC client will be permanently removed. All active sessions and refresh tokens issued to this client will be revoked.",
    confirm_delete_client_reversibility: "This cannot be undone.",
    confirm_delete_client_button: "Delete client",
    confirm_delete_signing_key_title: "Delete signing key?",
    confirm_delete_signing_key_impact: "Tokens signed by this key that have not yet expired will fail verification immediately.",
    confirm_delete_signing_key_reversibility: "This cannot be undone. Only delete keys whose tokens have all expired.",
    confirm_delete_signing_key_button: "Delete signing key",
    error_403_auditor_title: "Read-only access",
    error_403_auditor_body: "Your account has read-only (auditor) access. This action requires administrator privileges.",
    client_detail_readonly_title: "App details",
    confirm_rotate_signing_key_title: "Rotate signing key",
    confirm_rotate_signing_key_impact: "A new signing key will be issued. Existing tokens signed with the previous key remain valid until they expire.",
    confirm_rotate_signing_key_reversibility: "This cannot be undone. The previous key will be retired.",
    confirm_rotate_signing_key_button: "Rotate key",
    confirm_email_settings_title: "Confirm email settings",
    confirm_email_settings_impact: "The following email settings will be saved:",
    confirm_email_settings_button: "Save settings",
    login_title_admin: "Sign in to manage sui-id",
    login_body_admin: "Use an administrator or auditor account.",
    login_title_self_service: "Sign in to manage your security",
    login_body_self_service: "Manage MFA, passkeys, sessions, and password.",
    login_body_oidc: "sui-id will verify your identity for this application.",
    theme_noscript_note: "Theme follows your system setting.",
    empty_users: "No users yet.",
    empty_users_cta: "Create first user",
    empty_clients: "No applications registered yet.",
    empty_clients_cta: "Register first application",
    empty_signing_keys: "No signing keys found.",
    empty_audit: "No audit events yet.",
    error_summary_heading: "Please fix the following:",
    button_edit: "Edit",
    button_view_detail: "View details",

    // Admin: User detail (RFC 035)
    user_detail_back: "← Back to users",
    user_detail_auth_section: "Authentication",
    user_detail_totp_label: "Authenticator app (TOTP):",
    user_detail_passkeys_label: "Passkeys:",
    user_detail_sessions_section: "Active sessions",
    user_detail_sessions_th_started: "Started",
    user_detail_sessions_th_expires: "Expires",
    user_detail_sessions_th_factors: "Factors",
    user_detail_activity_section: "Recent activity",
    user_detail_danger_zone_body: "These actions affect this user's access and may be permanent. Each leads to a confirmation step.",
    role_admin: "Admin",
    role_auditor: "Auditor",
    role_user: "User",
    user_detail_role_section: "Access role",
    user_detail_role_change: "Change role",
    user_detail_role_saved: "Role updated.",
    user_detail_role_last_admin: "Cannot demote the last admin.",

    // Settings section keys
    settings_page_title_template: "Settings",
    settings_basic_description: "Review the current effective configuration. To change values, edit sui-id.toml and restart.",
    // Settings: Basic extensions (RFC 051)
    settings_basic_default_lang_hint: "Used as a fallback when no per-user preference is set and Accept-Language does not match a supported locale.",
    settings_basic_kv_issuer: "Issuer",
    settings_basic_kv_listen: "Listen address",
    settings_basic_kv_cookie_secure: "Cookie Secure flag",
    settings_basic_kv_trusted_proxies: "Trusted proxies",
    settings_security_session_section: "Session limits",
    settings_security_session_lede: "Idle timeout and concurrent session cap. Both default to 0 (disabled); opt in per your policy.",
    settings_security_idle_timeout_label: "Idle timeout (seconds)",
    settings_security_max_sessions_label: "Max concurrent sessions per user",
    settings_security_lockout_section: "Account lockout",
    settings_security_headers_section: "Security headers",
    // Settings: Security extensions (RFC 051)
    settings_security_idle_timeout_hint: "0 disables. 0 < N ≤ 2,592,000 (= 30 days).",
    settings_security_max_sessions_hint: "0 disables. 1 ≤ N ≤ 1000. When exceeded, the oldest session is auto-revoked (FIFO).",
    settings_security_lockout_hint_1: "Cap on progressive backoff. Failed-attempt lockout time will never exceed this value.",
    settings_security_lockout_hint_2_pre: "Administrators can clear a lockout at any time via the ",
    settings_security_lockout_hint_2_post: " command.",
    settings_security_headers_perm_policy_label: "Permissions-Policy (minimal)",
    settings_security_headers_hint: "All admin pages return the headers above. /oauth2/* public endpoints omit some headers where the protocol requires it.",
    settings_security_cors_token_label: "Token endpoint dynamic allow-list (origins of registered redirect_uris)",
    settings_security_cors_public_label: "Discovery / JWKS / userinfo open to all origins (*)",
    settings_auth_password_section: "Password",
    settings_auth_mfa_section: "Two-factor authentication",
    settings_auth_oidc_section: "OIDC / token settings",
    settings_logs_output_section: "Log output",
    settings_logs_audit_section: "Audit log hash-chain",
    settings_advanced_build_section: "Build info",
    settings_advanced_storage_section: "Storage",
    settings_advanced_record_counts: "Record counts",

    // OIDC consent screen (RFC 038)
    consent_title: "Authorize access",
    consent_app_wants_access: "wants access to:",
    consent_scope_openid: "Verify your identity",
    consent_scope_profile: "Your profile (name, language)",
    consent_scope_email: "Your email address",
    consent_scope_offline_access: "Stay signed in (refresh tokens)",
    consent_scope_openid_desc: "Confirms your sign-in and provides a unique identifier.",
    consent_scope_profile_desc: "Name, preferred language, and timezone.",
    consent_scope_email_desc: "Email address and whether it has been verified.",
    consent_scope_offline_access_desc: "Keeps the app signed in on your behalf when you are not present.",
    consent_approve: "Allow",
    consent_deny: "Deny",
    consent_policy_label: "Consent policy",
    consent_policy_none: "None (skip consent screen)",
    consent_policy_first_time: "First time only",
    consent_policy_always: "Always prompt",

    // Settings: page titles (RFC 039)
    settings_title_basic: "Settings — Basic",
    settings_title_security: "Settings — Security",
    settings_title_authentication: "Settings — Authentication",
    settings_title_logs: "Settings — Logs",
    settings_title_email: "Settings — Email",
    settings_title_advanced: "Settings — Advanced",
    // Settings: auth tab body (RFC 039)
    settings_auth_min_length_label: "Minimum length",
    settings_auth_hash_algorithm_label: "Hash algorithm",
    settings_auth_mfa_totp: "TOTP (authenticator app)",
    settings_auth_mfa_passkey: "WebAuthn (passkey)",
    settings_auth_mfa_recovery_label: "Recovery codes per enrollment",
    settings_auth_access_token_ttl: "Access token lifetime",
    settings_auth_id_token_ttl: "ID token lifetime",
    settings_auth_refresh_token_ttl: "Refresh token lifetime",
    settings_auth_refresh_rotate: "Refresh token rotation",
    settings_auth_refresh_theft: "Refresh theft detection (family revoke)",
    settings_auth_pkce_required: "PKCE required (all clients, all flows)",
    // Settings: logs tab body (RFC 039)
    settings_logs_recent_24h: "Events in the last 24 hours",
    settings_logs_chain_broken_note: "Hash-chain mismatch detected. Investigate immediately.",
    settings_logs_chain_ok_note: "Hash-chain integrity OK.",
    // Settings: advanced tab body (RFC 039)
    settings_advanced_version_label: "sui-id version",
    settings_advanced_schema_label: "Schema version",
    settings_advanced_server_time_label: "Server time",
    settings_advanced_db_file_label: "Database file",
    settings_advanced_key_file_label: "Master key file",
    settings_advanced_manage_link: "Manage →",
    // Settings: email tab body (RFC 039)
    settings_email_page_title: "Settings — Email",
    settings_email_lede: "SMTP settings for password-reset and change notification emails.",
    settings_email_smtp_section: "SMTP connection",
    settings_email_enable_label: "Enable email delivery",
    settings_email_host_label: "SMTP host",
    settings_email_port_label: "Port",
    settings_email_port_hint: "587 (STARTTLS) or 465 (implicit TLS) are common.",
    settings_email_tls_label: "TLS mode",
    settings_email_tls_implicit: "Implicit TLS (465)",
    settings_email_username_label: "Username (optional)",
    settings_email_from_addr_label: "From address",
    settings_email_from_name_label: "From display name (optional)",
    settings_email_base_url_label: "Public base URL",
    settings_email_test_button: "Test connection",
    settings_email_test_hint: "Attempts SMTP connection with current settings. No mail is sent.",

    // Error pages (RFC 042)
    error_generic_lede: "We could not process the request.",
    error_request_id_label: "Request ID",
    error_back_home: "Back to home",

    // RFC 042
    error_internal_lede: "Something went wrong. Please contact the server administrator.",

    // RFC 042
    error_too_many_requests_lede: "Please wait a moment and try again.",

    dashboard_recent_events_title: "Recent important events",

    dashboard_recent_events_empty: "No important events.",

    dashboard_recent_events_view_all: "View all →",

    // /me/security tabs (RFC 040)
    me_tab_overview: "Overview",
    me_tab_password: "Password",
    me_tab_apps: "Apps",
    me_apps_title: "Authorized applications",
    me_apps_intro: "Apps that can sign in as you. Revoke access at any time.",
    me_apps_granted_on: "Granted",
    me_apps_last_used: "Last used",
    me_apps_never_used: "Never used",
    me_apps_revoke_button: "Revoke access",
    me_apps_revoked: "Access revoked. The app will need to ask for permission again.",
    me_apps_empty: "You have not authorized any applications.",
    me_tab_mfa: "MFA",
    me_tab_passkey: "Passkeys",
    me_tab_sessions: "Sessions",
    me_tab_language: "Language",
    me_overview_section_status: "Security status",
    me_overview_section_activity: "Recent activity",
    me_overview_label_mfa_totp: "MFA (TOTP)",
    me_overview_label_passkeys: "Passkeys",
    me_overview_no_recent_events: "No recent activity to display.",
    setup_welcome_lang_picker_label: "Language picker",
    me_passkey_origin_warning: "Passkeys require HTTPS or localhost.",
    me_passkey_section_title: "Registered passkeys",
    me_passkey_button_rename: "Rename",
    me_passkey_nickname_label: "Nickname",
    me_passkey_nickname_placeholder: "e.g. YubiKey 5C",
    me_language_title: "Display language",
    me_language_lede: "Set your preferred language. If not set, your browser settings or the server default will be used.",
    me_language_use_default: "System default (Cookie / Accept-Language)",
    me_language_saved_flash: "Language preference saved.",

    disable_reason_label: "Reason for disabling (optional)",

    disable_reason_placeholder: "e.g. Left company, suspicious activity",

    disable_reason_hint: "Recorded in the audit log so future administrators can understand the context.",
};

// ── Formatters ───────────────────────────────────────────────────────────────

const EN_MONTHS: &[&str] = &[
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn en_fmt_date(dt: DateTime<Utc>) -> String {
    format!(
        "{} {} {}",
        dt.day(),
        EN_MONTHS[(dt.month() - 1) as usize],
        dt.year()
    )
}

fn en_fmt_date_time(dt: DateTime<Utc>) -> String {
    format!("{} {}", en_fmt_date(dt), fmt_time_shared(dt))
}

fn en_fmt_relative(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - at).num_seconds();
    if secs < 0 {
        return "just now".into();
    }
    if secs < 60 {
        let s = if secs == 1 { "second" } else { "seconds" };
        return format!("{secs} {s} ago");
    }
    let mins = secs / 60;
    if mins < 60 {
        let s = if mins == 1 { "minute" } else { "minutes" };
        return format!("{mins} {s} ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        let s = if hours == 1 { "hour" } else { "hours" };
        return format!("{hours} {s} ago");
    }
    let days = hours / 24;
    if days < 30 {
        let s = if days == 1 { "day" } else { "days" };
        return format!("{days} {s} ago");
    }
    let months = days / 30;
    if months < 12 {
        let s = if months == 1 { "month" } else { "months" };
        return format!("{months} {s} ago");
    }
    let years = months / 12;
    let s = if years == 1 { "year" } else { "years" };
    format!("{years} {s} ago")
}

/// English date and number formatters.
pub static FORMATTERS_EN: Formatters = Formatters {
    fmt_date: en_fmt_date,
    fmt_time: fmt_time_shared,
    fmt_date_time: en_fmt_date_time,
    fmt_relative: en_fmt_relative,
    fmt_count: fmt_count_shared,
};

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn ts(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[test]
    fn en_date_formatting() {
        let dt = ts(2024, 5, 12, 14, 7);
        assert_eq!(en_fmt_date(dt), "12 May 2024");
        assert_eq!(en_fmt_date_time(dt), "12 May 2024 14:07");
    }

    #[test]
    fn en_relative_formatting() {
        let now = ts(2024, 5, 12, 15, 0);
        assert_eq!(
            en_fmt_relative(ts(2024, 5, 12, 14, 59), now),
            "1 minute ago"
        );
        assert_eq!(
            en_fmt_relative(ts(2024, 5, 12, 14, 57), now),
            "3 minutes ago"
        );
        assert_eq!(en_fmt_relative(ts(2024, 5, 12, 12, 0), now), "3 hours ago");
        assert_eq!(en_fmt_relative(ts(2024, 5, 9, 15, 0), now), "3 days ago");
    }
}
