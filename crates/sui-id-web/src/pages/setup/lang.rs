use crate::pages::common::{Flash, flash_banner, render};
use leptos::prelude::*;

use super::setup_step_indicator;

/// Step 3 of 5 — language selection (RFC 012).
pub fn render_setup_lang(flash: Option<Flash>, current: &str, lang: sui_id_i18n::Locale) -> String {
    let current = current.to_owned();
    render(move || {
        let t = lang.strings();
        let ja_checked = current.is_empty() || current == "ja";
        let en_checked = current == "en";
        view! {
            <crate::layout::AuthShell title=t.setup_lang_title.to_string() lang=lang>
                {setup_step_indicator(2, lang)}
                <h1>{t.setup_lang_title}</h1>
                <p class="muted">{t.setup_lang_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/setup/lang" class="stack">
                    <fieldset class="button-reset">
                        <legend class="field__label">{t.setup_lang_field_label}</legend>
                        <div class="stack gap-2">
                            <label class="row row-gap2-center-clickable">
                                <input type="radio" name="lang" value="ja"
                                       checked=ja_checked />
                                <span>{t.locale_native_ja}</span>
                            </label>
                            <label class="row row-gap2-center-clickable">
                                <input type="radio" name="lang" value="en"
                                       checked=en_checked />
                                <span>{t.locale_native_en}</span>
                            </label>
                        </div>
                    </fieldset>
                    <p class="muted text-caption">{t.setup_lang_default_note}</p>
                    <div class="row justify-end">
                        <button type="submit">{t.setup_lang_submit}</button>
                    </div>
                </form>
            </crate::layout::AuthShell>
        }
    })
}
