use crate::pages::common::render;
use leptos::prelude::*;

use super::setup_step_indicator;

/// Step 5 of 5 — completion.
pub fn render_setup_done(initialized: bool, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        if initialized {
            view! {
                <crate::layout::AuthShell title=t.setup_done_title.to_string() lang=lang>
                    {setup_step_indicator(4, lang)}
                    <h1>{t.setup_done_title}</h1>
                    <p class="muted">{t.setup_done_lede}</p>
                    <div class="card card--callout">
                        <h3 class="card__title">{t.setup_done_next_steps_title}</h3>
                        <ul class="muted ul-indent">
                            <li>{t.setup_done_next_step_register_clients}</li>
                            <li>{t.setup_done_next_step_enable_mfa}</li>
                            <li>{t.setup_done_next_step_review_settings}</li>
                        </ul>
                    </div>
                    <p class="mt-4">
                        <a href="/admin" class="button">{t.setup_done_enter_admin}</a>
                    </p>
                </crate::layout::AuthShell>
            }
            .into_any()
        } else {
            view! {
                <crate::layout::AuthShell title=t.setup_not_complete_title.to_string() lang=lang>
                    {setup_step_indicator(0, lang)}
                    <h1>{t.setup_not_complete_title}</h1>
                    <p class="muted">{t.setup_not_complete_lede}</p>
                    <p class="mt-4">
                        <a href="/setup" class="button">{t.setup_welcome_begin}</a>
                    </p>
                </crate::layout::AuthShell>
            }
            .into_any()
        }
    })
}
