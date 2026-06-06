//! Settings authentication tab (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::super::common::*;
use super::*;  // SettingsTab + settings_tabs

pub struct SettingsAuthenticationData {
    pub password_min_length: usize,
    pub password_argon2id: String,
    pub totp_enabled_per_user: bool,
    pub webauthn_enabled_per_user: bool,
    pub recovery_codes_per_enrollment: usize,
    pub pkce_required: bool,
    pub access_token_lifetime_secs: i64,
    pub id_token_lifetime_secs: i64,
    pub refresh_token_lifetime_secs: i64,
    pub refresh_rotation: bool,
    pub refresh_theft_detection: bool,
}


fn fmt_lifetime(t: &'static sui_id_i18n::Strings, secs: i64) -> String {
    if secs % 86400 == 0 {
        (t.fmt_lifetime_days)(secs / 86400, secs)
    } else if secs % 3600 == 0 {
        (t.fmt_lifetime_hours)(secs / 3600, secs)
    } else if secs % 60 == 0 {
        (t.fmt_lifetime_minutes)(secs / 60, secs)
    } else {
        format!("{secs} s")
    }
}


pub fn render_settings_authentication(
    data: SettingsAuthenticationData,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsAuthenticationData {
            password_min_length,
            password_argon2id,
            totp_enabled_per_user,
            webauthn_enabled_per_user,
            recovery_codes_per_enrollment,
            pkce_required,
            access_token_lifetime_secs,
            id_token_lifetime_secs,
            refresh_token_lifetime_secs,
            refresh_rotation,
            refresh_theft_detection,
        } = data;
        view! {
            <Shell title=t.settings_title_authentication.to_string() show_nav=true current=Some("settings".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_basic_description}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Authentication, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_auth_password_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_text(t.settings_auth_min_length_label, (t.settings_auth_min_length_value)(password_min_length))}
                                {kv_text(t.settings_auth_hash_algorithm_label, password_argon2id)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_auth_mfa_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge(t, t.settings_auth_mfa_totp, totp_enabled_per_user)}
                                {kv_bool_badge(t, t.settings_auth_mfa_passkey, webauthn_enabled_per_user)}
                                {kv_text(t.settings_auth_recovery_codes_label, (t.settings_auth_recovery_codes_value)(recovery_codes_per_enrollment))}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted mt-2-mb-0">
                        {t.settings_auth_mfa_note_prefix}
                        <a href="/admin/profile">"/admin/profile"</a>
                        {t.settings_auth_mfa_note_suffix}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"OAuth 2.1 / OIDC"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge(t, t.settings_auth_pkce_required, pkce_required)}
                                {kv_text(t.settings_auth_access_token_ttl, fmt_lifetime(t, access_token_lifetime_secs))}
                                {kv_text(t.settings_auth_id_token_ttl, fmt_lifetime(t, id_token_lifetime_secs))}
                                {kv_text(t.settings_auth_refresh_token_ttl, fmt_lifetime(t, refresh_token_lifetime_secs))}
                                {kv_bool_badge(t, t.settings_auth_refresh_rotate, refresh_rotation)}
                                {kv_bool_badge(t, t.settings_auth_refresh_theft, refresh_theft_detection)}
                            </tbody>
                        </table>
                    </div>
                </div>
            </Shell>
        }
    })
}
