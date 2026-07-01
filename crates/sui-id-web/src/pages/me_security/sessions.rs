//! /me/security sessions (RFC 065).

use super::super::common::*;
use super::*;
use crate::layout::Shell;

pub struct MeSessionsData {
    pub shell: MeShellData,
    pub current_session_id: String,
    pub sessions: Vec<MeSessionDescriptor>,
    pub csrf_token: String,
}

pub fn render_me_sessions(
    data: MeSessionsData,
    flash: Option<Flash>,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Sessions, lang);
        let MeSessionsData {
            shell: _,
            current_session_id: _,
            sessions,
            csrf_token,
        } = data;
        let revoke_label = t.me_security_sessions_revoke;
        let revoke_confirm = t.me_security_sessions_revoke_confirm.to_owned();
        let current_badge = t.me_security_sessions_current_badge;
        let csrf2 = csrf_token.clone();

        let session_rows: Vec<_> = sessions.into_iter().map(|s| {
            let sid = s.id.clone();
            let created = s.created_at.format("%Y/%m/%d %H:%M").to_string();
            let expires = s.expires_at.format("%Y/%m/%d %H:%M").to_string();
            let methods = s.auth_methods.clone();
            let is_current = s.is_current;
            let csrf_row = csrf_token.clone();
            let revoke_action = format!("/me/security/sessions/{sid}/revoke");
            let confirm_js = revoke_confirm.clone();
            let action_cell = if is_current {
                view! {
                    <td><span class="badge badge--ok">{current_badge}</span></td>
                }.into_any()
            } else {
                view! {
                    <td>
                        <form method="post" action=revoke_action
                              onsubmit=format!("return confirm('{confirm_js}')")>
                            <input type="hidden" name="_csrf" value=csrf_row/>
                            <button type="submit" class="secondary danger-text">{revoke_label}</button>
                        </form>
                    </td>
                }.into_any()
            };
            view! {
                <tr>
                    <td><time>{created}</time></td>
                    <td><time>{expires}</time></td>
                    <td>{methods}</td>
                    {action_cell}
                </tr>
            }
        }).collect();

        let revoke_all_confirm = t.me_security_sessions_revoke_all_others_confirm;
        view! {
            <Shell title=t.me_security_sessions_section.to_string() show_nav=true current=Some("me".to_string()) lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <h1 class="page-header__title">{t.me_security_sessions_section}</h1>
                </header>
                {tabs}
                {flash_banner(flash)}
                <div class="stack mt-4">
                    <section class="card">
                        <p class="muted">{t.me_security_sessions_lede}</p>
                        {if session_rows.is_empty() {
                            view! { <p class="muted">{t.me_security_sessions_lede}</p> }.into_any()
                        } else {
                            view! {
                                <div class="table-wrap">
                                    <table>
                                        <thead>
                                            <tr>
                                                <th>{t.me_security_sessions_th_started}</th>
                                                <th>{t.me_security_sessions_th_expires}</th>
                                                <th>{t.me_security_sessions_th_factors}</th>
                                                <th/>
                                            </tr>
                                        </thead>
                                        <tbody>{session_rows}</tbody>
                                    </table>
                                </div>
                            }.into_any()
                        }}
                        <div class="mt-3">
                            <form method="post" action="/me/security/sessions/revoke-all-others"
                                  onsubmit=format!("return confirm('{revoke_all_confirm}')")>
                                <input type="hidden" name="_csrf" value=csrf2/>
                                <button type="submit" class="secondary">
                                    {t.me_security_sessions_revoke_all_others}
                                </button>
                            </form>
                        </div>
                    </section>
                </div>
            </Shell>
        }
    })
}
