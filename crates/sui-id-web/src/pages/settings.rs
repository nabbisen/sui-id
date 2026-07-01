//! Settings sub-screens (RFC 065 sub-split).

use super::common::*;
use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Basic,
    Security,
    Authentication,
    Logs,
    Email,
    Other,
}

impl SettingsTab {
    fn key(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Security => "security",
            Self::Authentication => "authentication",
            Self::Logs => "logs",
            Self::Email => "email",
            Self::Other => "other",
        }
    }
}

fn settings_tabs(active: SettingsTab, lang: sui_id_i18n::Locale) -> impl IntoView {
    let t = lang.strings();
    // RFC 074: Basic → "General", Other → "Advanced" (label-only rename;
    // routes and underlying pages unchanged). Full 6→4 group consolidation
    // is deferred to a future RFC (requires handler merging).
    let items = [
        (
            SettingsTab::Basic,
            t.settings_tab_general,
            "/admin/settings/basic",
        ),
        (
            SettingsTab::Security,
            t.settings_tab_security,
            "/admin/settings/security",
        ),
        (
            SettingsTab::Authentication,
            t.settings_tab_authentication,
            "/admin/settings/authentication",
        ),
        (
            SettingsTab::Logs,
            t.settings_tab_logs,
            "/admin/settings/logs",
        ),
        (
            SettingsTab::Email,
            t.settings_tab_email,
            "/admin/settings/email",
        ),
        (
            SettingsTab::Other,
            t.settings_tab_advanced,
            "/admin/settings/other",
        ),
    ];
    let active_key = active.key();
    let links: Vec<_> = items
        .iter()
        .map(|(tab, label, href)| {
            let aria = if tab.key() == active_key {
                Some("page")
            } else {
                None
            };
            view! {
                <a class="route-tabs__link" href=*href aria-current=aria>{*label}</a>
            }
        })
        .collect();
    // RFC-MI-022 (v0.51.1): migrated to .route-tabs markup.
    // Previously used <nav class="app-nav" style="…"> with an inline
    // style for margin-bottom; the .route-tabs class now carries
    // margin-bottom:var(--space-4) in the CSS.
    view! {
        <nav class="route-tabs" aria-label=t.settings_tabs_aria>
            {links}
        </nav>
    }
}

mod authentication;
/// Two-column key/value table used inside each settings card. Keeps
/// per-tab content boring and consistent.
mod basic;
mod email;
mod logs;
mod other;
mod security;

pub use authentication::*;
pub use basic::*;
pub use email::*;
pub use logs::*;
pub use other::*;
pub use security::*;
