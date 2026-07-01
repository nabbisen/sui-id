//! `/me/apps` — self-service view of OAuth consent grants (RFC 072, v0.60.0).

use super::super::common::*;
use super::*;
use chrono::{DateTime, Utc};

/// Render data for one consent grant (resolved from `ConsentGrantView`).
pub struct AppGrantData {
    pub client_id: String,
    pub client_name: String,
    pub granted_scopes: Vec<String>,
    pub granted_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

pub struct MeAppsData {
    pub grants: Vec<AppGrantData>,
    pub csrf_token: String,
    pub dev_mode: bool,
}

pub fn render_me_apps(data: MeAppsData, flash: Option<Flash>, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let MeAppsData {
            grants,
            csrf_token,
            dev_mode,
        } = data;

        let grant_items: Vec<_> = grants
            .into_iter()
            .map(|g| {
                let client_id = g.client_id.clone();
                let client_name = g.client_name.clone();
                let granted_at = fmt_time(g.granted_at);
                let last_used = g
                    .last_used_at
                    .map(fmt_time)
                    .unwrap_or_else(|| t.me_apps_never_used.to_owned());
                let csrf_val = csrf_token.clone();
                let scope_list: Vec<_> = g
                    .granted_scopes
                    .iter()
                    .map(|s| {
                        let (label, desc): (&'static str, Option<&'static str>) = match s.as_str() {
                            "openid" => (t.consent_scope_openid, Some(t.consent_scope_openid_desc)),
                            "profile" => {
                                (t.consent_scope_profile, Some(t.consent_scope_profile_desc))
                            }
                            "email" => (t.consent_scope_email, Some(t.consent_scope_email_desc)),
                            "offline_access" => (
                                t.consent_scope_offline_access,
                                Some(t.consent_scope_offline_access_desc),
                            ),
                            _ => ("—", None),
                        };
                        let slug = s.clone();
                        view! {
                            <li class="consent-scope-item">
                                <span class="consent-scope-item__title">{label}</span>
                                {desc.map(|d| view! {
                                    <span class="consent-scope-item__desc">{d}</span>
                                })}
                                <code class="text-caption muted">{slug}</code>
                            </li>
                        }
                    })
                    .collect();

                view! {
                    <div class="card">
                        <div class="card__header row gap-2">
                            <div class="card__header-meta">
                                <strong>{client_name}</strong>
                                <p class="muted text-caption">
                                    {t.me_apps_granted_on}": "{granted_at}
                                    " · "
                                    {t.me_apps_last_used}": "{last_used}
                                </p>
                            </div>
                            <form method="post"
                                  action=format!("/me/apps/{}/revoke", client_id)
                                  class="form-actions">
                                <input type="hidden" name="_csrf" value=csrf_val />
                                <button type="submit" class="danger">
                                    {t.me_apps_revoke_button}
                                </button>
                            </form>
                        </div>
                        <ul class="consent-scope-list">
                            {scope_list}
                        </ul>
                    </div>
                }
            })
            .collect();

        view! {
            <crate::layout::Shell
                title=t.me_apps_title.to_owned()
                show_nav=true
                current=Some("me".to_string())
                lang=lang
                csrf_token=csrf_token.clone()
                dev_mode=dev_mode>
                {me_security_tabs(MeTab::Apps, lang)}
                <header class="page-header">
                    <h1 class="page-header__title">{t.me_apps_title}</h1>
                    <p class="page-header__lede">{t.me_apps_intro}</p>
                </header>
                {flash_banner(flash)}
                {if grant_items.is_empty() {
                    view! {
                        <div class="callout callout--info">
                            {t.me_apps_empty}
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="stack">
                            {grant_items}
                        </div>
                    }.into_any()
                }}
            </crate::layout::Shell>
        }
    })
}
