//! Settings basic tab (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::super::common::*;
use super::*;  // SettingsTab + settings_tabs

pub struct SettingsBasicData {
    pub issuer: String,
    pub listen_addr: String,
    pub cookie_secure: bool,
    pub trusted_proxies: Vec<String>,
    pub discovery_url: String,
    pub jwks_url: String,
    /// Server-wide default UI language (BCP-47 tag, e.g. "ja").
    /// Comes from `server_settings.default_lang`. Editable via the
    /// form on this page; saved through `POST /admin/settings/basic/lang`.
    pub default_lang: String,
    /// CSRF token for the in-page edit form. Empty string when
    /// rendered without CSRF (legacy callers); the lang form
    /// no-ops when the token is missing.
    pub csrf_token: String,
}


pub fn render_settings_basic(data: SettingsBasicData, flash: Option<Flash>, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let SettingsBasicData {
            issuer,
            listen_addr,
            cookie_secure,
            trusted_proxies,
            discovery_url,
            jwks_url,
            default_lang,
            csrf_token,
        } = data;
        let csrf_for_lang = csrf_token.clone();
        let lang_form = view! {
            <section class="section">
                <h2 class="section__title">{t.settings_basic_default_lang}</h2>
                <p class="muted">
                    {t.settings_basic_default_lang_hint}
                </p>
                <div class="card">
                    <form method="post" action="/admin/settings/basic/lang" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_for_lang />
                        <div class="field">
                            <label for="default-lang-select" class="field__label">
                                {t.profile_lang_label}
                            </label>
                            <select id="default-lang-select" name="default_lang">
                                {sui_id_i18n::Locale::ALL.iter().map(|loc| {
                                    let tag = loc.tag();
                                    let selected = default_lang == tag;
                                    let label = loc.native_name();
                                    view! {
                                        <option value=tag selected=selected>{label}</option>
                                    }
                                }).collect::<Vec<_>>()}
                            </select>
                        </div>
                        <div>
                            <button type="submit">{t.button_save}</button>
                        </div>
                    </form>
                </div>
            </section>
        };
        let proxies_display = if trusted_proxies.is_empty() {
            t.settings_basic_trusted_proxies_none.to_owned()
        } else {
            trusted_proxies.join(", ")
        };
        view! {
            <Shell title=t.settings_title_basic.to_string() show_nav=true current=Some("settings".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.settings_title}</h1>
                        <p class="page-header__lede">
                            {t.settings_basic_description}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {settings_tabs(SettingsTab::Basic, lang)}

                <div class="card">
                    <h3 class="card__title">{t.settings_tab_basic}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                {kv_code(t.settings_basic_kv_issuer, issuer)}
                                {kv_code(t.settings_basic_kv_listen, listen_addr)}
                                {kv_bool_badge(t, t.settings_basic_kv_cookie_secure, cookie_secure)}
                                {kv_text(t.settings_basic_kv_trusted_proxies, proxies_display)}
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.settings_basic_oidc_section}</h3>
                    <div class="table-wrap">
                        <table>
                            <tbody>
                                <tr>
                                    <th scope="row" class="kv-label-cell">{t.dashboard_oidc_endpoint_discovery}</th>
                                    <td>
                                        {
                                            let url = discovery_url.clone();
                                            view! {
                                                <a href=discovery_url>
                                                    <span class="code">{url}</span>
                                                </a>
                                            }
                                        }
                                    </td>
                                </tr>
                                <tr>
                                    <th scope="row" class="kv-label-cell">{t.dashboard_oidc_endpoint_jwks}</th>
                                    <td>
                                        {
                                            let url = jwks_url.clone();
                                            view! {
                                                <a href=jwks_url>
                                                    <span class="code">{url}</span>
                                                </a>
                                            }
                                        }
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
                {lang_form}
            </Shell>
        }
    })
}
