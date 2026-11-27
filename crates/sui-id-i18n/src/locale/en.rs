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
    settings_auth_min_length_hint: |floor| {
        format!(
            "Applies to every new password: a reset, a change, a recovery link. Existing \
         passwords keep working, and meet the new minimum at their next change. It \
         cannot be set below {floor}."
        )
    },
    settings_auth_min_length_dev_note: |dev_floor| {
        format!("Dev mode uses {dev_floor} characters, whatever this says.")
    },
    settings_auth_min_length_flash_invalid: |floor, ceiling| {
        format!("The minimum must be between {floor} and {ceiling} characters.")
    },
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
    settings_email_password_hint_change: "Enter only to change the stored password; leave it blank to keep the current one.",
    settings_email_password_hint_none: "Leave blank if the SMTP server needs no authentication.",
    settings_email_base_url_hint: "No longer used for reset links: they are built from the issuer URL (server.issuer).",
    settings_email_save_button: "Save settings",
    settings_email_test_section: "Connection test",
    settings_email_test_lede: "Attempts a connection and authentication to the SMTP server using current settings. No email is sent.",

    // Navigation
    nav_dashboard: "Dashboard",
    nav_users: "Users",
    nav_clients: "Clients",
    nav_signing_keys: "Signing keys",
    nav_federation_providers: "Federation providers",
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
    nav_group_overview: "Overview",
    nav_group_people: "People",
    nav_group_applications: "Applications",
    nav_group_sign_in: "Sign-in",
    nav_group_security: "Security",
    nav_group_system: "System",
    nav_item_sign_in_settings: "Sign-in settings",
    nav_item_sessions_and_lockout: "Sessions and lockout",
    nav_item_about_this_server: "About this server",
    nav_menu_toggle: "Menu",

    // Theme toggle (RFC 050; RFC 149 §3: in the account menu)
    theme_toggle_group: "Theme",
    theme_toggle_light: "Light",
    theme_toggle_auto: "Auto",
    theme_toggle_dark: "Dark",

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
    status_unknown: "Unknown",
    status_self_registered: "Self-registered",
    status_auth_method_unrecorded: "Auth method unrecorded",
    status_auth_method_unrecognized: "Auth method unrecognized",

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
    signed_out_flash: "You are signed out.",
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
    login_fed_retry: "Sign-in could not be completed. Please try again.",
    login_fed_contact_admin: "Sign-in could not be completed for this account or provider. Contact an administrator.",
    login_fed_link_required: "This sign-in could not be linked to a local account automatically. Contact an administrator.",
    login_fed_no_verified_email: "Your identity provider did not send a verified email address, which is needed to create your account here. Contact your administrator.",
    login_federation_divider: "or",
    login_federation_button: "Sign in with {name}",

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
    setup_admin_password_hint: |n| format!("At least {n} characters."),
    setup_admin_confirm_label: "Password (confirm)",
    setup_admin_submit: "Create administrator",
    setup_done_title: "Setup complete",
    setup_done_lede: "The administrator account has been created and the initial signing key issued. You can now review system configuration in the admin console.",
    setup_done_next_steps_title: "Next steps",
    setup_done_next_step_register_clients: "Register an OIDC client.",
    setup_done_next_step_enable_mfa: "Enable two-factor authentication with a passkey or authenticator app.",
    setup_done_next_step_review_settings: "Your dashboard lists what to set up next.",
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
    mfa_challenge_method_mismatch_flash: "This sign-in is waiting for your passkey, not a code. Use the passkey option below.",

    // Profile / self-service security (v0.29.1)
    profile_subtitle_template: "Security settings for {username}",
    profile_recovery_save_now: "Save these recovery codes now. They will not be shown again.",
    profile_recovery_save_lede: "Each code can be used once. Keep them somewhere safe. If you lose access to your authenticator app, you can enter any one of these codes in place of the 6-digit code to sign in.",
    me_mfa_lede: "Set up or remove your own two-step verification: an authenticator app (TOTP) and recovery codes.",
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
    factor_add_password_label: "Current password",
    factor_add_password_hint: "Enter your current password to add your first second factor.",
    factor_add_reauth_failed: "We could not confirm it is you. Check your password and try again.",
    factor_add_reauth_unavailable: "Your password could not be checked with your directory right now. Try again later.",
    factor_add_federated_unavailable: "Accounts that sign in through an external identity provider cannot add a second factor here yet.",
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
    forgot_password_sent_title: "Request received",
    forgot_password_sent_lede: "If an account exists for the address you provided, we have sent a reset link.",
    forgot_password_sent_lede2: "The link is valid for 30 minutes. If you don't see it, check your spam folder.",
    forgot_password_sent_lede3: "If nothing arrives within a few minutes, the address you entered may not be the one registered to an account — double-check it, or ask your administrator for help.",
    reset_password_title: "Reset your password",
    reset_password_lede: "Enter your new password twice.",
    reset_password_new_label: "New password",
    reset_password_new_hint: |n| format!("At least {n} characters."),
    reset_password_token_label: "Reset code",
    reset_password_token_hint: "Paste the reset code from the email if this field is empty.",
    reset_password_confirm_label: "Confirm again",
    reset_password_submit: "Change password",
    reset_password_invalid_title: "This link is no longer valid",
    reset_password_invalid_lede: "The reset link has expired, has already been used, or is invalid.",
    reset_password_invalid_request_again: "Request a new link",
    reset_done_title: "Your password is set",
    reset_done_lede: "You can now sign in with it. Every other session for this account was signed out.",
    reset_done_lockout_cleared: "Sign-in for this account had been refused after repeated failed attempts. That is cleared.",
    reset_done_lockout_watch: "If you are refused again soon, someone may be trying passwords for your account. Wait a few minutes and try again.",
    reset_done_second_factor_before: "Sign-in that needs a second factor is paused until ",
    reset_done_second_factor_after: ". This reset does not change that.",
    password_mismatch_flash: "The password and the confirmation do not match.",
    reset_password_failed_flash: "Password reset failed. Please try again.",
    password_too_short_flash: |n| format!("The password must be at least {n} characters."),
    password_too_long_flash: |n| format!("The password is too long: at most {n} characters."),
    back_to_login: "Back to log in",

    // Settings hub
    settings_title: "Settings",
    settings_lede: "Review the current effective configuration.",
    settings_tab_basic: "Basic",
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
    password_change_new_hint: |n| {
        format!(
            "At least {n} characters. A long random passphrase is safer than a short \
         complicated one."
        )
    },
    password_change_confirm_label: "New password (confirm)",
    password_change_revoke_others_label: "Sign out other browsers / apps after changing the password",
    password_change_revoke_others_hint: "Recommended. Existing sessions and refresh tokens are invalidated; you will need to sign in again with the new password.",
    password_change_submit: "Change password",
    password_change_wrong_current: "Current password is incorrect.",
    password_change_done_flash: "Password changed.",
    password_match_yes: "The passwords match.",
    password_match_not_yet: "The passwords do not match yet.",

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
    email_password_reset_token_hint: |page| {
        format!("If the link does not work, open {page} and enter this reset code:")
    },
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
    audit_event_signing_key_rotate: "Signing key rotated",
    audit_event_signing_key_delete: "Signing key deleted",
    audit_event_admin_master_key_rotated: "Master key rotated",
    audit_event_setup_create_initial_admin: "Initial admin created",

    // Help, layer 2: on request (RFC 149 §3 / Q3 §1)
    help_learn_more: "Learn more",
    help_about_rotation: "About rotation",
    help_rotation_body: "Rotating creates a new key that signs from then on, and retires the current one, which stays in the JWKS so that tokens it signed still verify. Delete a retired key only once every token it signed has expired: the longer of the access-token and ID-token lifetimes after rotation (both set under [tokens] in the configuration). Deleting it sooner makes those tokens fail at every application.",
    help_about_session_limits: "About session limits",
    help_session_limits_body: "The idle timeout ends a session after it sits unused for this long; 0 disables it. The concurrent-session cap evicts the oldest session, first, once a person holds more sessions than this; 0 disables the cap.",
    help_about_lockout: "About lockout",
    help_lockout_body: "Failed sign-in attempts lock an account for a progressively longer time, capped at this value, so repeated failures never lock someone out forever. An administrator can clear a lockout immediately with \"sui-id admin user unlock\".",
    help_about_min_length: "About the minimum length",
    help_min_length_body: "Every new password — set at account creation, a reset, or a self-service change — must be at least this many characters. It can be raised up to 128 and never set below 12; in dev mode it is always 8, regardless of this setting.",
    help_about_two_step: "About two-step verification",
    help_two_step_body: "Two-step verification (an authenticator app, or a passkey) is turned on per account from that person's own account page, not here as a server-wide switch. Recovery codes, issued at enrollment, let someone sign in if they lose their second factor.",
    help_about_smtp_tls: "About SMTP and TLS",
    help_smtp_tls_body: "STARTTLS (587) starts in plain text and upgrades to TLS; implicit TLS (465) is encrypted from the first byte. sui-id sends two kinds of email from this address: a password-reset link, and a notice when a password has been changed.",
    help_about_requires_review: "About \"Requires review\"",
    help_requires_review_body: "A provider needs review whenever it has not been enabled through a passed check: when it is first added, after its trust policy changes, and after it is disabled. Check and enable runs every check, and enables the provider only if all of them pass. Check only runs the same checks and changes nothing.",
    help_about_chain_check: "About the chain check",
    help_chain_check_body: "Each row's hash covers its own content and the previous row's hash, so the rows form a chain. This check recomputes the links of the most recent rows and reports the first that does not match, which reveals a row changed or removed outside sui-id. It is not proof against someone who can write to the database: they could recompute every link after the row they changed.",
    help_about_storage_paths: "About the storage paths",
    help_storage_paths_body: "The database holds every account and setting, and the master key decrypts the secrets in it: from SUI_ID_MASTER_KEY when that is set, otherwise from the key file shown here. Back up with sui-id backup create, which takes a consistent copy while the server runs and includes the key file, so add --encrypt before the backup leaves this host. A copy of the database file without its key loses every secret in it.",

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
    dashboard_warn_smtp: "Forgot-password email is disabled. Configure SMTP in System → Email to enable it.",
    dashboard_warn_hibp: "Password breach checking is off. It was chosen during setup, and the console cannot change it yet.",
    dashboard_warn_cookie_insecure: "Cookie Secure flag is off. In production, set cookie_secure = true under [server] in the configuration file.",
    dashboard_warn_admins_no_mfa: |n| format!("{n} admin account(s) without MFA."),
    dashboard_warn_old_signing_key: |age| {
        format!("Oldest signing key is {age} days old — rotation recommended.")
    },
    dashboard_warn_outbox_stuck: |n| format!("{n} email(s) have been queued for over an hour."),
    dashboard_range_24h: "Last 24 hours",
    dashboard_range_7d: "Last 7 days",
    dashboard_range_30d: "Last 30 days",
    settings_email_test_passed: "The SMTP connection test passed.",
    settings_email_test_failed: |e| format!("The SMTP connection test failed: {e}"),
    dashboard_warn_pending_resets: |n| format!("{n} password-reset link(s) outstanding."),
    dashboard_gs_title: "Getting started",
    dashboard_gs_done: "Getting started: done",
    dashboard_gs_step1: "Protect your administrator account",
    dashboard_gs_step2: "Set up email (password reset and recovery need it)",
    dashboard_gs_step3: "Add your first app",
    dashboard_gs_step4: "Review sign-in and session settings",
    dashboard_gs_step5: "Add a federation provider",
    dashboard_gs_status_done: "Done",
    dashboard_gs_status_recommended: "Recommended",
    dashboard_gs_status_optional: "Optional",
    dashboard_warn_backup_overdue: |review_at, custody_reference| {
        format!(
            "A kept pre-rotation master-key backup is past its review date ({review_at}), under custody reference {custody_reference:?}. Settle it with `sui-id admin master-key backup remove` (once retired or re-encrypted) or `sui-id admin master-key backup renew`."
        )
    },
    dashboard_warn_federation_requires_review: |n| {
        format!("{n} federation provider(s) require review before they can be enabled.")
    },

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
    users_create_needs_second_factor: "You have no second factor set up. You can create this account, but issuing the recovery link that lets the new user choose a password needs one (a passkey or an authenticator app). Set one up on your security page first.",
    users_create_password_note: "No password is set here. After you create the account you will issue a recovery link, and the user chooses their own password when they open it.",

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

    // Clients page: application-identity links (RFC 140 stage 1)
    clients_label_homepage_uri: "Homepage URL (optional)",
    clients_label_privacy_policy_uri: "Privacy policy URL (optional)",
    clients_label_tos_uri: "Terms of service URL (optional)",

    // Admin: Audit log (RFC 029)
    audit_lede: "Administrative operation history (newest first).",

    // Audit log enhancements (RFC 033)
    audit_chain_ok: "Audit chain verified.",
    audit_chain_broken: "Audit chain integrity check failed — investigate immediately.",
    audit_filter_label: "Filter by event",
    audit_filter_hint: "e.g. auth.login",
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
    // Audit chain: could-not-verify outcome (RFC 121)
    audit_chain_could_not_verify: "Audit chain could not be verified.",
    audit_chain_could_not_verify_note: |detail| {
        format!(
            "The check itself did not complete ({detail}). This is not the same as no problem \
             found — investigate the cause before trusting either answer."
        )
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
    confirm_rotate_signing_key_count_suffix: |n| {
        if n == 1 {
            "active key will become retired.".to_owned()
        } else {
            "active keys will become retired.".to_owned()
        }
    },
    confirm_rotate_signing_key_count_zero: "No active keys will become retired.",
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
    theme_noscript_note: "The theme follows your device's setting.",
    empty_users: "No users yet.",
    empty_users_cta: "Create first user",
    empty_clients: "No applications registered yet.",
    empty_clients_cta: "Register first application",
    empty_signing_keys: "No signing keys found. This should not happen -- one is created \
        automatically when setup completes.",
    empty_signing_keys_cta: "Signing key rotation",
    empty_audit: "No audit events yet.",
    error_summary_heading: "Please fix the following:",
    dynamic_reg_token_issued_flash: "Registration token issued.",
    dynamic_reg_token_revoked_flash: "Registration token revoked.",
    dynamic_reg_registered_flash: "Client registered successfully.",
    dynamic_reg_token_invalid: "Registration token is invalid, exhausted, or expired.",
    dynamic_reg_token_expired: "Registration token has expired.",
    scope_catalog_created_flash: "Scope created.",
    scope_catalog_deleted_flash: "Scope deleted.",
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
    settings_basic_default_lang_hint: "Used as a fallback when no per-user preference is set, no language cookie is set, and the browser's Accept-Language matches no supported language. Applies to every page, including this console.",
    settings_basic_kv_issuer: "Issuer",
    settings_basic_kv_listen: "Listen address",
    settings_basic_kv_cookie_secure: "Cookie Secure flag",
    settings_basic_kv_trusted_proxies: "Trusted proxies",
    settings_security_lede: "Session limits, security headers, and CORS.",
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
    settings_authentication_lede: "Password policy, multi-factor authentication, and OAuth 2.1 / OIDC tokens.",
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
    consent_legal_links_intro: "Before continuing, review:",
    consent_privacy_policy_link: "Privacy Policy",
    consent_tos_link: "Terms of Service",
    consent_policy_label: "Consent policy",
    consent_policy_none: "None (skip consent screen)",
    consent_policy_first_time: "First time only",
    consent_policy_always: "Always prompt",

    // Settings: page titles (RFC 039)
    settings_title_email: "Settings — Email",
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
    me_overview_lede: "Your account, at a glance: two-step verification, passkeys, active sessions, and recent activity.",
    me_overview_section_status: "Security status",
    me_overview_section_activity: "Recent activity",
    me_overview_label_mfa_totp: "MFA (TOTP)",
    me_overview_label_passkeys: "Passkeys",
    me_overview_no_recent_events: "No recent activity to display.",

    // Account-page recovery event line (RFC 103 5b)
    me_overview_recovery_issued_by_admin: "An administrator issued you an account-recovery link on {date}.",
    me_overview_recovery_issued_by_operator: "The host operator issued you an account-recovery link on {date}.",
    me_overview_recovery_completed_self: "You reset your password using an emailed link on {date}.",
    me_overview_recovery_completed_by_admin: "Your password was reset on {date}, using a link an administrator issued.",
    me_overview_recovery_completed_by_operator: "Your password was reset on {date}, using a link the host operator issued.",
    setup_welcome_lang_picker_label: "Language picker",
    me_passkey_lede: "The passkeys registered to your own account — add or remove them here.",
    me_passkey_origin_warning: "Passkeys require HTTPS or localhost.",
    me_passkey_section_title: "Registered passkeys",
    me_passkey_button_rename: "Rename",
    me_passkey_nickname_label: "Nickname",
    me_language_title: "Display language",
    me_language_lede: "Set your preferred language. If not set, your browser settings or the server default will be used.",
    me_language_use_default: "System default (Cookie / Accept-Language)",
    me_language_saved_flash: "Language preference saved.",

    optional_reason_label: "Reason (optional)",

    optional_reason_hint: "Recorded in the audit log so future administrators can understand the context.",

    // Administrator-issued recovery links (RFC 103)
    copy_noun_recovery_link: "recovery link",
    copy_noun_recovery_token: "recovery token",
    confirm_recovery_link_title: "Issue a recovery link?",
    confirm_recovery_link_impact: "A single-use link, valid for 30 minutes, will be created. The user opens it and chooses their own password; you will not see or set it. Any earlier link for this user stops working. Their second factor, if any, is still required at sign-in.",
    confirm_recovery_link_reversibility: "The link cannot be recalled once handed over. Issuing another link, or disabling the user, makes it stop working.",
    confirm_recovery_link_button: "Issue recovery link",
    recovery_link_handover: "Give the link to the person through a channel that authenticates them, such as a call-back to a number already on record, or in person. Never send it to an address or number the requester supplies in the same request. Hand it over immediately: it expires in 30 minutes.",
    recovery_reason_label: "Reason (required)",
    recovery_reason_hint: "Recorded in the audit log. One line, at most 200 characters.",
    recovery_link_issued_title: "Recovery link issued",
    recovery_link_issued_once: "This is the only time the link is shown. Copy it now: leaving this page does not let you see it again.",
    recovery_link_url_label: "Link",
    recovery_link_url_aria: "Recovery link",
    recovery_link_token_label: "Token",
    recovery_link_token_hint: "If a mail or chat program drops the link's fragment (the part after #), the user can paste this token on the reset page instead.",
    recovery_link_token_aria: "Recovery token",
    recovery_link_expires_label: "Expires",
    recovery_link_invalidated_label: "Earlier links invalidated",
    recovery_link_back: "Back to the user",
    recovery_refused_title: "Recovery link not issued",
    recovery_refused_unchanged: "Nothing was changed.",
    recovery_refused_reason_required: "A reason is required.",
    recovery_refused_reason_too_long: "The reason is too long: at most 200 characters, and at most 512 bytes.",
    recovery_refused_reason_control: "The reason must not contain control characters such as a newline or tab.",
    recovery_refused_target_unknown: "There is no such user.",
    recovery_refused_target_self: "You cannot issue a recovery link for your own account. Change your password on your security page, or use the forgot-password page.",
    recovery_refused_target_admin: "Administrators cannot be recovered from the admin panel. Ask the operator to run sui-id admin issue-recovery-link on the host.",
    recovery_refused_target_non_local: "This account signs in through a directory or an upstream provider, so its password is not managed here.",
    recovery_refused_target_federated: "This account is linked to a federation provider and signs in through it, not a local password. Unlink it first, or issue a recovery link after it is unlinked.",
    recovery_refused_target_disabled: "This user is disabled. Enable the account first.",
    recovery_refused_target_deleted: "This user has been deleted.",
    recovery_refused_throttled: "You have issued the most recovery links allowed in one hour. Try again later.",
    recovery_refused_needs_second_factor: "Issuing recovery links requires that you have a second factor (an authenticator app or a passkey). Set one up on your security page, then try again.",

    // RFC 096-B2 S5: federation-provider admin surface.
    federation_providers_title: "Federation providers",
    federation_providers_lede: "See each upstream identity provider's state, and enable or disable it.",
    federation_providers_th_name: "Name",
    federation_providers_th_issuer: "Issuer",
    federation_providers_th_state: "State",
    federation_providers_th_review: "Review status",
    federation_providers_th_last_checked: "Enabled after a check at",
    federation_providers_th_linked_users: "Linked users",
    federation_providers_state_enabled: "Enabled",
    federation_providers_state_disabled: "Disabled",
    federation_providers_review_validated: "Validated",
    federation_providers_review_requires_review: "Requires review",
    federation_providers_never_checked: "—",
    federation_providers_linked_yes: "Yes",
    federation_providers_linked_no: "No",
    federation_providers_not_configured_note: "No configuration block names this provider. Restore it and restart sui-id to enable it again, or delete it.",
    federation_providers_check_and_enable_button: "Check and enable",
    federation_providers_check_only_button: "Check only",
    federation_providers_disable_button: "Disable",
    empty_federation_providers: "No federation providers are configured yet. Add one in the \
        configuration file, then restart sui-id.",
    empty_federation_providers_cta: "Configuration reference",
    federation_providers_enable_confirm_title: "Check and enable this federation provider",
    federation_providers_enable_confirm_body: "sui-id will contact the upstream now, check its discovery document and signing keys, and enable the provider only if every check passes.",
    federation_providers_check_passed_flash: "Every check passed.",
    federation_providers_disable_confirm_title: "Disable this federation provider",
    federation_providers_disable_confirm_body: "Federated sign-in through this provider stops now. Sign-ins already in progress end. Users keep their links.",
    federation_providers_enabled_flash: "The provider was checked and enabled.",
    federation_providers_disabled_flash: "The provider was disabled.",
    federation_providers_already_disabled_flash: "The provider was already disabled.",
    fed_err_not_found: "This provider no longer exists. Reload the list.",
    fed_err_storage: "The provider could not be updated. Try again, and check the server log if it persists.",
    fed_err_not_disabled: "This provider is already enabled. Disable it first to check it again.",
    fed_err_delete_still_enabled: "This provider is still enabled. Disable it first, then delete it.",
    fed_err_not_in_configuration: "No [[federation_providers]] block names this provider. Add one, then restart sui-id.",
    fed_err_policy_not_recorded: "This provider's trust policy has not been recorded yet. Restart sui-id, and check the server log if it persists.",
    fed_err_transport: "The upstream identity provider could not be reached. Check this host's network, DNS, and CA certificates, then check the server log.",
    fed_err_discovery: "The upstream's discovery document was refused. Check the server log for the reason.",
    fed_err_jwks: "The upstream's signing keys were refused, or none of them is usable. Check the server log for the reason.",
    fed_err_auth_method_unsupported: "The upstream does not accept this provider's client authentication method. Check client_secret_env against the upstream's client registration.",
    fed_err_no_common_algorithm: "The upstream does not sign with any algorithm this provider accepts. Add one it supports to id_token_algs, then restart sui-id.",
    fed_err_changed_during_check: "The provider changed while the check was running. Run the check again.",
    fed_err_changed_since_check: "The provider changed since the check. Run it again.",
    fed_err_check_too_old: "The check is more than 10 minutes old. Run it again.",
    fed_err_step_up_lapsed: "Your identity confirmation expired while the check ran. Confirm it again, then run the check again.",

    fed_cause_invalid_issuer: "The provider's configured issuer or allowed_origins is not acceptable: the issuer must be an absolute https URL on a public host name, and each allowed origin a canonical https origin. Check the provider's configuration block.",
    fed_cause_not_absolute: |field| {
        format!(
            "The upstream's discovery document's {field} is not an absolute URL. This cannot be used."
        )
    },
    fed_cause_not_https: |field| {
        format!(
            "The upstream's discovery document's {field} is not https. This provider only uses https endpoints."
        )
    },
    fed_cause_has_userinfo: |field| {
        format!(
            "The upstream's discovery document's {field} carries a username or password. This is not allowed."
        )
    },
    fed_cause_has_query: |field| {
        format!(
            "The upstream's discovery document's {field} carries a query string. This is not allowed."
        )
    },
    fed_cause_has_fragment: |field| {
        format!(
            "The upstream's discovery document's {field} carries a fragment. This is not allowed."
        )
    },
    fed_cause_noncanonical: |field| {
        format!(
            "The upstream's discovery document's {field} is not a canonical URL. This cannot be used."
        )
    },
    fed_cause_origin_not_allowed: |field, origin| {
        format!(
            "The upstream's discovery document puts its {field} on {origin}, which this provider's \
         allowed_origins does not list. If you trust that origin, add it to allowed_origins and \
         restart sui-id."
        )
    },
    fed_cause_issuer_mismatch: "The upstream's discovery document's issuer does not match the configured issuer exactly. Check both against the upstream's own documentation.",
    fed_cause_response_types_missing_code: "The upstream's discovery document does not list \"code\" among its response types. sui-id only uses the authorization code flow.",
    fed_cause_grant_types_missing_auth_code: "The upstream's discovery document lists grant types but omits \"authorization_code\". sui-id only uses the authorization code flow.",
    fed_cause_code_challenge_missing_s256: "The upstream's discovery document does not list \"S256\" among its PKCE code challenge methods. sui-id requires S256.",
    fed_cause_auth_response_iss_not_true: "The upstream's discovery document does not confirm that it sends an \"iss\" parameter on the authorization response. sui-id requires it.",
    fed_cause_subject_types_missing_public: "The upstream's discovery document does not list \"public\" among its subject types. sui-id requires public subjects.",
    fed_cause_bounds_status: |code| {
        format!("The upstream answered with HTTP status {code}; sui-id accepts only 200 OK here.")
    },
    fed_cause_bounds_content_type: "The upstream's response does not declare itself as JSON (its Content-Type is missing or another type).",
    fed_cause_bounds_too_large: |limit| {
        format!("The upstream's response is larger than sui-id reads ({limit} bytes).")
    },
    fed_cause_bounds_structure: |limit| {
        format!(
            "The upstream's response has more members, items or characters in one place than sui-id allows ({limit})."
        )
    },
    fed_cause_too_deep: |limit| {
        format!(
            "The upstream's response nests deeper than sui-id allows ({limit} levels). This is not a usable document."
        )
    },
    fed_cause_duplicate_member: "The upstream's response repeats a field name. This is not a usable document.",
    fed_cause_not_json: "The upstream's response is not JSON.",
    fed_cause_jwks_not_object: "The upstream's signing-key document is JSON, but its root is not an object.",
    fed_cause_jwks_no_keys: "The upstream's signing-key document has no \"keys\" array.",
    fed_cause_jwks_too_many_keys: |limit| {
        format!("The upstream's signing-key document has more than {limit} keys.")
    },
    fed_cause_jwks_key_not_object: "One of the upstream's signing keys is not an object.",
    fed_cause_jwks_duplicate_kid: "Two of the upstream's signing keys share the same key ID.",
    fed_cause_no_ca_certificates: "This host has no CA certificates to verify TLS with. Install a CA certificate bundle and restart sui-id.",
    fed_cause_dns_over_cap: |host, count, cap| {
        format!("{host} answered with {count} addresses; sui-id accepts at most {cap}.")
    },
    fed_cause_dns_denied: |host, reason| {
        format!("{host} resolves to an address sui-id never connects to ({reason}).")
    },
    dns_class_private_network: "a private network address",
    dns_class_loopback: "a loopback address",
    dns_class_link_local: "a link-local address",
    dns_class_documentation: "a documentation/example address",
    dns_class_broadcast_or_unspecified: "a broadcast or unspecified address",
    dns_class_translation_or_tunnel: "an IPv4/IPv6 translation or tunnelling address",
    dns_class_reserved_for_special_use: "an address reserved for a specific protocol purpose",
    fed_cause_tls: "The TLS handshake with the upstream failed. Check this host's CA certificates and the upstream's certificate.",
    fed_cause_timeout: "The request to the upstream timed out. Check this host's network and the upstream's availability.",
    fed_cause_unreachable: "The upstream could not be reached. Check this host's network and DNS, and see the server log.",
    fed_cause_no_jwks_uri: "The upstream's discovery document names no jwks_uri. Its signing keys cannot be found.",
    fed_cause_no_usable_key: "None of the upstream's signing keys is usable for an algorithm both sides support. Check the server log for the reason.",

    federation_providers_delete_button: "Delete",
    confirm_delete_federation_provider_title: "Delete this federation provider",
    confirm_delete_federation_provider_impact_zero: "No user signs in through this provider.",
    confirm_delete_federation_provider_impact_suffix: |n| {
        if n == 1 {
            "user will no longer be able to sign in through this provider; their sui-id \
             account remains."
                .to_owned()
        } else {
            "users will no longer be able to sign in through this provider; their sui-id \
             accounts remain."
                .to_owned()
        }
    },
    confirm_delete_federation_provider_reversibility: "This cannot be undone.",
    federation_providers_deleted_flash: "The provider was deleted.",
    fed_err_still_configured: "A [[federation_providers]] block still names this provider. \
         Remove it and restart sui-id first, or it would be created again on restart.",

    user_detail_federation_section: "Federated sign-in",
    user_detail_federation_th_provider: "Provider",
    user_detail_federation_th_linked: "Linked",
    user_detail_federation_th_last_used: "Last used",
    user_detail_federation_unlink_button: "Unlink",
    user_detail_federation_empty: "No federated sign-in methods linked.",
    confirm_unlink_federation_title: "Unlink this federated sign-in",
    confirm_unlink_federation_impact: "{user} will no longer be able to sign in through \
         {provider}. The link cannot be made again from the admin UI: this release has no \
         way to re-link a user.",
    confirm_unlink_federation_sessions_note: "Sessions already open, and tokens already \
         issued to applications, stay valid — unlinking does not end them. To end them, \
         disable the account on this user's page.",
    confirm_unlink_federation_sessions_link: "Go to this user's page",
    fed_err_link_not_found: "This link no longer exists; reload the page.",
    sign_in_method_password: "their password",
    sign_in_method_passkey: "a passkey",
    sign_in_method_other_federation: "another federation provider",
    sign_in_method_directory: "their organization's directory",
    join_sign_in_methods: |methods| match methods {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
    },
    confirm_unlink_federation_methods_remain: |methods| {
        format!("Their account remains, and they can still sign in with {methods}.")
    },
    confirm_unlink_federation_no_methods_remain: "{user} has no other way to sign in. After \
         this, they cannot sign in at all. To give them access again, issue a recovery link \
         from their page.",
    confirm_unlink_federation_offer_heading: "What you can do next",
    confirm_unlink_federation_offer_disable_link: "Disable this user",
    confirm_unlink_federation_offer_disable_text: ": ends open sessions and the tokens \
         already issued to applications.",
    confirm_unlink_federation_offer_delete_link: "Delete this user",
    confirm_unlink_federation_offer_delete_text: ": removes the account and its personal data.",
    confirm_unlink_federation_offer_recovery_link_link: "Issue a recovery link",
    confirm_unlink_federation_offer_recovery_link_text: ": after unlinking, gives them access \
         again, with a password they choose. While the link exists, a recovery link is refused.",
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
mod tests;
