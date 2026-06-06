//! Page renderers for the "auth" screen domain (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::common::*;

pub fn render_login(
    flash: Option<Flash>,
    next: Option<String>,
    lang: sui_id_i18n::Locale,
    // When true, renders a passkey sign-in button above the password form (RFC 034).
    show_passkey_option: bool,
) -> String {
    render(move || {
        let next_value = next.clone().unwrap_or_default();
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.login_title.to_string() lang=lang>
                <h1>{t.login_title}</h1>
                {flash_banner(flash)}
                {show_passkey_option.then(|| view! {
                    <form id="passkey-login-form" method="post"
                          action="/admin/login/webauthn/start" class="stack">
                        <button type="submit">{t.login_passkey_primary}</button>
                    </form>
                    <div class="divider-with-label" aria-hidden="true">
                        <span>"or"</span>
                    </div>
                })}
                <form method="post" action="/admin/login" class="stack">
                    <input type="hidden" name="next" value=next_value />
                    <div class="field">
                        <label for="username" class="field__label">{t.login_username_label}</label>
                        <input id="username" name="username" type="text"
                               required=true autocomplete="username"
                               autofocus=true />
                    </div>
                    <div class="field">
                        <label for="password" class="field__label">{t.login_password_label}</label>
                        <input id="password" name="password" type="password"
                               required=true autocomplete="current-password" />
                    </div>
                    <button type="submit">{t.login_submit}</button>
                </form>
                <p class="muted" style="margin-top:var(--space-3);text-align:center;font-size:var(--font-size-caption)">
                    <a href="/forgot-password">{t.login_forgot_password_link}</a>
                </p>
                {show_passkey_option.then(|| view! {
                    <script src="/static/webauthn.js"></script>
                })}
            </crate::layout::AuthShell>
        }
    })
}

// ---------- MFA challenge ----------


pub fn render_mfa_challenge(
    flash: Option<Flash>,
    csrf_token: String,
    has_passkey: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let csrf_for_totp = csrf_token.clone();
        let csrf_for_pk = csrf_token.clone();
        let passkey_block = if has_passkey {
            view! {
                <hr class="divider" />
                <p class="muted">{t.mfa_challenge_passkey_alt}</p>
                <form id="passkey-auth-form" method="post"
                      action="/admin/login/webauthn/start" class="stack">
                    <input type="hidden" name="_csrf" value=csrf_for_pk />
                    <button type="submit" class="secondary">{t.mfa_challenge_passkey_button}</button>
                </form>
                <script src="/static/webauthn.js"></script>
            }
            .into_any()
        } else {
            view! { <></> }.into_any()
        };
        view! {
            <crate::layout::AuthShell title=t.mfa_challenge_shell_title.to_string() lang=lang>
                <h1>{t.mfa_challenge_title}</h1>
                {flash_banner(flash)}
                <p class="muted">{t.mfa_challenge_lede}</p>
                <form method="post" action="/admin/login/mfa" class="stack">
                    <input type="hidden" name="_csrf" value=csrf_for_totp />
                    <div class="field">
                        <label for="code" class="field__label">{t.mfa_challenge_code_label}</label>
                        <input id="code" name="code" type="text"
                               required=true autocomplete="one-time-code"
                               inputmode="text" autofocus=true />
                    </div>
                    <button type="submit">{t.mfa_challenge_submit}</button>
                </form>
                {passkey_block}
            </crate::layout::AuthShell>
        }
    })
}

// ---------- profile (MFA settings) ----------


pub struct PasskeyDescriptor {
    pub id: String,
    pub nickname: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}



pub struct MfaSetupData {
    /// otpauth:// URI for the QR code
    pub otpauth_uri: String,
    /// Pre-rendered SVG of the QR code (full <svg>...</svg> string).
    pub qr_svg: String,
    /// Base32-encoded secret string for users who would rather type it
    /// in than scan the QR code.
    pub secret_b32: String,
}


pub fn render_mfa_setup(
    data: MfaSetupData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let MfaSetupData { otpauth_uri, qr_svg, secret_b32 } = data;
        view! {
            <Shell title=t.mfa_setup_shell_title.to_string() show_nav=true current=Some("me".to_string()) lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.mfa_setup_title}</h1>
                        <p class="page-header__lede">{t.mfa_setup_lede}</p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card">
                    <h3 class="card__title">{t.mfa_setup_steps_title}</h3>
                    <ol>
                        <li>{t.mfa_setup_step1}</li>
                        <li>{t.mfa_setup_step2}</li>
                        <li>{t.mfa_setup_step3}</li>
                    </ol>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.mfa_setup_qr_card_title}</h3>
                    <div inner_html=qr_svg style="max-width:240px;margin-bottom:var(--space-3)"></div>
                    <p>{t.mfa_setup_secret_label}<span class="code ml-1">{secret_b32}</span></p>
                    <details>
                        <summary class="muted">{t.mfa_setup_otpauth_summary}</summary>
                        <p><span class="code">{otpauth_uri}</span></p>
                    </details>
                </div>

                <div class="card">
                    <h3 class="card__title">{t.mfa_setup_verify_card_title}</h3>
                    <form method="post" action="/me/security/mfa/enroll/confirm" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />
                        <div class="field">
                            <label for="code" class="field__label">{t.mfa_setup_code_label}</label>
                            <input id="code" name="code" type="text" required=true
                                   autocomplete="one-time-code" inputmode="text" autofocus=true />
                            <span class="field__hint">{t.mfa_setup_code_hint}</span>
                        </div>
                        <div>
                            <button type="submit">{t.mfa_setup_confirm_button}</button>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}

// ---------- dashboard ----------

/// One bucket of the login-activity sparkline as the renderer
/// wants it: pre-formatted display label, plus the two raw counts.
/// The renderer doesn't need to know the bucket spacing or the
/// range — that's the caller's job.

pub struct PasswordChangeData {
    pub username: String,
    /// Pre-filled checked value of "sign out other sessions". The
    /// caller hands it in so a re-render after a validation error
    /// keeps the user's previous choice.
    pub revoke_others_default: bool,
}


pub fn render_password_change(
    data: PasswordChangeData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let PasswordChangeData {
            username,
            revoke_others_default,
        } = data;
        let revoke_attr = if revoke_others_default { Some("") } else { None };
        view! {
            <Shell title=t.password_change_title.to_owned() show_nav=false current=None lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.password_change_title}</h1>
                        <p class="page-header__lede">
                            <strong>{username}</strong>{t.me_security_signed_in_as_suffix}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}

                <div class="card" style="max-width:var(--content-narrow-width)">
                    <form method="post" action="/me/security/password"
                          autocomplete="off" class="stack">
                        <input type="hidden" name="_csrf" value=csrf_token />

                        <div class="field">
                            <label for="current_password" class="field__label">{t.password_change_current_label}</label>
                            <input type="password" id="current_password" name="current_password"
                                   required autocomplete="current-password" />
                        </div>

                        <div class="field">
                            <label for="new_password" class="field__label">{t.password_change_new_label}</label>
                            <input type="password" id="new_password" name="new_password"
                                   required autocomplete="new-password" minlength="12" />
                            <span class="field__hint">{t.password_change_new_hint}</span>
                        </div>

                        <div class="field">
                            <label for="confirm_password" class="field__label">{t.password_change_confirm_label}</label>
                            <input type="password" id="confirm_password" name="confirm_password"
                                   required autocomplete="new-password" minlength="12" />
                        </div>

                        <div class="field">
                            <label class="row gap-2">
                                <input type="checkbox" name="revoke_others" value="1"
                                       checked=revoke_attr />
                                <span>{t.password_change_revoke_others_label}</span>
                            </label>
                            <span class="field__hint">{t.password_change_revoke_others_hint}</span>
                        </div>

                        <div class="row">
                            <button type="submit">{t.password_change_submit}</button>
                            <a href="/me/security" class="button secondary">{t.button_cancel}</a>
                        </div>
                    </form>
                </div>
            </Shell>
        }
    })
}

// ---------- /admin/settings/* (v0.20.3) ----------
//
// Five read-only tabs surfacing the current effective configuration.
// Each tab is its own route; this view module just renders the
// shell + 5-tab strip + the tab body. The strip is intentionally
// styled with the same `.app-nav__link` vocabulary as the main nav,
// so the visual treatment is consistent: hover, focus ring, and the
// `aria-current="page"` pill all behave the same.

pub fn render_step_up(
    return_to: &str,
    csrf_token: String,
    has_passkey: bool,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    let return_to = return_to.to_owned();
    render(move || {
        let t = lang.strings();
        let return_to_for_input = return_to.clone();
        let csrf_for_passkey = csrf_token.clone();
        let return_to_for_passkey = return_to.clone();
        let passkey_block = if has_passkey {
            view! {
                <hr class="divider" />
                <p class="muted">{format!("{}:", t.step_up_passkey_alt)}</p>
                <form id="step-up-passkey-form" method="post"
                      action="/me/security/step-up/webauthn/start"
                      class="stack">
                    <input type="hidden" name="_csrf" value=csrf_for_passkey />
                    <input type="hidden" name="return_to" value=return_to_for_passkey />
                    <button type="submit" class="secondary">{t.step_up_passkey_button}</button>
                </form>
                <script src="/static/step-up-webauthn.js"></script>
            }
            .into_any()
        } else {
            view! { <></> }.into_any()
        };
        view! {
            <Shell title=t.step_up_title.to_string() show_nav=false current=None lang=lang>
                <div class="auth-page">
                    <div class="auth-card">
                        <h1>{t.step_up_title}</h1>
                        <p class="muted">{t.step_up_lede}</p>
                        {flash_banner(flash)}
                        <form method="post" action="/me/security/step-up"
                              autocomplete="off" class="stack">
                            <input type="hidden" name="_csrf" value=csrf_token />
                            <input type="hidden" name="return_to" value=return_to_for_input />
                            <div class="field">
                                <label for="code" class="field__label">{t.step_up_code_label}</label>
                                <input id="code" name="code" type="text"
                                       required=true
                                       autocomplete="one-time-code"
                                       inputmode="text"
                                       autofocus=true />
                                <span class="field__hint">{t.step_up_code_hint}</span>
                            </div>
                            <div class="row">
                                <button type="submit">{t.button_confirm}</button>
                                <a href="/me/security" class="button secondary">{t.button_cancel}</a>
                            </div>
                        </form>
                        {passkey_block}
                    </div>
                </div>
            </Shell>
        }
    })
}

// ---------- /forgot-password & /reset-password (v0.22.0) ----------


pub fn render_forgot_password(
    csrf_token: String,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.forgot_password_title.to_string() lang=lang>
                <h1>{t.forgot_password_title}</h1>
                <p class="muted">{t.forgot_password_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/forgot-password" class="stack">
                    <input type="hidden" name="_csrf" value=csrf_token />
                    <div class="field">
                        <label for="email" class="field__label">{t.forgot_password_email_label}</label>
                        <input id="email" name="email" type="email"
                               required=true autocomplete="email" autofocus=true />
                    </div>
                    <div class="row">
                        <button type="submit">{t.forgot_password_submit}</button>
                        <a href="/admin/login" class="button secondary">{t.back_to_login}</a>
                    </div>
                </form>
            </crate::layout::AuthShell>
        }
    })
}


pub fn render_forgot_password_sent(lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.forgot_password_sent_title.to_string() lang=lang>
                <h1>{t.forgot_password_sent_title}</h1>
                <p class="muted">{t.forgot_password_sent_lede}</p>
                <p class="muted">{t.forgot_password_sent_lede2}</p>
                <p class="mt-4">
                    <a href="/admin/login" class="button">{t.back_to_login}</a>
                </p>
            </crate::layout::AuthShell>
        }
    })
}


pub fn render_reset_password(
    token: String,
    csrf_token: String,
    flash: Option<Flash>,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.reset_password_title.to_string() lang=lang>
                <h1>{t.reset_password_title}</h1>
                <p class="muted">{t.reset_password_lede}</p>
                {flash_banner(flash)}
                <form method="post" action="/reset-password" class="stack" autocomplete="off">
                    <input type="hidden" name="_csrf" value=csrf_token />
                    <input type="hidden" name="token" value=token />
                    <div class="field">
                        <label for="password" class="field__label">{t.reset_password_new_label}</label>
                        <input id="password" name="password" type="password"
                               required=true minlength="12"
                               autocomplete="new-password" autofocus=true />
                        <span class="field__hint">{t.reset_password_new_hint}</span>
                    </div>
                    <div class="field">
                        <label for="confirm_password" class="field__label">{t.reset_password_confirm_label}</label>
                        <input id="confirm_password" name="confirm_password" type="password"
                               required=true minlength="12"
                               autocomplete="new-password" />
                    </div>
                    <button type="submit">{t.reset_password_submit}</button>
                </form>
            </crate::layout::AuthShell>
        }
    })
}


pub fn render_reset_password_invalid(lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        view! {
            <crate::layout::AuthShell title=t.reset_password_invalid_title.to_string() lang=lang>
                <h1>{t.reset_password_invalid_title}</h1>
                <p class="muted">{t.reset_password_invalid_lede}</p>
                <p class="mt-4">
                    <a href="/forgot-password" class="button">{t.reset_password_invalid_request_again}</a>
                </p>
            </crate::layout::AuthShell>
        }
    })
}
