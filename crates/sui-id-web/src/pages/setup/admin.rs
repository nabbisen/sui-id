use crate::pages::common::{Flash, flash_banner, render};
use leptos::prelude::*;

use super::setup_step_indicator;

/// Step 2 of 3 — admin form.
///
/// v0.48.4: `token` is now a hidden input pre-filled from the URL
/// parameter rather than a visible text field the operator had to
/// type. The POST handler validates it from the form body unchanged.
pub fn render_setup_admin(flash: Option<Flash>, lang: sui_id_i18n::Locale, token: &str) -> String {
    let token = token.to_owned();
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.setup_admin_title.to_string() lang=lang>
                {setup_step_indicator(1, lang)}
                <h1>{t.setup_admin_title}</h1>
                <p class="muted">{t.setup_admin_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/setup/admin" class="stack" autocomplete="off">
                    <input type="hidden" name="setup_token" value=token />
                    <div class="field">
                        <label for="username" class="field__label">{t.setup_admin_username_label}</label>
                        <input id="username" name="username" type="text"
                               required=true autocomplete="username" autofocus=true />
                    </div>
                    <div class="field">
                        <label for="email" class="field__label">{t.setup_admin_email_label}</label>
                        <input id="email" name="email" type="email" autocomplete="email" />
                        <span class="field__hint">{t.setup_admin_email_hint}</span>
                    </div>
                    <div class="field">
                        <label for="display" class="field__label">{t.setup_admin_display_label}</label>
                        <input id="display" name="display_name" type="text" autocomplete="name" />
                    </div>
                    <div class="field">
                        <label for="password" class="field__label">{t.setup_admin_password_label}</label>
                        <input id="password" name="password" type="password"
                               required=true minlength="12" autocomplete="new-password" />
                        <span class="field__hint">{t.setup_admin_password_hint}</span>
                    </div>
                    <div class="field">
                        <label for="confirm_password" class="field__label">{t.setup_admin_confirm_label}</label>
                        <input id="confirm_password" name="confirm_password" type="password"
                               required=true minlength="12" autocomplete="new-password" />
                    </div>
                    <div class="row">
                        <a href="/setup" class="button secondary">{t.button_back}</a>
                        <button type="submit">{t.setup_admin_submit}</button>
                    </div>
                </form>
            </crate::layout::AuthShell>
        }
    })
}
