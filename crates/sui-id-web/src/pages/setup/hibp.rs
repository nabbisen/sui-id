use crate::pages::common::{Flash, flash_banner, render};
use leptos::prelude::*;

use super::setup_step_indicator;

/// Step 4 of 5 — HIBP policy selection (RFC 012).
pub fn render_setup_hibp(flash: Option<Flash>, current: &str, lang: sui_id_i18n::Locale) -> String {
    let current = current.to_owned();
    render(move || {
        let t = lang.strings();
        let off_checked = current == "off";
        let warn_checked = current.is_empty() || current == "warn";
        let block_checked = current == "block";
        view! {
            <crate::layout::AuthShell title=t.setup_hibp_step_title.to_string() lang=lang>
                {setup_step_indicator(3, lang)}
                <h1>{t.setup_hibp_step_title}</h1>
                <p class="muted">{t.setup_hibp_step_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/setup/hibp" class="stack">
                    <fieldset class="button-reset">
                        <div class="stack gap-3">
                            <label class="card clickable-block">
                                <div class="row row-gap2-center">
                                    <input type="radio" name="hibp_mode" value="off"
                                           checked=off_checked />
                                    <strong>{t.setup_hibp_option_off}</strong>
                                </div>
                                <p class="muted radio-hint">{t.setup_hibp_option_off_desc}</p>
                            </label>
                            <label class="card clickable-block">
                                <div class="row row-gap2-center">
                                    <input type="radio" name="hibp_mode" value="warn"
                                           checked=warn_checked />
                                    <strong>{t.setup_hibp_option_warn}</strong>
                                </div>
                                <p class="muted radio-hint">{t.setup_hibp_option_warn_desc}</p>
                            </label>
                            <label class="card clickable-block">
                                <div class="row row-gap2-center">
                                    <input type="radio" name="hibp_mode" value="block"
                                           checked=block_checked />
                                    <strong>{t.setup_hibp_option_block}</strong>
                                </div>
                                <p class="muted radio-hint">{t.setup_hibp_option_block_desc}</p>
                            </label>
                        </div>
                    </fieldset>
                    <p class="muted text-caption">{t.setup_hibp_step_default_note}</p>
                    <div class="row justify-end">
                        <button type="submit">{t.setup_hibp_step_submit}</button>
                    </div>
                </form>
            </crate::layout::AuthShell>
        }
    })
}
