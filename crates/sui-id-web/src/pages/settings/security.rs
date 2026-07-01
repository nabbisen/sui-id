//! Settings security tab (RFC 065).

use super::super::common::*;
use super::*;
use crate::layout::Shell;

pub struct SettingsSecurityData {
    pub max_lockout_label: String,
    pub hsts_enabled: bool,
    pub csp_enabled: bool,
    pub x_frame_deny: bool,
    pub permissions_policy_minimal: bool,
    pub cors_token_dynamic_from_clients: bool,
    pub cors_public_endpoints_open: bool,
    /// v0.25.0 — current value in seconds, 0 = disabled.
    pub idle_session_timeout_secs: i64,
    /// v0.25.0 — current cap, 0 = disabled.
    pub max_concurrent_sessions: i64,
    /// CSRF token for the inline edit forms. Empty string is
    /// tolerated (forms no-op without it) but production callers
    /// should always pass a real token.
    pub csrf_token: String,
}

pub fn render_settings_security(
    data: SettingsSecurityData,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsSecurityData {
            max_lockout_label,
            hsts_enabled,
            csp_enabled,
            x_frame_deny,
            permissions_policy_minimal,
            cors_token_dynamic_from_clients,
            cors_public_endpoints_open,
            idle_session_timeout_secs,
            max_concurrent_sessions,
            csrf_token,
        } = data;
        let csrf_for_idle = csrf_token.clone();
        let csrf_for_cap = csrf_token.clone();
        let session_forms = view! {
            <section class="section">
                <h2 class="section__title">{t.settings_security_session_section}</h2>
                <p class="muted">
                    {t.settings_security_session_lede}
                </p>
                <div class="card">
                    <form method="post" action="/admin/settings/security/idle-timeout" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_for_idle />
                        <div class="field">
                            <label for="idle-timeout" class="field__label">
                                {t.settings_security_idle_timeout_label}
                            </label>
                            <input id="idle-timeout" name="secs" type="number"
                                   min="0" max="2592000"
                                   value=idle_session_timeout_secs.to_string() />
                            <span class="field__hint">
                                {t.settings_security_idle_timeout_hint}
                            </span>
                        </div>
                        <div>
                            <button type="submit">{t.button_save}</button>
                        </div>
                    </form>
                </div>
                <div class="card">
                    <form method="post" action="/admin/settings/security/max-sessions" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_for_cap />
                        <div class="field">
                            <label for="max-sessions" class="field__label">
                                {t.settings_security_max_sessions_label}
                            </label>
                            <input id="max-sessions" name="cap" type="number"
                                   min="0" max="1000"
                                   value=max_concurrent_sessions.to_string() />
                            <span class="field__hint">
                                {t.settings_security_max_sessions_hint}
                            </span>
                        </div>
                        <div>
                            <button type="submit">{t.button_save}</button>
                        </div>
                    </form>
                </div>
            </section>
        };
        view! {
            <Shell title=t.settings_title_security.to_string() show_nav=true current=Some("settings".to_string()) lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_basic_description}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Security, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_security_lockout_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code(t.settings_security_lockout_section, max_lockout_label)}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted mt-2-mb-0">
                        {t.settings_security_lockout_hint_1}
                        " "
                        {t.settings_security_lockout_hint_2_pre}
                        <span class="code">"sui-id admin unlock-user"</span>
                        {t.settings_security_lockout_hint_2_post}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_security_headers_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge(t, "HSTS (Strict-Transport-Security)", hsts_enabled)}
                                {kv_bool_badge(t, "Content-Security-Policy", csp_enabled)}
                                {kv_bool_badge(t, "X-Frame-Options: DENY", x_frame_deny)}
                                {kv_bool_badge(t, t.settings_security_headers_perm_policy_label, permissions_policy_minimal)}
                            </tbody>
                        </table>
                    </div>
                    <p class="muted mt-2-mb-0">
                        {t.settings_security_headers_hint}
                    </p>
                </div>

                <div class="card">
                    <h3 class="card__title">"CORS"</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_bool_badge(t, t.settings_security_cors_token_label, cors_token_dynamic_from_clients)}
                                {kv_bool_badge(t, t.settings_security_cors_public_label, cors_public_endpoints_open)}
                            </tbody>
                        </table>
                    </div>
                </div>
                {session_forms}
            </Shell>
        }
    })
}
