//! Page renderers for the "oidc" screen domain (RFC 065).

use super::common::*;
use leptos::prelude::*;

pub struct ConsentData {
    pub client_name: String,
    pub requested_scopes: Vec<String>,
    pub csrf_token: String,
}

pub fn render_consent(data: ConsentData, lang: sui_id_i18n::Locale) -> String {
    render(move || {
        let t = lang.strings();
        let client_name = data.client_name.clone();
        // RFC-MI-070: each scope item renders as a .consent-scope-item block
        // with a bold short label and a muted description sentence.
        // The raw scope slug is kept as a <code> tag for developer context.
        // Unmapped scopes fall back to the raw slug with no description.
        let scope_items: Vec<_> = data
            .requested_scopes
            .iter()
            .map(|s| {
                let (label, desc): (&'static str, Option<&'static str>) = match s.as_str() {
                    "openid" => (t.consent_scope_openid, Some(t.consent_scope_openid_desc)),
                    "profile" => (t.consent_scope_profile, Some(t.consent_scope_profile_desc)),
                    "email" => (t.consent_scope_email, Some(t.consent_scope_email_desc)),
                    "offline_access" => (
                        t.consent_scope_offline_access,
                        Some(t.consent_scope_offline_access_desc),
                    ),
                    _ => ("—", None),
                };
                let scope_str = s.clone();
                view! {
                    <li class="consent-scope-item">
                        <span class="consent-scope-item__title">{label}</span>
                        {desc.map(|d| view! {
                            <span class="consent-scope-item__desc">{d}</span>
                        })}
                        <code class="text-caption muted">{scope_str}</code>
                    </li>
                }
            })
            .collect();

        let csrf = data.csrf_token.clone();
        view! {
            <crate::layout::AuthShell title=t.consent_title.to_string() lang=lang>
                // RFC-MI-070: .consent-card overrides the auth-card max-width to
                // give the scope list comfortable reading room (32rem vs 28rem).
                // The four inline styles present before this RFC are eliminated.
                <div class="auth-card consent-card">
                    <h1>{t.consent_title}</h1>
                    <p class="consent-intro">
                        <strong>{client_name}</strong>
                        " " {t.consent_app_wants_access}
                    </p>
                    <ul class="consent-scope-list">
                        {scope_items}
                    </ul>
                    // RFC: deny must not be a small text link — both Approve and
                    // Deny are POST forms with equal-weight buttons, so the user
                    // can make an informed choice using the keyboard.
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
