//! /me/security mfa (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::super::common::*;
use super::*;  // MeShellData + MeTab + me_security_tabs

pub struct MeMfaData {
    pub shell: MeShellData,
    pub totp_enabled: bool,
    pub passkey_count: usize,
    pub recovery_codes_remaining: usize,
    /// Recovery codes shown once after enrollment or regeneration.
    /// Wrapped in an `<ol>` and prominent banner; this is the only
    /// chance to copy them since the server only stores hashes.
    pub fresh_recovery_codes: Option<Vec<String>>,
    pub csrf_token: String,
}


pub fn render_me_mfa(
    data: MeMfaData,
    flash: Option<Flash>,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Mfa, lang);
        let MeMfaData {
            shell: _,
            totp_enabled,
            passkey_count,
            recovery_codes_remaining,
            fresh_recovery_codes,
            csrf_token,
        } = data;
        // Recovery codes banner — shown once, immediately after enrollment
        // or regeneration. The server only keeps hashes; if the user
        // doesn't save these codes now, they're unrecoverable.
        let recovery_block = fresh_recovery_codes.map(|codes| {
            let lis: Vec<_> = codes
                .into_iter()
                .map(|c| view! { <li><span class="code">{c}</span></li> })
                .collect();
            view! {
                <div class="flash warn" role="status">
                    <div class="stack-tight">
                        <strong>{t.profile_recovery_save_now}</strong>
                        <p class="muted">{t.profile_recovery_save_lede}</p>
                        <ol>{lis}</ol>
                    </div>
                </div>
            }
        });
        let mfa_disable_onsubmit = format!(
            "return confirm('{}');",
            t.profile_mfa_disable_confirm.replace('\'', "\\'")
        );
        let csrf_for_enroll = csrf_token.clone();
        let csrf_for_disable = csrf_token.clone();
        let csrf_for_regen = csrf_token.clone();
        view! {
            <Shell title=t.me_tab_mfa.to_string() show_nav=true current=Some("me".to_string()) lang=lang>
                <header class="page-header">
                    <h1 class="page-header__title">{t.me_tab_mfa}</h1>
                </header>
                {tabs}
                {flash_banner(flash)}
                {recovery_block}
                <div class="stack mt-4">
                    // TOTP card
                    <section class="card">
                        <h2 class="card__title">{t.profile_mfa_totp_card_title}</h2>
                        <dl class="kv-list">
                            {kv_bool_badge(t, t.me_security_mfa_status_label, totp_enabled)}
                            {if totp_enabled {
                                view! {
                                    <div>
                                        {kv_row(t.me_security_mfa_recovery_section_label,
                                                (t.me_security_mfa_recovery_codes_remaining)(recovery_codes_remaining))}
                                    </div>
                                }.into_any()
                            } else { view! { <div/> }.into_any() }}
                        </dl>
                        <div class="row mt-3">
                            {if totp_enabled {
                                view! {
                                    <>
                                    <form method="post" action="/me/security/mfa/recovery-codes/regenerate"
                                          class="inline-el">
                                        <input type="hidden" name="_csrf" value=csrf_for_regen />
                                        <button type="submit" class="secondary">{t.profile_mfa_regenerate_codes}</button>
                                    </form>
                                    <form method="post" action="/me/security/mfa/disable"
                                          class="inline-el"
                                          onsubmit=mfa_disable_onsubmit>
                                        <input type="hidden" name="_csrf" value=csrf_for_disable />
                                        <button type="submit" class="danger">{t.profile_mfa_disable_button}</button>
                                    </form>
                                    </>
                                }.into_any()
                            } else {
                                view! {
                                    <form method="post" action="/me/security/mfa/enroll/start">
                                        <input type="hidden" name="_csrf" value=csrf_for_enroll />
                                        <button type="submit">{t.profile_mfa_enroll_button}</button>
                                    </form>
                                }.into_any()
                            }}
                        </div>
                    </section>
                    // Passkeys summary
                    <section class="card">
                        <h2 class="card__title">{t.me_passkey_section_title}</h2>
                        {kv_row(t.profile_passkeys_section, passkey_count.to_string())}
                        <p class="mt-3">
                            <a href="/me/security/passkeys" class="button secondary">{t.me_tab_passkey}</a>
                        </p>
                    </section>
                </div>
            </Shell>
        }
    })
}
