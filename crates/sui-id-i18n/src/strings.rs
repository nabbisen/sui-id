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
    pub button_continue: &'static str,
    pub button_delete: &'static str,
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

    // ---- Navigation ----
    pub nav_dashboard: &'static str,
    pub nav_users: &'static str,
    pub nav_clients: &'static str,
    pub nav_signing_keys: &'static str,
    pub nav_audit: &'static str,
    pub nav_settings: &'static str,
    pub nav_profile: &'static str,
    pub nav_logout: &'static str,

    // ---- Login ----
    pub signed_out_flash: &'static str,
    pub login_title: &'static str,
    pub login_username_label: &'static str,
    pub login_password_label: &'static str,
    pub login_submit: &'static str,
    pub login_passkey_button: &'static str,
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

    // ---- Errors / generic ----
    pub error_generic_title: &'static str,
    pub error_not_found_title: &'static str,
    pub error_not_found_lede: &'static str,
    pub error_internal: &'static str,
    pub error_too_many_requests_label: &'static str,

    // ---- Audit log labels ----
    pub audit_title: &'static str,
    pub audit_col_when: &'static str,
    pub audit_col_actor: &'static str,
    pub audit_col_action: &'static str,
    pub audit_col_target: &'static str,
    pub audit_col_outcome: &'static str,
    pub audit_col_note: &'static str,
    // ---- Settings tab (RFC 023 renames "Other" → "Advanced") ----
    pub settings_tab_advanced: &'static str,

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
    pub dashboard_oidc_endpoints_section: &'static str,
    // ---- Admin: Dashboard operator prompts (RFC 031) ----
    pub dashboard_warn_smtp: &'static str,
    pub dashboard_warn_hibp: &'static str,
    pub dashboard_warn_cookie_insecure: &'static str,
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
    // ---- Settings: common sections (RFC 029 second pass) ----
    pub settings_page_title_template: &'static str, // "{tab} — Settings"
    // Basic tab sections
    pub settings_basic_description: &'static str,
    // Security tab sections
    pub settings_security_session_section: &'static str,
    pub settings_security_session_lede: &'static str,
    pub settings_security_idle_timeout_label: &'static str,
    pub settings_security_max_sessions_label: &'static str,
    pub settings_security_lockout_section: &'static str,
    pub settings_security_headers_section: &'static str,
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




    // ---- Admin: Users (RFC 029) ----
    pub users_title: &'static str,
    pub users_lede: &'static str,
    pub users_create_section: &'static str,
    pub users_create_button: &'static str,
    pub users_table_section: &'static str,
    pub users_table_th_display: &'static str,
    pub users_table_th_status: &'static str,
    pub users_table_th_mfa: &'static str,
    pub users_table_th_created: &'static str,
    pub users_is_admin_label: &'static str,
    pub users_empty: &'static str,

    // ---- Admin: Clients (RFC 029) ----
    pub clients_title: &'static str,
    pub clients_lede: &'static str,
    pub clients_create_section: &'static str,
    pub clients_table_section: &'static str,
    pub clients_secret_once_banner: &'static str,
    pub clients_table_th_name: &'static str,
    pub clients_table_th_kind: &'static str,
    pub clients_table_th_scopes: &'static str,
    pub clients_table_th_logout: &'static str,
    pub clients_table_th_status: &'static str,
    pub clients_empty: &'static str,
    pub clients_single_realm_note: &'static str,

    // ---- Admin: Audit log (RFC 029) ----
    pub audit_lede: &'static str,
    // ---- Audit log enhancements (RFC 033) ----
    pub audit_chain_ok: &'static str,
    pub audit_chain_broken: &'static str,
    pub audit_filter_label: &'static str,
    pub audit_filter_placeholder: &'static str,
    pub audit_export_csv: &'static str,


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
