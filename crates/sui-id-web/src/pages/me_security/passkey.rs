//! /me/security passkey (RFC 065).

use super::super::auth::PasskeyDescriptor;
use super::super::common::*;
use super::*;
use crate::layout::Shell;

pub struct MePasskeyData {
    pub shell: MeShellData,
    pub passkeys: Vec<PasskeyDescriptor>,
    /// False = origin is plain HTTP on a non-localhost host → show warning.
    pub origin_eligible: bool,
    pub csrf_token: String,
}

pub fn render_me_passkey(
    data: MePasskeyData,
    flash: Option<Flash>,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Passkey, lang);
        let MePasskeyData {
            shell: _,
            passkeys,
            origin_eligible,
            csrf_token,
        } = data;
        let warning = (!origin_eligible).then(|| {
            view! {
                <div class="banner banner--warning" role="alert">
                    {t.me_passkey_origin_warning}
                </div>
            }
        });
        let rows: Vec<_> = passkeys.iter().map(|p| {
            let cred_id = p.id.clone();
            let nick = p.nickname.clone();
            let nick2 = p.nickname.clone();
            let csrf = csrf_token.clone();
            let delete_action = format!("/me/security/passkeys/{}/delete", cred_id);
            view! {
                <tr>
                    <td>
                        <strong>{nick}</strong>
                        <br/>
                        <span class="muted text-small">
                            {t.profile_passkeys_th_registered} ": " {p.created_at.format("%Y/%m/%d").to_string()}
                        </span>
                    </td>
                    <td>
                        <details>
                            <summary class="button secondary text-small">
                                {t.me_passkey_button_rename}
                            </summary>
                            <form method="post"
                                  action={format!("/me/security/passkeys/{cred_id}/rename")}
                                  class="mt-2">
                                <input type="hidden" name="_csrf" value=csrf.clone()/>
                                <div class="row">
                                    <input type="text" name="nickname"
                                           placeholder=t.me_passkey_nickname_placeholder
                                           required=true maxlength="64"
                                           class="flex-1"/>
                                    <button type="submit">{t.button_save}</button>
                                </div>
                            </form>
                        </details>
                    </td>
                    <td>
                        <form method="post" action=delete_action>
                            <input type="hidden" name="_csrf" value=csrf/>
                            <button type="submit" class="button danger"
                                    aria-label={format!("{} {nick2}", t.button_delete)}>
                                {t.button_delete}
                            </button>
                        </form>
                    </td>
                </tr>
            }
        }).collect();
        view! {
            <Shell title=t.me_passkey_section_title.to_string() show_nav=true current=Some("me".to_string()) lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header"><h1 class="page-header__title">{t.me_passkey_section_title}</h1></header>
                {tabs}
                {flash_banner(flash)}
                {warning}
                <div class="stack mt-4">
                    {if rows.is_empty() {
                        empty_state(EmptyStateData {
                            message: t.profile_passkeys_empty.into(),
                            hint: None,
                            action: None,
                            compact: false,
                        }).into_any()
                    } else {
                        view! {
                            <div class="table-wrap">
                                <table><tbody>{rows}</tbody></table>
                            </div>
                        }.into_any()
                    }}
                    {origin_eligible.then(|| view! {
                        <section class="card">
                            <h3 class="card__title">{t.profile_passkeys_register_section}</h3>
                            <form id="passkey-register-form" method="post"
                                  action="/me/security/passkeys/register/start" class="stack">
                                <input type="hidden" name="_csrf" value=csrf_token.clone()/>
                                <div class="field">
                                    <label for="pk-nickname" class="field__label">
                                        {t.profile_passkeys_nickname_label}
                                    </label>
                                    <input id="pk-nickname" name="nickname" type="text" required=true
                                           placeholder=t.me_passkey_nickname_placeholder
                                           maxlength="64" />
                                    <span class="field__hint">{t.profile_passkeys_nickname_hint}</span>
                                </div>
                                <div>
                                    <button type="submit">{t.profile_passkeys_register_button}</button>
                                </div>
                            </form>
                        </section>
                    })}
                    <script src="/static/webauthn.js"></script>
                </div>
            </Shell>
        }
    })
}
