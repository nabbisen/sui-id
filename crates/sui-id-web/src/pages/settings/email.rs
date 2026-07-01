//! Settings email tab (RFC 065).

use super::super::common::*;
use super::*;
use crate::layout::Shell;

pub struct SettingsEmailData {
    pub configured: bool,
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub tls_mode: String,
    pub username: String,
    pub has_password: bool,
    pub from_address: String,
    pub from_name: String,
    pub base_url: String,
}

pub fn render_settings_email(
    data: SettingsEmailData,
    csrf_token: String,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsEmailData {
            configured: _,
            enabled,
            host,
            port,
            tls_mode,
            username,
            has_password,
            from_address,
            from_name,
            base_url,
        } = data;
        let csrf_save = csrf_token.clone();
        let csrf_test = csrf_token.clone();
        let port_str = port.to_string();
        let pw_placeholder = if has_password {
            t.settings_email_password_placeholder_change
        } else {
            t.settings_email_password_placeholder_none
        };
        let enabled_attr = if enabled { Some("checked") } else { None };
        let tls_implicit = if tls_mode == "implicit" {
            Some("selected")
        } else {
            None
        };
        let tls_starttls = if tls_mode == "starttls" {
            Some("selected")
        } else {
            None
        };

        view! {
            <Shell title=t.settings_email_page_title.to_string() show_nav=true current=Some("settings".to_string()) lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_email_lede}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Email, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_email_smtp_section}</h3>
                    <form method="post" action="/admin/settings/email" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_save />
                        <div class="field">
                            <label class="field__label">
                                <input type="checkbox" name="enabled" value="on" checked=enabled_attr />
                                " "{t.settings_email_enable_checkbox}
                            </label>
                            <span class="field__hint">
                                {t.settings_email_enable_hint}
                            </span>
                        </div>
                        <div class="field">
                            <label for="host" class="field__label">{t.settings_email_host_label}</label>
                            <input id="host" name="host" type="text" required=true value=host />
                        </div>
                        <div class="field">
                            <label for="port" class="field__label">{t.settings_email_port_label}</label>
                            <input id="port" name="port" type="number" min="1" max="65535"
                                   required=true value=port_str />
                            <span class="field__hint">{t.settings_email_port_hint}</span>
                        </div>
                        <div class="field">
                            <label for="tls_mode" class="field__label">{t.settings_email_tls_label}</label>
                            <select id="tls_mode" name="tls_mode">
                                <option value="starttls" selected=tls_starttls>"STARTTLS (587)"</option>
                                <option value="implicit" selected=tls_implicit>{t.settings_email_tls_implicit}</option>
                            </select>
                        </div>
                        <div class="field">
                            <label for="username" class="field__label">{t.settings_email_username_label}</label>
                            <input id="username" name="username" type="text"
                                   autocomplete="off" value=username />
                        </div>
                        <div class="field">
                            <label for="password" class="field__label">{t.settings_auth_password_section}</label>
                            <input id="password" name="password" type="password"
                                   autocomplete="off" placeholder=pw_placeholder />
                            <span class="field__hint">
                                {t.settings_email_password_hint}
                            </span>
                        </div>
                        <hr class="divider" />
                        <div class="field">
                            <label for="from_address" class="field__label">{t.settings_email_from_addr_label}</label>
                            <input id="from_address" name="from_address" type="email"
                                   required=true value=from_address />
                        </div>
                        <div class="field">
                            <label for="from_name" class="field__label">{t.settings_email_from_name_label}</label>
                            <input id="from_name" name="from_name" type="text" value=from_name />
                        </div>
                        <div class="field">
                            <label for="base_url" class="field__label">{t.settings_email_base_url_label}</label>
                            <input id="base_url" name="base_url" type="url"
                                   required=true value=base_url
                                   placeholder="https://idp.example.com" />
                            <span class="field__hint">
                                {t.settings_email_base_url_hint}
                            </span>
                        </div>
                        <button type="submit">{t.settings_email_save_button}</button>
                    </form>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_email_test_section}</h3>
                    <p class="muted">
                        {t.settings_email_test_lede}
                    </p>
                    <form method="post" action="/admin/settings/email/test" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_test />
                        <button type="submit" class="secondary">{t.settings_email_test_button}</button>
                    </form>
                </div>
            </Shell>
        }
    })
}
