use crate::pages::common::{Flash, flash_banner, render};
use leptos::prelude::*;

use super::setup_step_indicator;

/// Step 1 of 3 — welcome.
///
/// v0.48.4: accepts `token` (the setup token from the startup URL) so
/// the "Begin setup" button can carry it to `/setup/admin?token=xxx`.
/// The zh language option is removed from the picker (v0.48.4);
/// the core i18n support covers only ja and en.
pub fn render_setup_welcome(
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
    token: &str,
) -> String {
    let token = token.to_owned();
    render(move || {
        let t = lang.strings();
        let current_tag = lang.tag();
        let admin_href = if token.is_empty() {
            "/setup/admin".to_owned()
        } else {
            format!("/setup/admin?token={token}")
        };
        let lang_ja = if token.is_empty() {
            "/setup?lang=ja".to_owned()
        } else {
            format!("/setup?lang=ja&token={token}")
        };
        let lang_en = if token.is_empty() {
            "/setup?lang=en".to_owned()
        } else {
            format!("/setup?lang=en&token={token}")
        };
        view! {
            <crate::layout::AuthShell title=t.setup_welcome_title.to_string() lang=lang>
                {setup_step_indicator(0, lang)}
                <nav class="setup-lang-picker" aria-label=t.setup_welcome_lang_picker_label>
                    <a href=lang_ja
                       class={if current_tag == "ja" { "setup-lang-picker__opt setup-lang-picker__opt--active" }
                              else { "setup-lang-picker__opt" }}
                       aria-current={if current_tag == "ja" { "true" } else { "false" }}>
                        "日本語"
                    </a>
                    <a href=lang_en
                       class={if current_tag == "en" { "setup-lang-picker__opt setup-lang-picker__opt--active" }
                              else { "setup-lang-picker__opt" }}
                       aria-current={if current_tag == "en" { "true" } else { "false" }}>
                        "English"
                    </a>
                </nav>
                <h1>{t.setup_welcome_title}</h1>
                <p class="muted">{t.setup_welcome_lede}</p>
                <p class="muted">{t.setup_welcome_lede2}</p>
                {flash_banner(flash)}
                <p class="mt-4">
                    <a href=admin_href class="button">{t.setup_welcome_begin}</a>
                </p>
            </crate::layout::AuthShell>
        }
    })
}
