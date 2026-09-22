//! /me/security overview (RFC 065).

use super::*;
use crate::layout::Shell;

pub struct MeOverviewData {
    pub shell: MeShellData,
    pub totp_enabled: bool,
    pub passkey_count: usize,
    pub active_session_count: usize,
    pub recent_events: Vec<MeAuditEntry>,
    pub csrf_token: String,
    /// RFC 074: timestamp of the user's previous successful login.
    /// None = no prior login recorded (first login, or pre-migration row).
    pub last_login_at: Option<chrono::DateTime<chrono::Utc>>,
    /// RFC 103 5b: the user's own most recent recovery-link event, if any.
    pub recovery_event: Option<MeRecoveryEvent>,
}

/// The signed-in user's own most recent recovery-link event (RFC 103 5b):
/// when an administrator- or operator-issued link was created for them, or
/// when their password was last reset through a link of any origin. Never
/// carries a token, a reason, or a name — only what kind of actor and when.
#[derive(Debug, Clone, Copy)]
pub enum MeRecoveryEvent {
    IssuedByAdmin(chrono::DateTime<chrono::Utc>),
    IssuedByOperator(chrono::DateTime<chrono::Utc>),
    CompletedSelfService(chrono::DateTime<chrono::Utc>),
    CompletedByAdmin(chrono::DateTime<chrono::Utc>),
    CompletedByOperator(chrono::DateTime<chrono::Utc>),
}

pub fn render_me_overview(
    data: MeOverviewData,
    _is_dev: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let tabs = me_security_tabs(MeTab::Overview, lang);
        let last_login_at = data.last_login_at;
        let MeOverviewData {
            shell: _,
            totp_enabled,
            passkey_count,
            active_session_count,
            recent_events,
            recovery_event,
            ..
        } = data;
        let event_rows: Vec<_> = recent_events
            .iter()
            .map(|e| {
                let badge = match e.result.as_str() {
                    "ok" => view! { <span class="badge badge--ok">{"ok"}</span> }.into_any(),
                    "fail" | "denied" => {
                        view! { <span class="badge badge--danger">{e.result.clone()}</span> }
                            .into_any()
                    }
                    other => view! { <span class="badge">{other.to_string()}</span> }.into_any(),
                };
                view! {
                    <tr>
                        <td><time>{e.at.format("%Y/%m/%d %H:%M").to_string()}</time></td>
                        <td><code>{e.action.clone()}</code></td>
                        <td>{badge}</td>
                    </tr>
                }
            })
            .collect();
        // RFC 074: anti-phishing last-login line.
        let last_login_line = match last_login_at {
            Some(ts) => {
                let date = fmt_time(ts);
                let text = t.me_overview_last_login.replace("{date}", &date);
                view! { <p class="muted text-caption">{text}</p> }.into_any()
            }
            None => {
                view! { <p class="muted text-caption">{t.me_overview_first_login}</p> }.into_any()
            }
        };
        // RFC 103 5b: the account-page recovery-link line. `None` renders
        // nothing — a user with no such event sees no line at all.
        let recovery_event_line = recovery_event.map(|e| {
            let (template, at) = match e {
                MeRecoveryEvent::IssuedByAdmin(at) => (t.me_overview_recovery_issued_by_admin, at),
                MeRecoveryEvent::IssuedByOperator(at) => {
                    (t.me_overview_recovery_issued_by_operator, at)
                }
                MeRecoveryEvent::CompletedSelfService(at) => {
                    (t.me_overview_recovery_completed_self, at)
                }
                MeRecoveryEvent::CompletedByAdmin(at) => {
                    (t.me_overview_recovery_completed_by_admin, at)
                }
                MeRecoveryEvent::CompletedByOperator(at) => {
                    (t.me_overview_recovery_completed_by_operator, at)
                }
            };
            let text = template.replace("{date}", &fmt_time(at));
            view! { <p class="muted text-caption">{text}</p> }
        });
        view! {
            <Shell title=t.me_tab_overview.to_string() show_nav=true current=Some("me".to_string()) lang=lang csrf_token=data.csrf_token.clone()>
                <header class="page-header">
                    <h1 class="page-header__title">{t.me_tab_overview}</h1>
                    {last_login_line}
                    {recovery_event_line}
                </header>
                {tabs}
                <div class="stack mt-4">
                    <section class="card">
                        <h2 class="card__title">{t.me_overview_section_status}</h2>
                        <dl class="kv-list">
                            {kv_bool_badge(t, t.me_overview_label_mfa_totp, totp_enabled)}
                            {kv_row(t.me_overview_label_passkeys, passkey_count.to_string())}
                            {kv_row(t.me_security_sessions_section,
                                    active_session_count.to_string())}
                        </dl>
                    </section>
                    <section class="card">
                        <h2 class="card__title">{t.me_overview_section_activity}</h2>
                        {if event_rows.is_empty() {
                            view! { <p class="muted">{t.me_overview_no_recent_events}</p> }.into_any()
                        } else {
                            view! {
                                <div class="table-wrap">
                                    <table><tbody>{event_rows}</tbody></table>
                                </div>
                            }.into_any()
                        }}
                    </section>
                </div>
            </Shell>
        }
    })
}
