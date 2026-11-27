//! Settings authentication tab (RFC 065).

use super::*;
use crate::layout::{Chrome, NavItem, Shell};

pub struct SettingsAuthenticationData {
    /// The *stored* `server_settings.password_min_len` -- the value
    /// this tab's own form edits. Not the value a password is
    /// actually checked against in dev mode (always 8 there,
    /// RFC 151 §2); `is_dev_mode` below is what says so.
    pub password_min_length: usize,
    pub password_min_length_floor: usize,
    pub password_min_length_ceiling: usize,
    pub is_dev_mode: bool,
    /// Dev mode's own fixed minimum (8) -- computed by the caller,
    /// which depends on `sui-id-core`; this crate does not.
    pub password_min_length_dev_floor: usize,
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
    /// RFC 088: false for auditors → render static rows instead of form.
    pub can_write: bool,
    /// RFC 144 (Refresh, §4): the signed-in administrator's name, for
    /// the account menu.
    pub username: String,
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
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsAuthenticationData {
            password_min_length,
            password_min_length_floor,
            password_min_length_ceiling,
            is_dev_mode,
            password_min_length_dev_floor,
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
            can_write,
            username,
        } = data;
        let csrf_for_min_length = csrf_token.clone();
        let min_length_row = if can_write {
            view! {
                <form method="post" action="/admin/settings/authentication/password-min-len" class="stack">
                    <input type="hidden" name="_csrf" value=csrf_for_min_length />
                    <div class="field">
                        <label for="password-min-len" class="field__label">
                            {t.settings_auth_min_length_label}
                        </label>
                        <input id="password-min-len" name="password_min_len" type="number"
                               min=password_min_length_floor.to_string()
                               max=password_min_length_ceiling.to_string()
                               value=password_min_length.to_string() />
                        <span class="field__hint">
                            {(t.settings_auth_min_length_hint)(password_min_length_floor)}
                        </span>
                        {is_dev_mode.then(|| view! {
                            <p class="muted mt-2-mb-0">
                                {(t.settings_auth_min_length_dev_note)(password_min_length_dev_floor)}
                            </p>
                        })}
                    </div>
                    <div>
                        <button type="submit">{t.button_save}</button>
                    </div>
                </form>
            }
            .into_any()
        } else {
            view! {
                <div class="table-wrap">
                    <table>
                        <tbody>
                            {kv_text(t.settings_auth_min_length_label, (t.settings_auth_min_length_value)(password_min_length))}
                        </tbody>
                    </table>
                </div>
            }
            .into_any()
        };
        view! {
            <Shell title=NavItem::SignInSettings.label(t).to_string() nav=Chrome::Console{username, item: NavItem::SignInSettings} lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <p class="page-header__group">{NavItem::SignInSettings.group().label(t)}</p>
                        <h1 class="page-header__title">{NavItem::SignInSettings.label(t)}</h1>
                        <p class="page-header__lede">
                            {t.settings_authentication_lede}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card">
                    <h3 class="card__title">
                        {t.settings_auth_password_section}
                        {crate::components::help_details(
                            t,
                            t.help_about_min_length.to_string(),
                            t.help_min_length_body,
                            concat!(
                                crate::github_doc_url!("docs/src/guides/operators.md"),
                                "#password-minimum-length",
                            ),
                        )}
                    </h3>
                    {min_length_row}
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_text(t.settings_auth_hash_algorithm_label, password_argon2id)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">
                        {t.settings_auth_mfa_section}
                        {crate::components::help_details(
                            t,
                            t.help_about_two_step.to_string(),
                            t.help_two_step_body,
                            concat!(
                                crate::github_doc_url!("docs/src/guides/operators.md"),
                                "#multi-factor-authentication",
                            ),
                        )}
                    </h3>
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
