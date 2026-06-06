//! /me/security security (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::super::common::*;
use super::*;  // MeShellData + MeTab + me_security_tabs

pub struct MeSecurityData {
    pub username: String,
    pub is_admin: bool,
    /// Whether the user has TOTP enrolled.
    pub totp_enabled: bool,
    /// Number of active WebAuthn passkeys.
    pub passkey_count: usize,
    /// Identifier of the session that issued the current request.
    /// Used to mark "this is you" in the session list and to keep it
    /// alive when the user clicks "sign out everywhere else".
    pub current_session_id: String,
    pub sessions: Vec<MeSessionDescriptor>,
    pub recent_events: Vec<MeAuditEntry>,
}


pub struct MeSessionDescriptor {
    pub id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Comma-separated human display: "password", "password + TOTP", etc.
    pub auth_methods: String,
    pub is_current: bool,
}


pub struct MeAuditEntry {
    pub at: chrono::DateTime<chrono::Utc>,
    pub action: String,
    pub result: String,
    pub note: Option<String>,
}


pub fn render_me_security(
    data: MeSecurityData,
    flash: Option<Flash>,
    csrf_token: String,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let MeSecurityData {
            username,
            is_admin,
            totp_enabled,
            passkey_count,
            current_session_id,
            sessions,
            recent_events,
        } = data;

        let csrf_for_revoke_others = csrf_token.clone();
        let revoke_label = t.me_security_sessions_revoke;
        let revoke_confirm = t.me_security_sessions_revoke_confirm.to_owned();
        let current_badge = t.me_security_sessions_current_badge;

        // Session table rows. Each non-current row gets its own
        // mini-form so a user can revoke that specific entry.
        let session_rows: Vec<_> = sessions
            .into_iter()
            .map(|s| {
                let MeSessionDescriptor {
                    id,
                    created_at,
                    expires_at,
                    auth_methods,
                    is_current,
                } = s;
                let when = created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                let until = expires_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                let action_cell = if is_current {
                    view! {
                        <td>
                            <span class="badge badge--accent">{current_badge}</span>
                        </td>
                    }
                    .into_any()
                } else {
                    let csrf_for_row = csrf_token.clone();
                    let post_url = format!("/me/security/sessions/{id}/revoke");
                    let onsubmit_attr = format!("return confirm('{}');", revoke_confirm.replace('\'', "\\'"));
                    view! {
                        <td>
                            <form method="post" action=post_url class="inline-el"
                                  onsubmit=onsubmit_attr>
                                <input type="hidden" name="_csrf" value=csrf_for_row />
                                <button type="submit" class="secondary">{revoke_label}</button>
                            </form>
                        </td>
                    }
                    .into_any()
                };
                view! {
                    <tr>
                        <td class="muted">{when}</td>
                        <td class="muted">{until}</td>
                        <td>{auth_methods}</td>
                        {action_cell}
                    </tr>
                }
            })
            .collect();

        // Activity timeline.
        let event_rows: Vec<_> = recent_events
            .into_iter()
            .map(|e| {
                let MeAuditEntry {
                    at,
                    action,
                    result,
                    note,
                } = e;
                let when = at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                let note_str = note.unwrap_or_default();
                let result_badge = match result.as_str() {
                    "ok" => view! { <span class="badge badge--ok">"ok"</span> }.into_any(),
                    "fail" | "error" | "denied" => {
                        view! { <span class="badge badge--danger">{result.clone()}</span> }
                            .into_any()
                    }
                    _ => view! { <span class="badge">{result.clone()}</span> }.into_any(),
                };
                view! {
                    <tr>
                        <td class="muted">{when}</td>
                        <td><span class="code">{action}</span></td>
                        <td>{result_badge}</td>
                        <td class="muted">{note_str}</td>
                    </tr>
                }
            })
            .collect();

        let admin_link = is_admin.then(|| {
            view! {
                <p class="muted">
                    <a href="/admin">{t.me_security_admin_link}</a>
                </p>
            }
        });

        let mfa_summary = if totp_enabled || passkey_count > 0 {
            let parts = {
                let mut v = Vec::<String>::new();
                if totp_enabled {
                    v.push(t.me_security_mfa_factor_totp.to_owned());
                }
                if passkey_count > 0 {
                    v.push(
                        {t.me_security_mfa_factor_passkey_n}
                            .replace("{n}", &passkey_count.to_string()),
                    );
                }
                v.join(" / ")
            };
            view! {
                <p>
                    {t.me_security_mfa_status_label}
                    <span class="badge badge--ok ml-1">{t.me_security_mfa_status_enabled}</span>
                    <span class="muted ml-2">{parts}</span>
                </p>
            }
            .into_any()
        } else {
            view! {
                <div class="flash warn" role="status">
                    <div class="stack-tight">
                        <strong>{t.me_security_mfa_disabled_title}</strong>
                        <p class="muted mb-0">{t.me_security_mfa_disabled_lede}</p>
                    </div>
                </div>
            }
            .into_any()
        };

        let revoke_all_others_onsubmit = format!(
            "return confirm('{}');",
            t.me_security_sessions_revoke_all_others_confirm.replace('\'', "\\'")
        );

        view! {
            <Shell title=t.me_security_title.to_owned() show_nav=false current=None lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.me_security_title}</h1>
                        <p class="page-header__lede">
                            <strong>{username}</strong>{t.me_security_signed_in_as_suffix}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {admin_link}

                <section>
                    <h2>{t.me_security_mfa_section}</h2>
                    <div class="card">
                        {mfa_summary}
                        <div class="card__footer">
                            <a href="/admin/profile" class="button secondary">
                                {t.me_security_mfa_manage}
                            </a>
                            <a href="/me/security/password" class="button secondary">
                                {t.me_security_password_change_link}
                            </a>
                        </div>
                    </div>
                </section>

                <section>
                    <h2>{t.me_security_sessions_section}</h2>
                    <p class="muted">{t.me_security_sessions_lede}</p>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.me_security_sessions_th_started}</th>
                                    <th>{t.me_security_sessions_th_expires}</th>
                                    <th>{t.me_security_sessions_th_factors}</th>
                                    <th></th>
                                </tr>
                            </thead>
                            <tbody>{session_rows}</tbody>
                        </table>
                    </div>
                    <form method="post" action="/me/security/sessions/revoke-all-others"
                          class="mt-3"
                          onsubmit=revoke_all_others_onsubmit>
                        <input type="hidden" name="_csrf" value=csrf_for_revoke_others />
                        <input type="hidden" name="current_session" value=current_session_id />
                        <button type="submit" class="secondary">
                            {t.me_security_sessions_revoke_all_others}
                        </button>
                    </form>
                </section>

                <section>
                    <h2>{t.me_security_activity_section}</h2>
                    <p class="muted">{t.me_security_activity_lede}</p>
                    <div class="table-wrap">
                        <table>
                            <thead>
                                <tr>
                                    <th>{t.me_security_activity_th_when}</th>
                                    <th>{t.me_security_activity_th_event}</th>
                                    <th>{t.me_security_activity_th_result}</th>
                                    <th>{t.me_security_activity_th_note}</th>
                                </tr>
                            </thead>
                            <tbody>{event_rows}</tbody>
                        </table>
                    </div>
                </section>
            </Shell>
        }
    })
}
