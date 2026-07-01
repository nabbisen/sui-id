//! Translatable string table.
//!
//! Adding a new translatable text means adding a field here. The
//! exhaustive struct literal in each `static STRINGS_*` constant
//! (see [`crate::ja`], [`crate::en`]) then fails to compile until
//! every locale supplies a value, which is what gives us
//! "translation completeness" as a compile-time guarantee.

/// All translatable UI strings.
///
/// Keep field names descriptive — the field name is what shows up
/// at every call site, and `t.login_button_submit` is much easier
/// to grep for than `t.s127`.
///
/// Group fields by area with section comments. When a string has
/// variable interpolation, expose it as a method below the struct
/// (`impl Strings { pub fn n_outstanding_tokens(...) }`) rather
/// than as a field.
pub struct Strings {
    // ---- Generic UI ----
    pub button_save: &'static str,
    pub button_cancel: &'static str,
    pub button_back: &'static str,
    pub button_edit: &'static str,
    pub button_view_detail: &'static str,
    pub button_continue: &'static str,
    pub button_delete: &'static str,
    pub danger_zone_title: &'static str,
    pub button_create: &'static str,
    pub button_confirm: &'static str,
    pub button_test: &'static str,
    pub badge_enabled: &'static str,
    pub badge_disabled: &'static str,
    pub badge_ok: &'static str,
    pub badge_warn: &'static str,
    pub badge_error: &'static str,
    pub label_optional: &'static str,
    pub label_required: &'static str,
    pub muted_none: &'static str,

    // ---- Language native names (RFC 051) ----
    // Each language's name in its own script. These values are
    // intentionally the SAME across all three locale files: a Japanese
    // user sees "日本語" exactly as a French user would, since this is
    // how the language refers to itself. Routing through `Strings`
    // makes the convention explicit and silences the CJK grep.
    pub locale_native_ja: &'static str,
    pub locale_native_en: &'static str,
    /// Name of the Simplified Chinese locale in the current locale's language.
    /// Used by the language picker.
    pub locale_native_zh_hans: &'static str,
    /// Name of the Traditional Chinese locale in the current locale's language.
    /// Used by the language picker once zh-Hant translations are complete.
    pub locale_native_zh_hant: &'static str,

    // ---- Lifetime formatting (RFC 051) ----
    // Used by fmt_lifetime() to render durations like "30 days (2592000s)".
    pub fmt_lifetime_days: fn(i64, i64) -> String,
    pub fmt_lifetime_hours: fn(i64, i64) -> String,
    pub fmt_lifetime_minutes: fn(i64, i64) -> String,

    // ---- Settings: Auth extensions (RFC 051) ----
    pub settings_auth_min_length_value: fn(usize) -> String,
    pub settings_auth_recovery_codes_label: &'static str,
    pub settings_auth_recovery_codes_value: fn(usize) -> String,
    pub settings_auth_mfa_note_prefix: &'static str,
    pub settings_auth_mfa_note_suffix: &'static str,

    // ---- Settings: Logs extensions (RFC 051) ----
    pub settings_logs_lede: &'static str,
    pub settings_logs_kv_format: &'static str,
    pub settings_logs_kv_filter: &'static str,
    pub settings_logs_audit_link_prefix: &'static str,
    pub settings_logs_audit_link_suffix: &'static str,

    // ---- Settings: Advanced/Other extensions (RFC 051) ----
    pub settings_advanced_lede: &'static str,
    pub settings_advanced_storage_note_prefix: &'static str,
    pub settings_advanced_storage_note_suffix: &'static str,
    pub settings_advanced_users_count: fn(usize) -> String,
    pub settings_advanced_clients_count: fn(usize) -> String,

    // ---- Settings: Email extensions (RFC 051) ----
    pub settings_email_enable_checkbox: &'static str,
    pub settings_email_enable_hint: &'static str,
    pub settings_email_password_placeholder_change: &'static str,
    pub settings_email_password_placeholder_none: &'static str,
    pub settings_email_password_hint: &'static str,
    pub settings_email_base_url_hint: &'static str,
    pub settings_email_save_button: &'static str,
    pub settings_email_test_section: &'static str,
    pub settings_email_test_lede: &'static str,

    // ---- Navigation ----
    pub nav_dashboard: &'static str,
    pub nav_users: &'static str,
    pub nav_clients: &'static str,
    pub nav_signing_keys: &'static str,
    pub nav_audit: &'static str,
    pub nav_settings: &'static str,
    pub nav_profile: &'static str,
    /// Nav label for the consolidated self-service security surface
    /// (RFC 055, v0.44.0). Replaces the use of `nav_profile` which
    /// pointed to the legacy `/admin/profile` single page.
    pub nav_apps: &'static str,
    pub nav_my_account: &'static str,
    pub settings_tab_general: &'static str,
    pub settings_tab_advanced: &'static str,
    pub me_overview_last_login: &'static str,
    pub me_overview_first_login: &'static str,
    pub nav_security: &'static str,
    pub nav_logout: &'static str,
    pub a11y_skip_to_main: &'static str,
    pub nav_aria_main: &'static str,
    pub nav_aria_signout: &'static str,

    // ---- Footer (RFC 050) ----
    pub footer_tagline: &'static str,
    pub footer_a11y_group_label: &'static str,
    pub a11y_keyboard: &'static str,
    pub a11y_screen_reader: &'static str,
    pub a11y_contrast: &'static str,

    // ---- Theme toggle (RFC 050) ----
    pub theme_toggle_group: &'static str,
    pub theme_toggle_light: &'static str,
    pub theme_toggle_auto: &'static str,
    pub theme_toggle_dark: &'static str,
    pub theme_toggle_light_title: &'static str,
    pub theme_toggle_auto_title: &'static str,
    pub theme_toggle_dark_title: &'static str,

    // ---- Status words (RFC 052) ----
    // Single source of truth for the status badges that previously
    // duplicated their text+class assignment across 24+ call sites.
    // Used via `components::status_badge(t, kind)`.
    pub status_active: &'static str,
    pub status_disabled: &'static str,
    pub status_deleted: &'static str,
    pub status_admin: &'static str,
    pub status_on: &'static str,
    pub status_off: &'static str,
    pub status_in_use: &'static str,
    pub status_retired: &'static str,
    pub status_published: &'static str,
    pub status_pending: &'static str,
    pub status_healthy: &'static str,
    pub status_unhealthy: &'static str,

    // ---- Empty placeholders (RFC 052) ----
    // For rendering "no value" / "any" / "fallback" cells consistently.
    // The em-dash `empty_dash` replaces the ASCII `-` previously used,
    // because U+2014 renders consistently across CJK and Latin fonts
    // and is unambiguous (the ASCII hyphen could be confused with a
    // real value).
    pub empty_dash: &'static str,
    pub empty_any: &'static str,
    pub empty_none: &'static str,
    pub empty_falls_back_redirect_uris: &'static str,
    pub empty_no_email: &'static str,
    pub empty_not_set: &'static str,

    // ---- Copy-to-clipboard button (RFC 053) ----
    // The clipboard button reads `copy_button_label` for its idle text
    // and `copy_button_label_done` for its post-click confirmation. The
    // aria-label / title attributes are built from
    // `copy_button_aria_template`, where `{noun}` is replaced with one
    // of the `copy_noun_*` strings at the call site.
    pub copy_button_label: &'static str,
    pub copy_button_label_done: &'static str,
    pub copy_button_aria_template: &'static str,
    pub copy_noun_client_id: &'static str,
    pub copy_noun_client_secret: &'static str,
    pub copy_noun_jwks_uri: &'static str,
    pub copy_noun_redirect_uri: &'static str,
    pub copy_noun_audit_row_id: &'static str,
    pub copy_noun_setup_token: &'static str,
    pub copy_noun_recovery_code: &'static str,
    pub copy_noun_passkey_id: &'static str,

    // ---- Login ----
    pub signed_out_flash: &'static str,
    pub login_title: &'static str,
    pub login_username_label: &'static str,
    pub login_password_label: &'static str,
    pub login_submit: &'static str,
    pub login_passkey_button: &'static str,
    /// Shown when a non-admin user attempts to log in directly to the
    /// admin panel (not via the OIDC flow). The session is not established.
    pub login_no_admin_access: &'static str,
    pub login_passkey_primary: &'static str,
    pub login_forgot_password_link: &'static str,
    pub login_invalid_credentials: &'static str,
    pub login_account_locked: &'static str,
    pub login_reset_ok_banner: &'static str,

    // ---- Setup wizard ----
    pub setup_step_welcome: &'static str,
    pub setup_step_admin: &'static str,
    pub setup_step_done: &'static str,
    pub setup_welcome_title: &'static str,
    pub setup_welcome_lede: &'static str,
    pub setup_welcome_lede2: &'static str,
    pub setup_welcome_begin: &'static str,
    pub setup_admin_title: &'static str,
    pub setup_admin_lede: &'static str,
    pub setup_admin_token_label: &'static str,
    pub setup_admin_token_hint: &'static str,
    pub setup_admin_username_label: &'static str,
    pub setup_admin_email_label: &'static str,
    pub setup_admin_email_hint: &'static str,
    pub setup_admin_display_label: &'static str,
    pub setup_admin_password_label: &'static str,
    pub setup_admin_password_hint: &'static str,
    pub setup_admin_confirm_label: &'static str,
    pub setup_admin_submit: &'static str,
    pub setup_done_title: &'static str,
    pub setup_done_lede: &'static str,
    pub setup_done_next_steps_title: &'static str,
    pub setup_done_next_step_register_clients: &'static str,
    pub setup_done_next_step_enable_mfa: &'static str,
    pub setup_done_next_step_review_settings: &'static str,
    pub setup_done_enter_admin: &'static str,
    pub setup_not_complete_title: &'static str,
    pub setup_not_complete_lede: &'static str,
    pub setup_password_mismatch: &'static str,
    pub setup_already_initialized: &'static str,
    pub setup_invalid_token: &'static str,
    /// Flash text shown by the setup wizard when HIBP is in
    /// `Block` mode and the supplied password is found in the
    /// breach corpus.
    pub setup_hibp_blocked: &'static str,
    /// Generic fallback flash text for an otherwise-unmapped
    /// setup-wizard error (e.g. an internal storage failure).
    pub setup_generic_failure: &'static str,

    // ---- RFC 012: Setup wizard — Language step ----
    pub setup_step_lang: &'static str,
    pub setup_lang_title: &'static str,
    pub setup_lang_lede: &'static str,
    pub setup_lang_field_label: &'static str,
    pub setup_lang_default_note: &'static str,
    pub setup_lang_submit: &'static str,

    // ---- RFC 012: Setup wizard — HIBP step ----
    pub setup_step_hibp: &'static str,
    pub setup_hibp_step_title: &'static str,
    pub setup_hibp_step_lede: &'static str,
    pub setup_hibp_option_off: &'static str,
    pub setup_hibp_option_off_desc: &'static str,
    pub setup_hibp_option_warn: &'static str,
    pub setup_hibp_option_warn_desc: &'static str,
    pub setup_hibp_option_block: &'static str,
    pub setup_hibp_option_block_desc: &'static str,
    pub setup_hibp_step_default_note: &'static str,
    pub setup_hibp_step_submit: &'static str,

    // ---- Step-up auth ----
    pub step_up_title: &'static str,
    pub step_up_lede: &'static str,
    pub step_up_code_label: &'static str,
    pub step_up_code_hint: &'static str,
    pub step_up_code_invalid: &'static str,
    pub step_up_passkey_alt: &'static str,
    pub step_up_passkey_button: &'static str,

    // ---- MFA challenge (login flow) ----
    pub mfa_challenge_title: &'static str,
    pub mfa_challenge_lede: &'static str,
    pub mfa_challenge_code_label: &'static str,
    pub mfa_challenge_submit: &'static str,
    pub mfa_challenge_passkey_alt: &'static str,
    pub mfa_challenge_passkey_button: &'static str,
    pub mfa_challenge_shell_title: &'static str,
    /// Flash shown when the submitted TOTP / recovery code does
    /// not verify. Same text covers both factors so the UI
    /// doesn't leak which one the user got wrong.
    pub mfa_challenge_failed_flash: &'static str,

    // ---- Profile / self-service security (v0.29.1) ----
    pub profile_subtitle_template: &'static str,
    pub profile_recovery_save_now: &'static str,
    pub profile_recovery_save_lede: &'static str,
    pub profile_mfa_section: &'static str,
    pub profile_mfa_totp_card_title: &'static str,
    pub profile_mfa_status_label: &'static str,
    pub profile_mfa_status_enabled: &'static str,
    pub profile_mfa_status_not_configured: &'static str,
    pub profile_mfa_regenerate_codes: &'static str,
    pub profile_mfa_disable_confirm: &'static str,
    pub profile_mfa_disable_button: &'static str,
    pub profile_mfa_enroll_lede: &'static str,
    pub profile_mfa_enroll_apps_note: &'static str,
    pub profile_mfa_enroll_button: &'static str,
    /// Flash shown after MFA enrolment completes successfully.
    /// Pinned in audit-friendly wording: "now enabled" reads as a
    /// state assertion that doubles as confirmation the action
    /// landed.
    pub profile_passkeys_section: &'static str,
    pub profile_passkeys_lede: &'static str,
    pub profile_passkeys_th_name: &'static str,
    pub profile_passkeys_th_registered: &'static str,
    pub profile_passkeys_th_last_used: &'static str,
    pub profile_passkeys_last_used_never: &'static str,
    pub profile_passkeys_delete_confirm: &'static str,
    pub profile_passkeys_delete_button: &'static str,
    pub profile_passkeys_empty: &'static str,
    pub profile_passkeys_register_section: &'static str,
    pub profile_passkeys_nickname_label: &'static str,
    pub profile_passkeys_nickname_hint: &'static str,
    pub profile_passkeys_register_button: &'static str,
    pub profile_lang_section: &'static str,
    pub profile_lang_lede: &'static str,
    pub profile_lang_field_label: &'static str,
    /// Flash shown after a successful TOTP enrolment confirmation.
    pub profile_mfa_enrolled_flash: &'static str,
    /// Flash shown after recovery codes are regenerated.
    pub profile_recovery_regenerated_flash: &'static str,

    // ---- MFA setup (TOTP enrollment) ----
    pub mfa_setup_shell_title: &'static str,
    pub mfa_setup_title: &'static str,
    pub mfa_setup_lede: &'static str,
    pub mfa_setup_steps_title: &'static str,
    pub mfa_setup_step1: &'static str,
    pub mfa_setup_step2: &'static str,
    pub mfa_setup_step3: &'static str,
    pub mfa_setup_qr_card_title: &'static str,
    pub mfa_setup_secret_label: &'static str,
    pub mfa_setup_otpauth_summary: &'static str,
    pub mfa_setup_verify_card_title: &'static str,
    pub mfa_setup_code_label: &'static str,
    pub mfa_setup_code_hint: &'static str,
    pub mfa_setup_confirm_button: &'static str,

    // ---- Forgot password ----
    pub forgot_password_title: &'static str,
    pub forgot_password_lede: &'static str,
    pub forgot_password_email_label: &'static str,
    pub forgot_password_submit: &'static str,
    pub forgot_password_sent_title: &'static str,
    pub forgot_password_sent_lede: &'static str,
    pub forgot_password_sent_lede2: &'static str,
    pub reset_password_title: &'static str,
    pub reset_password_lede: &'static str,
    pub reset_password_new_label: &'static str,
    pub reset_password_new_hint: &'static str,
    pub reset_password_confirm_label: &'static str,
    pub reset_password_submit: &'static str,
    pub reset_password_invalid_title: &'static str,
    pub reset_password_invalid_lede: &'static str,
    pub reset_password_invalid_request_again: &'static str,
    pub password_mismatch_flash: &'static str,
    pub reset_password_failed_flash: &'static str,
    /// Common navigational link from forgot-password / reset-password
    /// flows back to the login screen. Reused across multiple
    /// auth-flow views.
    pub back_to_login: &'static str,

    // ---- Settings hub ----
    pub settings_title: &'static str,
    pub settings_lede: &'static str,
    pub settings_tab_basic: &'static str,
    pub settings_tab_security: &'static str,
    pub settings_tab_authentication: &'static str,
    pub settings_tab_logs: &'static str,
    pub settings_tab_email: &'static str,
    pub settings_tab_other: &'static str,
    pub settings_basic_section: &'static str,
    pub settings_basic_oidc_section: &'static str,
    pub settings_basic_issuer: &'static str,
    pub settings_basic_listen_addr: &'static str,
    pub settings_basic_cookie_secure: &'static str,
    pub settings_basic_trusted_proxies: &'static str,
    pub settings_basic_trusted_proxies_none: &'static str,
    pub settings_basic_default_lang: &'static str,
    pub settings_basic_default_lang_hint: &'static str,
    pub settings_basic_save: &'static str,
    pub settings_basic_saved: &'static str,

    // ---- Profile ----
    pub profile_title: &'static str,
    pub profile_username_label: &'static str,
    pub profile_email_label: &'static str,
    pub profile_display_name_label: &'static str,
    pub profile_lang_label: &'static str,
    pub profile_lang_hint: &'static str,
    pub profile_lang_browser_default: &'static str,
    pub profile_save: &'static str,
    pub profile_saved: &'static str,

    // ---- Password change ----
    pub password_change_title: &'static str,
    pub password_change_lede: &'static str,
    pub password_change_current_label: &'static str,
    pub password_change_new_label: &'static str,
    /// Hint shown under the new-password input. Reminds the user
    /// that a long passphrase beats a short complicated string.
    pub password_change_new_hint: &'static str,
    pub password_change_confirm_label: &'static str,
    pub password_change_revoke_others_label: &'static str,
    /// Hint shown under the "sign out other sessions" checkbox.
    pub password_change_revoke_others_hint: &'static str,
    pub password_change_submit: &'static str,
    pub password_change_wrong_current: &'static str,
    pub password_change_done_flash: &'static str,

    // ---- /me/security (self-service security page) ----
    pub me_security_title: &'static str,
    /// Lede line; rendered as `<strong>{username}</strong> {me_security_signed_in_as_suffix}`.
    pub me_security_signed_in_as_suffix: &'static str,
    pub me_security_admin_link: &'static str,
    pub me_security_mfa_section: &'static str,
    pub me_security_mfa_status_label: &'static str,
    pub me_security_mfa_status_enabled: &'static str,
    pub me_security_mfa_factor_totp: &'static str,
    /// Format string fragment; the call site interpolates the
    /// passkey count via `format!("{n} {label}")`.
    pub me_security_mfa_factor_passkey_n: &'static str,
    pub me_security_mfa_disabled_title: &'static str,
    pub me_security_mfa_disabled_lede: &'static str,
    pub me_security_mfa_manage: &'static str,
    // ---- Self-service MFA tab extensions (RFC 055, 056) ----
    pub me_security_mfa_recovery_section_label: &'static str,
    pub me_security_mfa_recovery_codes_remaining: fn(usize) -> String,
    /// Success banner shown after `POST /me/security/language` (RFC 057).
    pub me_security_language_saved_banner: &'static str,

    // ---- Aria-label nav landmarks (RFC 054, v0.44.0) ----
    // Short navigation cues announced by screen readers. Distinct
    // from the visible labels so the announcement names the
    // landmark's purpose without being redundant with on-page text.
    pub setup_steps_aria: &'static str,
    pub me_security_tabs_aria: &'static str,
    pub settings_tabs_aria: &'static str,
    pub me_security_password_change_link: &'static str,
    pub me_security_sessions_section: &'static str,
    pub me_security_sessions_lede: &'static str,
    pub me_security_sessions_th_started: &'static str,
    pub me_security_sessions_th_expires: &'static str,
    pub me_security_sessions_th_factors: &'static str,
    pub me_security_sessions_current_badge: &'static str,
    pub me_security_sessions_revoke: &'static str,
    pub me_security_sessions_revoke_confirm: &'static str,
    pub me_security_sessions_revoke_all_others: &'static str,
    pub me_security_sessions_revoke_all_others_confirm: &'static str,
    pub me_security_activity_section: &'static str,
    pub me_security_activity_lede: &'static str,
    pub me_security_activity_th_when: &'static str,
    pub me_security_activity_th_event: &'static str,
    pub me_security_activity_th_result: &'static str,
    pub me_security_activity_th_note: &'static str,

    // ---- Email subjects (notifications) ----
    pub email_subject_password_reset: &'static str,
    pub email_subject_password_changed: &'static str,
    pub email_greeting_suffix: &'static str,
    pub email_password_reset_intro: &'static str,
    pub email_password_reset_link_label: &'static str,
    pub email_password_reset_disregard: &'static str,
    pub email_password_changed_intro: &'static str,
    pub email_password_changed_security_warning: &'static str,
    pub email_password_changed_link_security: &'static str,

    // ---- Errors / generic (RFC 042 extended) ----
    pub error_generic_title: &'static str,
    pub error_generic_lede: &'static str,
    pub error_not_found_title: &'static str,
    pub error_not_found_lede: &'static str,
    pub error_internal: &'static str,
    pub error_internal_lede: &'static str,
    pub error_too_many_requests_label: &'static str,
    pub error_too_many_requests_lede: &'static str,
    pub error_request_id_label: &'static str,
    pub error_back_home: &'static str,
    // Aliases used in locale files from prior sessions — kept for compatibility.
    pub error_404_title: &'static str,
    pub error_404_lede: &'static str,
    pub error_429_title: &'static str,
    pub error_429_lede: &'static str,
    pub error_500_title: &'static str,
    pub error_500_lede: &'static str,

    // ---- Audit log labels ----
    pub audit_title: &'static str,
    pub audit_col_when: &'static str,
    pub audit_col_actor: &'static str,
    pub audit_col_action: &'static str,
    pub audit_col_target: &'static str,
    pub audit_col_outcome: &'static str,
    pub audit_col_note: &'static str,
    // ---- Settings tab (RFC 023 renames "Other" → "Advanced") ----

    // ---- Audit event labels (RFC 002 § D) ----
    pub audit_event_auth_login_success: &'static str,
    pub audit_event_auth_login_failure: &'static str,
    pub audit_event_auth_login_locked: &'static str,
    pub audit_event_auth_login_mfa_required: &'static str,
    pub audit_event_auth_logout: &'static str,
    pub audit_event_auth_mfa_success: &'static str,
    pub audit_event_auth_mfa_failure: &'static str,
    pub audit_event_auth_password_changed_self: &'static str,
    pub audit_event_auth_password_reset_requested: &'static str,
    pub audit_event_auth_password_reset_email_sent: &'static str,
    pub audit_event_auth_password_reset_email_failed: &'static str,
    pub audit_event_auth_password_reset_throttled: &'static str,
    pub audit_event_auth_password_reset_completed: &'static str,
    pub audit_event_auth_refresh_theft_detected: &'static str,
    pub audit_event_auth_session_revoked: &'static str,
    pub audit_event_auth_sessions_bulk_revoke_self: &'static str,
    pub audit_event_auth_smtp_config_changed: &'static str,
    pub audit_event_user_create: &'static str,
    pub audit_event_user_delete: &'static str,
    pub audit_event_user_reset_password: &'static str,
    pub audit_event_admin_user_unlock: &'static str,
    pub audit_event_client_create: &'static str,
    pub audit_event_client_update: &'static str,
    pub audit_event_client_delete: &'static str,
    pub audit_event_client_set_allowed_scopes: &'static str,
    pub audit_event_signing_key_rotate: &'static str,
    pub audit_event_signing_key_delete: &'static str,
    pub audit_event_admin_master_key_rotated: &'static str,
    pub audit_event_setup_create_initial_admin: &'static str,
    // ---- Admin: Dashboard (RFC 029) ----
    pub dashboard_title: &'static str,
    pub dashboard_lede: &'static str,
    pub dashboard_stat_users: &'static str,
    pub dashboard_stat_clients: &'static str,
    pub dashboard_stat_sessions: &'static str,
    pub dashboard_stat_service_status: &'static str,
    pub dashboard_stat_service_ok: &'static str,
    pub dashboard_activity_title: &'static str,
    pub dashboard_activity_period: &'static str,
    // ---- Admin: Dashboard extensions (RFC 051) ----
    pub dashboard_greeting: fn(&str) -> String,
    pub dashboard_aria_stats: &'static str,
    pub dashboard_aria_action_required: &'static str,
    pub dashboard_action_required_title: &'static str,
    pub dashboard_activity_success: &'static str,
    pub dashboard_activity_failure: &'static str,
    pub dashboard_activity_hover_hint: &'static str,
    pub dashboard_oidc_endpoint_discovery: &'static str,
    pub dashboard_oidc_endpoint_jwks: &'static str,
    pub dashboard_sparkline_aria: &'static str,
    pub dashboard_sparkline_tooltip: fn(&str, i64, i64) -> String,
    // ---- Admin: Dashboard operator prompts (RFC 031) ----
    pub dashboard_warn_smtp: &'static str,
    pub dashboard_warn_hibp: &'static str,
    pub dashboard_warn_cookie_insecure: &'static str,
    pub dashboard_warn_admins_no_mfa: fn(usize) -> String,
    pub dashboard_warn_old_signing_key: fn(i64) -> String,
    pub dashboard_warn_outbox_stuck: fn(usize) -> String,
    pub dashboard_warn_pending_resets: fn(usize) -> String,
    pub dashboard_getting_started_title: &'static str,
    pub dashboard_getting_started_smtp: &'static str,
    pub dashboard_getting_started_first_app: &'static str,
    pub dashboard_getting_started_admin_mfa: &'static str,
    // ---- Admin: User detail page (RFC 035) ----
    pub user_detail_back: &'static str,
    pub user_detail_auth_section: &'static str,
    pub user_detail_totp_label: &'static str,
    pub user_detail_passkeys_label: &'static str,
    pub user_detail_sessions_section: &'static str,
    pub user_detail_sessions_th_started: &'static str,
    pub user_detail_sessions_th_expires: &'static str,
    pub user_detail_sessions_th_factors: &'static str,
    pub user_detail_activity_section: &'static str,
    /// Description paragraph inside the user-detail danger zone.
    pub user_detail_danger_zone_body: &'static str,
    pub role_admin: &'static str,
    pub role_auditor: &'static str,
    pub role_user: &'static str,
    pub user_detail_role_section: &'static str,
    pub user_detail_role_change: &'static str,
    pub user_detail_role_saved: &'static str,
    pub user_detail_role_last_admin: &'static str,
    // ---- Settings: common sections (RFC 029 second pass) ----
    pub settings_page_title_template: &'static str, // "{tab} — Settings"
    // Basic tab sections
    pub settings_basic_description: &'static str,
    // Settings: Basic extensions (RFC 051)
    pub settings_basic_kv_issuer: &'static str,
    pub settings_basic_kv_listen: &'static str,
    pub settings_basic_kv_cookie_secure: &'static str,
    pub settings_basic_kv_trusted_proxies: &'static str,
    // Security tab sections
    pub settings_security_session_section: &'static str,
    pub settings_security_session_lede: &'static str,
    pub settings_security_idle_timeout_label: &'static str,
    pub settings_security_max_sessions_label: &'static str,
    pub settings_security_lockout_section: &'static str,
    pub settings_security_headers_section: &'static str,
    // ---- Settings: Security extensions (RFC 051) ----
    pub settings_security_idle_timeout_hint: &'static str,
    pub settings_security_max_sessions_hint: &'static str,
    pub settings_security_lockout_hint_1: &'static str,
    pub settings_security_lockout_hint_2_pre: &'static str,
    pub settings_security_lockout_hint_2_post: &'static str,
    pub settings_security_headers_perm_policy_label: &'static str,
    pub settings_security_headers_hint: &'static str,
    pub settings_security_cors_token_label: &'static str,
    pub settings_security_cors_public_label: &'static str,
    // Authentication tab sections
    pub settings_auth_password_section: &'static str,
    pub settings_auth_mfa_section: &'static str,
    pub settings_auth_oidc_section: &'static str,
    // Logs tab sections
    pub settings_logs_output_section: &'static str,
    pub settings_logs_audit_section: &'static str,
    // Other/Advanced tab sections
    pub settings_advanced_build_section: &'static str,
    pub settings_advanced_storage_section: &'static str,
    pub settings_advanced_record_counts: &'static str,
    // ---- User disable reason (RFC 045) ----
    pub disable_reason_label: &'static str,
    pub disable_reason_placeholder: &'static str,
    pub disable_reason_hint: &'static str,

    // ---- /me/security tabs (RFC 040) ----
    pub me_tab_overview: &'static str,
    pub me_tab_password: &'static str,
    pub me_tab_apps: &'static str,
    pub me_apps_title: &'static str,
    pub me_apps_intro: &'static str,
    pub me_apps_granted_on: &'static str,
    pub me_apps_last_used: &'static str,
    pub me_apps_never_used: &'static str,
    pub me_apps_revoke_button: &'static str,
    pub me_apps_revoked: &'static str,
    pub me_apps_empty: &'static str,
    pub me_tab_mfa: &'static str,
    pub me_tab_passkey: &'static str,
    pub me_tab_sessions: &'static str,
    pub me_tab_language: &'static str,
    // Overview tab
    pub me_overview_section_status: &'static str,
    pub me_overview_section_activity: &'static str,
    /// v0.48.2: status row label for "MFA (TOTP)" — was a hardcoded
    /// English literal in pages/me_security/overview.rs.
    pub me_overview_label_mfa_totp: &'static str,
    /// v0.48.2: status row label for "Passkeys" — was a hardcoded
    /// English literal in the same place.
    pub me_overview_label_passkeys: &'static str,
    /// v0.48.2: empty-state copy for the "Recent activity" panel.
    /// Pre-v0.48.2 this slot mistakenly used
    /// `me_security_sessions_lede` (which describes "other active
    /// sessions"), reading as nonsense in context.
    pub me_overview_no_recent_events: &'static str,
    /// v0.48.2: aria-label for the setup-wizard welcome screen's
    /// language picker (`<nav>` containing 3 language links).
    pub setup_welcome_lang_picker_label: &'static str,
    // Passkey tab
    pub me_passkey_origin_warning: &'static str,
    pub me_passkey_section_title: &'static str,
    pub me_passkey_button_rename: &'static str,
    pub me_passkey_nickname_label: &'static str,
    pub me_passkey_nickname_placeholder: &'static str,
    // Language tab
    pub me_language_title: &'static str,
    pub me_language_lede: &'static str,
    pub me_language_use_default: &'static str,
    pub me_language_saved_flash: &'static str,

    // ---- Dashboard recent events (RFC 043) ----
    pub dashboard_recent_events_title: &'static str,
    pub dashboard_recent_events_empty: &'static str,
    pub dashboard_recent_events_view_all: &'static str,

    // ---- Settings: page titles (RFC 039) ----
    pub settings_title_basic: &'static str,
    pub settings_title_security: &'static str,
    pub settings_title_authentication: &'static str,
    pub settings_title_logs: &'static str,
    pub settings_title_email: &'static str,
    pub settings_title_advanced: &'static str,

    // ---- Settings: auth tab body (RFC 039) ----
    pub settings_auth_min_length_label: &'static str,
    pub settings_auth_hash_algorithm_label: &'static str,
    pub settings_auth_mfa_totp: &'static str,
    pub settings_auth_mfa_passkey: &'static str,
    pub settings_auth_mfa_recovery_label: &'static str,
    pub settings_auth_access_token_ttl: &'static str,
    pub settings_auth_id_token_ttl: &'static str,
    pub settings_auth_refresh_token_ttl: &'static str,
    pub settings_auth_refresh_rotate: &'static str,
    pub settings_auth_refresh_theft: &'static str,
    pub settings_auth_pkce_required: &'static str,

    // ---- Settings: logs tab body (RFC 039) ----
    pub settings_logs_recent_24h: &'static str,
    pub settings_logs_chain_broken_note: &'static str,
    pub settings_logs_chain_ok_note: &'static str,

    // ---- Settings: advanced tab body (RFC 039) ----
    pub settings_advanced_version_label: &'static str,
    pub settings_advanced_schema_label: &'static str,
    pub settings_advanced_server_time_label: &'static str,
    pub settings_advanced_db_file_label: &'static str,
    pub settings_advanced_key_file_label: &'static str,
    pub settings_advanced_manage_link: &'static str,

    // ---- Settings: email tab body (RFC 039) ----
    pub settings_email_page_title: &'static str,
    pub settings_email_lede: &'static str,
    pub settings_email_smtp_section: &'static str,
    pub settings_email_enable_label: &'static str,
    pub settings_email_host_label: &'static str,
    pub settings_email_port_label: &'static str,
    pub settings_email_port_hint: &'static str,
    pub settings_email_tls_label: &'static str,
    pub settings_email_tls_implicit: &'static str,
    pub settings_email_username_label: &'static str,
    pub settings_email_from_addr_label: &'static str,
    pub settings_email_from_name_label: &'static str,
    pub settings_email_base_url_label: &'static str,
    pub settings_email_test_button: &'static str,
    pub settings_email_test_hint: &'static str,

    // ---- OIDC consent screen (RFC 038) ----
    pub consent_title: &'static str,
    pub consent_app_wants_access: &'static str,
    pub consent_scope_openid: &'static str,
    pub consent_scope_profile: &'static str,
    pub consent_scope_email: &'static str,
    pub consent_scope_offline_access: &'static str,
    pub consent_scope_openid_desc: &'static str,
    pub consent_scope_profile_desc: &'static str,
    pub consent_scope_email_desc: &'static str,
    pub consent_scope_offline_access_desc: &'static str,
    pub consent_approve: &'static str,
    pub consent_deny: &'static str,
    pub consent_policy_label: &'static str,
    pub consent_policy_none: &'static str,
    pub consent_policy_first_time: &'static str,
    pub consent_policy_always: &'static str,





    // ---- Admin: Users (RFC 029) ----
    pub users_title: &'static str,
    pub users_lede: &'static str,
    pub users_create_section: &'static str,
    pub users_create_button: &'static str,
    pub users_table_section: &'static str,
    pub users_table_th_status: &'static str,
    pub users_table_th_mfa: &'static str,
    pub users_is_admin_label: &'static str,
    pub users_empty: &'static str,
    // ---- Admin: Users extensions (RFC 051) ----
    pub users_count_caption: fn(usize) -> String,
    pub users_label_username: &'static str,
    pub users_label_display_name: &'static str,
    pub users_label_email: &'static str,
    pub users_label_password: &'static str,

    // ---- Admin: Client edit (RFC 051) ----
    pub client_edit_title: &'static str,
    pub client_edit_basic_section: &'static str,
    pub client_edit_basic_note: &'static str,
    pub client_edit_new_secret_label: &'static str,
    pub client_edit_label_client_id: &'static str,
    pub client_edit_label_kind: &'static str,
    pub client_edit_label_status: &'static str,
    pub client_edit_post_logout_hint: &'static str,

    // ---- Admin: Clients (RFC 029) ----
    pub clients_title: &'static str,
    pub clients_lede: &'static str,
    pub clients_create_section: &'static str,
    pub clients_table_section: &'static str,
    pub clients_secret_once_banner: &'static str,
    pub clients_table_th_name: &'static str,
    pub clients_table_th_kind: &'static str,
    pub clients_table_th_status: &'static str,
    pub clients_empty: &'static str,
    pub clients_single_realm_note: &'static str,
    // ---- Clients page extensions (RFC 051) ----
    pub clients_table_th_client_id: &'static str,
    pub clients_count_caption: fn(usize) -> String,
    pub clients_label_app_name: &'static str,
    pub clients_label_redirect_uris: &'static str,
    pub clients_hint_redirect_uris: &'static str,
    pub clients_label_allowed_scopes: &'static str,
    pub clients_hint_scopes_intro: &'static str,
    pub clients_hint_scopes_openid_note: &'static str,
    pub clients_hint_scopes_profile_note: &'static str,
    pub clients_hint_scopes_email_note: &'static str,
    pub clients_hint_scopes_offline_note: &'static str,
    pub clients_hint_scopes_default: &'static str,
    pub clients_label_post_logout_uris: &'static str,
    pub clients_hint_one_per_line: &'static str,
    pub clients_label_confidential_checkbox: &'static str,
    pub clients_button_register: &'static str,

    // ---- Admin: Audit log (RFC 029) ----
    pub audit_lede: &'static str,
    // ---- Audit log enhancements (RFC 033) ----
    pub audit_chain_ok: &'static str,
    pub audit_chain_broken: &'static str,
    pub audit_filter_label: &'static str,
    pub audit_filter_placeholder: &'static str,
    pub audit_export_csv: &'static str,
    // ---- Audit log extensions (RFC 051) ----
    pub audit_entry_count_caption: fn(usize) -> String,
    pub audit_filter_button: &'static str,
    pub audit_chain_broken_note: fn(i64) -> String,
    pub audit_chain_ok_note: fn(usize, usize) -> String,


    // ---- Admin: Signing keys (RFC 029) ----
    pub signing_keys_title: &'static str,
    pub signing_keys_lede: &'static str,
    pub signing_keys_rotate_section: &'static str,
    pub signing_keys_rotate_button: &'static str,
    pub signing_keys_rotate_warning: &'static str,
    pub signing_keys_table_section: &'static str,
    pub signing_keys_th_algorithm: &'static str,
    pub signing_keys_th_status: &'static str,
    pub signing_keys_th_created: &'static str,
    pub signing_keys_th_retired: &'static str,
    pub signing_keys_empty: &'static str,
    pub signing_keys_in_use_badge: &'static str,
    // ---- Signing keys extensions (RFC 051) ----
    pub signing_keys_count_caption: fn(usize) -> String,
    pub signing_keys_th_key_id: &'static str,
    pub signing_keys_rotate_explanation_1: &'static str,
    pub signing_keys_rotate_explanation_2: &'static str,
    pub signing_keys_rotate_explanation_3: &'static str,
    // ---- Dangerous operation confirmation screens (RFC 030) ----
    pub confirm_cancel: &'static str,
    pub badge_recoverable: &'static str,
    pub badge_not_recoverable: &'static str,
    // disable user
    pub confirm_disable_title: &'static str,
    pub confirm_disable_impact: &'static str,
    pub confirm_disable_reversibility: &'static str,
    pub confirm_disable_button: &'static str,
    // enable user (undo disable)
    pub confirm_enable_title: &'static str,
    pub confirm_enable_button: &'static str,
    // delete user
    pub confirm_delete_user_title: &'static str,
    pub confirm_delete_user_impact: &'static str,
    pub confirm_delete_user_reversibility: &'static str,
    pub confirm_delete_user_button: &'static str,
    // reset MFA
    pub confirm_reset_mfa_title: &'static str,
    pub confirm_reset_mfa_impact: &'static str,
    pub confirm_reset_mfa_reversibility: &'static str,
    pub confirm_reset_mfa_button: &'static str,
    // delete client
    pub confirm_delete_client_title: &'static str,
    pub confirm_delete_client_impact: &'static str,
    pub confirm_delete_client_reversibility: &'static str,
    pub confirm_delete_client_button: &'static str,
    // delete signing key
    pub confirm_delete_signing_key_title: &'static str,
    pub confirm_delete_signing_key_impact: &'static str,
    pub confirm_delete_signing_key_reversibility: &'static str,
    pub confirm_delete_signing_key_button: &'static str,
}
