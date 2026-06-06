//! Page renderers for the "oidc" screen domain (RFC 065).

use leptos::prelude::*;
use super::common::*;

pub struct ConsentData {
    pub client_name: String,
    pub requested_scopes: Vec<String>,
    pub csrf_token: String,
}


pub fn render_consent(data: ConsentData, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let client_name = data.client_name.clone();
        let scope_labels: Vec<_> = data.requested_scopes.iter().map(|s| {
            let label: &'static str = match s.as_str() {
                "openid"         => t.consent_scope_openid,
                "profile"        => t.consent_scope_profile,
                "email"          => t.consent_scope_email,
                "offline_access" => t.consent_scope_offline_access,
                _                => "—",
            };
            let scope_str = s.clone();
            view! {
                <li style="margin:var(--space-1) 0">
                    <span class="badge">{scope_str}</span>
                    " — " {label}
                </li>
            }
        }).collect();

        let csrf = data.csrf_token.clone();
        view! {
            <crate::layout::AuthShell title=t.consent_title.to_string() lang=lang>
                <div class="auth-card" style="max-width:32rem">
                    <h1>{t.consent_title}</h1>
                    <p style="margin:var(--space-3) 0">
                        <strong>{client_name}</strong>
                        " " {t.consent_app_wants_access}
                    </p>
                    <ul style="list-style:none;padding:0;margin-bottom:var(--space-4)">
                        {scope_labels}
                    </ul>
                    <div class="row gap-2">
                        <form method="post" action="/oauth2/consent">
                            <input type="hidden" name="_csrf" value=csrf.clone() />
                            <input type="hidden" name="decision" value="approve" />
                            <button type="submit">{t.consent_approve}</button>
                        </form>
                        <form method="post" action="/oauth2/consent">
                            <input type="hidden" name="_csrf" value=csrf />
                            <input type="hidden" name="decision" value="deny" />
                            <button type="submit" class="secondary">{t.consent_deny}</button>
                        </form>
                    </div>
                </div>
            </crate::layout::AuthShell>
        }
    })
}
