//! /me/security language (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::super::common::*;
use super::*;  // MeShellData + MeTab + me_security_tabs

pub struct MeLanguageData {
    pub shell: MeShellData,
    pub current_preferred_lang: Option<String>,
    pub csrf_token: String,
    /// True when the page was rendered immediately after a successful
    /// POST. The view shows a localised success banner (RFC 057).
    pub just_saved: bool,
}


pub fn render_me_language(
    data: MeLanguageData,
    flash: Option<Flash>,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Language, lang);
        let MeLanguageData { shell: _, current_preferred_lang, csrf_token, just_saved } = data;
        let cur = current_preferred_lang.clone().unwrap_or_default();
        let cur2 = cur.clone();
        let cur3 = cur.clone();
        let cur4 = cur.clone();
        view! {
            <Shell title=t.me_language_title.to_string() show_nav=true current=Some("me".to_string()) lang=lang>
                <header class="page-header"><h1 class="page-header__title">{t.me_language_title}</h1></header>
                {tabs}
                {flash_banner(flash)}
                {just_saved.then(|| view! {
                    <div class="banner banner--success mt-3" role="status">
                        {t.me_security_language_saved_banner}
                    </div>
                })}
                <div class="card mt-4">
                    <p class="muted">{t.me_language_lede}</p>
                    <form method="post" action="/me/security/language" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token/>
                        <div class="field">
                            <div class="stack gap-2">
                                <label class="row row-gap2-center">
                                    <input type="radio" name="locale" value=""
                                           checked=move || cur.is_empty()/>
                                    {t.me_language_use_default}
                                </label>
                                <label class="row row-gap2-center">
                                    <input type="radio" name="locale" value="ja"
                                           checked=move || cur2 == "ja"/>
                                    {t.locale_native_ja}
                                </label>
                                <label class="row row-gap2-center">
                                    <input type="radio" name="locale" value="en"
                                           checked=move || cur3 == "en"/>
                                    {t.locale_native_en}
                                </label>
                                <label class="row row-gap2-center">
                                    <input type="radio" name="locale" value="zh"
                                           checked=move || cur4 == "zh"/>
                                    {t.locale_native_zh}
                                </label>
                            </div>
                        </div>
                        <div>
                            <button type="submit">{t.button_save}</button>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}
