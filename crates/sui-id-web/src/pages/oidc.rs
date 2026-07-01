//! Page renderers for the "oidc" screen domain (RFC 065).

use super::common::*;
use leptos::prelude::*;

pub struct ConsentData {
    pub client_name: String,
    pub requested_scopes: Vec<String>,
    pub csrf_token: String,
    /// RFC 008: optional application logo URL (validated HTTPS, never fetched).
    pub logo_uri: Option<String>,
    /// RFC 008: optional application home-page URL.
    pub homepage_uri: Option<String>,
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

        // RFC 008: application identity — logo and homepage link.
        let logo_uri = data.logo_uri.clone();
        let homepage_uri = data.homepage_uri.clone();
        let client_name_display = client_name.clone();

        let csrf = data.csrf_token.clone();
        view! {
            <crate::layout::AuthShell title=t.consent_title.to_string() lang=lang>
                // RFC-MI-070: .consent-card overrides the auth-card max-width to
                // give the scope list comfortable reading room (32rem vs 28rem).
                <div class="auth-card consent-card">
                    // RFC 008: show app logo if provided.
                    {logo_uri.map(|uri| view! {
                        <div class="consent-app-logo">
                            <img src=uri alt=client_name_display.clone()
                                 class="consent-app-logo__img" />
                            <p class="text-caption muted">{"App logo provided by the application"}</p>
                        </div>
                    })}
                    <h1>{t.consent_title}</h1>
                    <p class="consent-intro">
                        // RFC 008: wrap client name in a link to homepage if available.
                        {match homepage_uri {
                            Some(hp) => view! {
                                <><a href=hp target="_blank" rel="noopener noreferrer">
                                    <strong>{client_name}</strong>
                                </a>
                                " " {t.consent_app_wants_access}</>
                            }.into_any(),
                            None => view! {
                                <><strong>{client_name}</strong>
                                " " {t.consent_app_wants_access}</>
                            }.into_any(),
                        }}
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
