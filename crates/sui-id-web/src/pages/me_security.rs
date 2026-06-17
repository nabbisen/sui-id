//! Self-service security tabs (RFC 065 sub-split).

use leptos::prelude::*;
use super::common::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeTab {
    Overview,
    Password,
    Mfa,
    Passkey,
    Sessions,
    Apps,       // RFC 072
    Language,
}

impl MeTab {
    /// Returns the URL path slug that identifies this tab (matches the
    /// route segment under `/me/security/` or `/me/`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Overview  => "overview",
            Self::Password  => "password",
            Self::Mfa       => "mfa",
            Self::Passkey   => "passkeys",
            Self::Sessions  => "sessions",
            Self::Apps      => "apps",
            Self::Language  => "language",
        }
    }
}


pub struct MeShellData {
    pub username: String,
    pub is_admin: bool,
    pub active_tab: MeTab,
}

/// Static tab definitions for `/me/security/*` and `/me/apps`.
/// Order matches the visual left-to-right order.
/// Labels are resolved at render time via the `lang` argument.
static ME_SECURITY_TABS_KEYS: &[(&str, &str)] = &[
    ("overview",  "/me/security/overview"),
    ("password",  "/me/security/password"),
    ("mfa",       "/me/security/mfa"),
    ("passkeys",  "/me/security/passkeys"),
    ("sessions",  "/me/security/sessions"),
    ("apps",      "/me/apps"),           // RFC 072
    ("language",  "/me/security/language"),
];

/// Render the `/me/security/*` route-based tab strip.
///
/// `active` identifies the current tab; `aria-current="page"` is
/// applied to the matching link.
pub fn me_security_tabs(active: MeTab, lang: sui_id_i18n::Locale) -> impl IntoView {
    let t = lang.strings();
    // Labels resolved at render time (locale-aware).
    let labels: &[&str] = &[
        t.me_tab_overview,
        t.me_tab_password,
        t.me_tab_mfa,
        t.me_tab_passkey,
        t.me_tab_sessions,
        t.me_tab_apps,       // RFC 072
        t.me_tab_language,
    ];
    // Build RouteTab entries inline — static keys/hrefs, runtime labels.
    let items: Vec<_> = ME_SECURITY_TABS_KEYS
        .iter()
        .zip(labels.iter())
        .map(|((key, href), label)| {
            // SAFETY: we produce `impl IntoView` directly; no &'static needed
            // for label since route_tabs takes &str.
            view! {
                <a class="route-tabs__link"
                   href=*href
                   aria-current={if *key == active.key() { Some("page") } else { None }}>
                    {*label}
                </a>
            }
        })
        .collect();
    view! {
        <nav class="route-tabs" aria-label=t.me_security_tabs_aria>
            {items}
        </nav>
    }
}


mod overview;
mod mfa;
mod sessions;
mod passkey;
mod language;
mod security;
pub mod apps;    // RFC 072: pub so sui-id handler can access AppGrantData/MeAppsData

pub use overview::*;
pub use mfa::*;
pub use sessions::*;
pub use passkey::*;
pub use language::*;
pub use security::*;
pub use apps::*;
