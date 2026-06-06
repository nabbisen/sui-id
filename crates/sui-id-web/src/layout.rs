//! Common HTML shell shared by every page.
//!
//! Composes the design tokens (`tokens.rs`) and component styles
//! (`components.rs`) and adds:
//!
//! - External scripts (`/static/theme-init.js`, `/static/copy.js`,
//!   `/static/logout-csrf.js`) loaded with `defer`. The theme init
//!   script resolves the user's theme choice from `localStorage`
//!   *as soon as the DOM is parsed* and sets `data-theme` on
//!   `<html>` before the visible body is painted, avoiding a
//!   flash of unthemed content (FOUT). Earlier versions used inline
//!   `<script>` blocks for this; v0.48.1 moved them to external
//!   files because CSP `script-src 'self'` blocks inline JS.
//! - The footer with the theme toggle (light / dark / auto) and the
//!   accessibility badges.
//! - The admin nav (when `show_nav` is true).

use crate::components::COMPONENTS_CSS;
use crate::tokens::TOKENS_CSS;
use leptos::prelude::*;

/// Wrap a page body in the standard sui-id chrome.
///
/// `lang` controls the `<html lang="…">` attribute and propagates
/// down to `Nav`, `Footer`, and `ThemeToggle` so every label in the
/// admin chrome reads in the user's locale (RFC 050).
#[component]
pub fn Shell(
    title: String,
    show_nav: bool,
    current: Option<String>,
    lang: sui_id_i18n::Locale,
    /// When true, renders the DEV MODE banner (RFC 032).
    #[prop(optional)] dev_mode: Option<bool>,
    children: Children,
) -> impl IntoView {
    let stylesheet = format!("{}\n{}", TOKENS_CSS, COMPONENTS_CSS);
    let lang_tag = lang.tag();
    let dir_attr = lang.direction();
    view! {
        <html lang=lang_tag dir=dir_attr>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="referrer" content="same-origin" />
                <title>{format!("{title} · sui-id")}</title>
                <style>{stylesheet}</style>
                <script src="/static/theme-init.js" defer></script>
                <script src="/static/copy.js" defer></script>
            </head>
            <body>
                {dev_mode.unwrap_or(false).then(|| view! {
                    <div class="dev-banner" role="alert">
                        <strong>"DEV MODE"</strong>
                        " — not for production. cookie_secure=false, HIBP off, lockout disabled."
                    </div>
                })}
                <header class="app-header">
                    <h1 class="app-header__brand">"sui-id"</h1>
                    {show_nav.then(|| view! {
                        <Nav current=current.clone() lang=lang csrf_token="".to_string() />
                    })}
                </header>
                <main class="app-main">{children()}</main>
                <Footer lang=lang />
            </body>
        </html>
    }
}

/// Centred narrow layout for login / setup. Same chrome but the
/// main column is narrowed and vertically centred — produces the
/// "card on a quiet field" look the design memo asks for on
/// auth-style screens.
#[component]
pub fn AuthShell(
    title: String,
    lang: sui_id_i18n::Locale,
    children: Children,
) -> impl IntoView {
    let stylesheet = format!("{}\n{}", TOKENS_CSS, COMPONENTS_CSS);
    let lang_tag = lang.tag();
    let dir_attr = lang.direction();
    view! {
        <html lang=lang_tag dir=dir_attr>
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="referrer" content="same-origin" />
                <title>{format!("{title} · sui-id")}</title>
                <style>{stylesheet}</style>
                <script src="/static/theme-init.js" defer></script>
            </head>
            <body>
                <header class="app-header">
                    <h1 class="app-header__brand">"sui-id"</h1>
                </header>
                <main class="app-main app-main--narrow auth-page">
                    <div class="auth-card">{children()}</div>
                </main>
                <Footer lang=lang />
            </body>
        </html>
    }
}

#[component]
fn Nav(current: Option<String>, lang: sui_id_i18n::Locale, csrf_token: String) -> impl IntoView {
    let t = lang.strings();
    let items: [(&'static str, &'static str, &'static str); 7] = [
        ("dashboard",    t.nav_dashboard,    "/admin"),
        ("users",        t.nav_users,        "/admin/users"),
        ("clients",      t.nav_clients,      "/admin/clients"),
        ("signing-keys", t.nav_signing_keys, "/admin/signing-keys"),
        ("audit",        t.nav_audit,        "/admin/audit"),
        ("settings",     t.nav_settings,     "/admin/settings"),
        // RFC 055 (v0.44.0): "Profile" → "Security", pointing to the
        // consolidated tabbed /me/security/* surface. The current-tab
        // key is "me" to match the highlight Shell/`current=` already
        // uses across the tabbed render_me_* views.
        ("me",           t.nav_security,     "/me/security/overview"),
    ];
    // The CSRF token for the logout form. If none was passed in by the
    // handler (the page was rendered without the cookie), fall back to
    // reading the cookie via JS on the client side.
    let token_value = if csrf_token.is_empty() { "".into() } else { csrf_token };
    view! {
        <nav class="app-nav" aria-label=t.nav_aria_main>
            {items.into_iter().map(|(key, label, href)| {
                let aria = if current.as_deref() == Some(key) { Some("page") } else { None };
                view! {
                    <a class="app-nav__link" href=href aria-current=aria>{label}</a>
                }
            }).collect::<Vec<_>>()}
            // Sign out uses POST + CSRF to prevent logout-CSRF attacks.
            // The CSRF token is read from the sui_id_csrf cookie (not HttpOnly)
            // and populated by the inline script below if not server-rendered.
            <form method="post" action="/admin/logout" class="app-nav__signout-form"
                  id="logout-form">
                <input type="hidden" name="_csrf" id="logout-csrf" value=token_value />
                <button type="submit" class="app-nav__link app-nav__signout"
                        aria-label=t.nav_aria_signout>
                    {t.nav_logout}
                </button>
            </form>
            // CSP-safe replacement (v0.48.1) for the previously
            // inline injector. Populates the hidden #logout-csrf
            // input from the `sui_id_csrf` cookie before submit.
            <script src="/static/logout-csrf.js" defer></script>
        </nav>
    }
}

#[component]
fn Footer(lang: sui_id_i18n::Locale) -> impl IntoView {
    let t = lang.strings();
    view! {
        <footer class="app-footer" role="contentinfo">
            <span class="app-footer__tagline">
                {t.footer_tagline}
            </span>
            <span class="app-footer__a11y" aria-label=t.footer_a11y_group_label>
                <span title=t.a11y_keyboard>"⌨ " {t.a11y_keyboard}</span>
                <span title=t.a11y_screen_reader>"⊙ " {t.a11y_screen_reader}</span>
                <span title=t.a11y_contrast>"◐ " {t.a11y_contrast}</span>
            </span>
            <ThemeToggle lang=lang />
            <span class="app-footer__version">{format!("v{}", env!("CARGO_PKG_VERSION"))}</span>
        </footer>
    }
}

#[component]
fn ThemeToggle(lang: sui_id_i18n::Locale) -> impl IntoView {
    let t = lang.strings();
    // Click handlers and initial aria-pressed are attached by
    // `/static/theme-init.js` on DOM ready, keyed off the
    // `data-theme-value` attribute on each button.
    //
    // Prior to v0.48.1 these were inline `onclick=` attributes;
    // CSP `script-src 'self'` blocks inline event handlers
    // (`script-src-attr`), so the toggle silently did nothing on
    // production builds.
    view! {
        <div class="theme-toggle" role="group" aria-label=t.theme_toggle_group>
            <button class="theme-toggle__btn"
                    type="button"
                    data-theme-value="light"
                    aria-pressed="false"
                    title=t.theme_toggle_light_title>
                "☀ " {t.theme_toggle_light}
            </button>
            <button class="theme-toggle__btn"
                    type="button"
                    data-theme-value="system"
                    aria-pressed="false"
                    title=t.theme_toggle_auto_title>
                "🖥 " {t.theme_toggle_auto}
            </button>
            <button class="theme-toggle__btn"
                    type="button"
                    data-theme-value="dark"
                    aria-pressed="false"
                    title=t.theme_toggle_dark_title>
                "☾ " {t.theme_toggle_dark}
            </button>
        </div>
    }
}
