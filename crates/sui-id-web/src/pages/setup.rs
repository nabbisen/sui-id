//! Page renderers for the "setup" screen domain (RFC 065).

use leptos::prelude::*;
use super::common::*;

fn setup_step_indicator(active: usize, lang: sui_id_i18n::Locale) -> impl IntoView {
    // Five labelled dots showing which step the operator is on.
    // Steps are: Welcome(0), Admin(1), Language(2), HIBP(3), Done(4).
    let t = lang.strings();
    let labels = [
        t.setup_step_welcome,
        t.setup_step_admin,
        t.setup_step_lang,
        t.setup_step_hibp,
        t.setup_step_done,
    ];
    let dots: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_active = i == active;
            let aria = if is_active { Some("step") } else { None };
            let badge = if i < active {
                view! { <span class="badge badge--ok">{format!("{}", i + 1)}</span> }
                    .into_any()
            } else if is_active {
                view! { <span class="badge badge--accent">{format!("{}", i + 1)}</span> }
                    .into_any()
            } else {
                view! { <span class="badge">{format!("{}", i + 1)}</span> }.into_any()
            };
            let style = if is_active {
                "color:var(--fg-default);font-weight:var(--font-weight-medium)"
            } else if i < active {
                "color:var(--fg-muted)"
            } else {
                "color:var(--fg-subtle)"
            };
            view! {
                <span class="row gap1-center" aria-current=aria>
                    {badge}
                    <span style=style>{*label}</span>
                </span>
            }
        })
        .collect();
    view! {
        <nav class="row"
             aria-label=t.setup_steps_aria
             style="gap:var(--space-3);justify-content:center;margin-bottom:var(--space-4);flex-wrap:wrap;font-size:var(--font-size-caption)">
            {dots}
        </nav>
    }
}

/// Step 1 of 3 — welcome.
///
/// v0.48.4: accepts `token` (the setup token from the startup URL) so
/// the "Begin setup" button can carry it to `/setup/admin?token=xxx`.
/// The zh language option is removed from the picker (v0.48.4);
/// the core i18n support covers only ja and en.
pub fn render_setup_welcome(flash: Option<Flash>, lang: sui_id_i18n::Locale, token: &str) -> String {
    let token = token.to_owned();
    render(move || {
        let t = lang.strings();
        let current_tag = lang.tag();
        let admin_href = if token.is_empty() {
            "/setup/admin".to_owned()
        } else {
            format!("/setup/admin?token={token}")
        };
        // Language picker links also carry the token through the PRG redirect.
        let lang_ja = if token.is_empty() { "/setup?lang=ja".to_owned() }
                      else { format!("/setup?lang=ja&token={token}") };
        let lang_en = if token.is_empty() { "/setup?lang=en".to_owned() }
                      else { format!("/setup?lang=en&token={token}") };
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
                    // v0.48.4: token is carried as a hidden field, pre-filled
                    // from the URL parameter. Operators no longer see or type it.
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
