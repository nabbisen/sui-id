//! Common HTML shell shared by every page.
//!
//! Composes the design tokens (`tokens.rs`) and component styles
//! (`components.rs`) and adds:
//!
//! - External scripts (`/static/theme-init.js`, `/static/copy.js`)
//!   loaded with `defer`. The theme init script resolves the user's
//!   theme choice from `localStorage` *as soon as the DOM is parsed*
//!   and sets `data-theme` on `<html>` before the visible body is
//!   painted, avoiding a flash of unthemed content (FOUT). Earlier
//!   versions used inline `<script>` blocks for this; v0.48.1 moved
//!   them to external files because CSP `script-src 'self'` blocks
//!   inline JS.
//!
//!   `/static/logout-csrf.js` was a third external script (added in
//!   v0.48.1) that read the `sui_id_csrf` cookie and populated the
//!   sign-out form's hidden input at submit time. RFC-MI-021
//!   (v0.51.0) server-renders the token into the form directly and
//!   removes both the script and its asset; the file no longer
//!   ships.
//! - The footer with the theme toggle (light / dark / auto) and the
//!   accessibility badges.
//! - The admin nav, with its account menu, for [`Chrome::Console`].

use crate::components::components_css;
use crate::tokens::TOKENS_CSS;
use leptos::prelude::*;

/// RFC 144, *Refresh*: the navigation **is** the signed-in person's
/// name, or there is no navigation — there is no third state where a
/// page shows the console chrome to someone the type system cannot
/// name. Replaces the former `show_nav: bool` + `admin_username:
/// Option<String>` pair on [`Shell`], which let 38 of 39 pages fall
/// through to a bare *Sign out* with no account menu (measured
/// 2026-11-15; see RFC 144's refresh and N2a's review, F5).
#[derive(Debug, Clone)]
pub enum Chrome {
    /// The console navigation, with the account menu for `username`.
    Console { username: String },
    /// No navigation: sign-in, setup, error pages.
    Bare,
}

/// Wrap a page body in the standard sui-id chrome.
///
/// `lang` controls the `<html lang="…">` attribute and propagates
/// down to `Nav`, `Footer`, and `ThemeToggle` so every label in the
/// admin chrome reads in the user's locale (RFC 050).
#[component]
pub fn Shell(
    title: String,
    nav: Chrome,
    current: Option<String>,
    lang: sui_id_i18n::Locale,
    /// CSRF token for the Shell-internal sign-out form (RFC-MI-021,
    /// v0.51.0). Required even for `Chrome::Bare`, so handlers have a
    /// uniform API; in that case the token is not rendered to any
    /// form. Must be obtained from the request's session (e.g. via
    /// `crate::csrf::extract_token`) — passing an empty string is a
    /// contract violation.
    csrf_token: String,
    /// When true, renders the DEV MODE banner (RFC 032).
    #[prop(optional)]
    dev_mode: Option<bool>,
    children: Children,
) -> impl IntoView {
    let stylesheet = format!("{}\n{}", TOKENS_CSS, components_css());
    let t = lang.strings();
    let lang_tag = lang.tag();
    let dir_attr = lang.direction();
    view! {
        <html lang=lang_tag dir=dir_attr class="no-js">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="referrer" content="same-origin" />
                <title>{format!("{title} · sui-id")}</title>
                <style>{stylesheet}</style>
                <script src="/static/theme-init.js"></script>
                <script src="/static/copy.js" defer></script>
            </head>
            <body>
                <a href="#main-content" class="skip-link">
                    {t.a11y_skip_to_main}
                </a>
                {dev_mode.unwrap_or(false).then(|| view! {
                    <div class="dev-banner" role="alert">
                        <strong>"DEV MODE"</strong>
                        " — not for production. cookie_secure=false, HIBP off."
                    </div>
                })}
                <header class="app-header" role="banner">
                    <h1 class="app-header__brand">"sui-id"</h1>
                    {match nav {
                        Chrome::Console { username } => Some(view! {
                            <Nav current=current.clone() lang=lang
                                 csrf_token=csrf_token.clone()
                                 username=username />
                        }),
                        Chrome::Bare => None,
                    }}</header>
                <main class="app-main" id="main-content">{children()}</main>
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
pub fn AuthShell(title: String, lang: sui_id_i18n::Locale, children: Children) -> impl IntoView {
    let stylesheet = format!("{}\n{}", TOKENS_CSS, components_css());
    let t = lang.strings();
    let lang_tag = lang.tag();
    let dir_attr = lang.direction();
    view! {
        <html lang=lang_tag dir=dir_attr class="no-js">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="referrer" content="same-origin" />
                <title>{format!("{title} · sui-id")}</title>
                <style>{stylesheet}</style>
                <script src="/static/theme-init.js"></script>
            </head>
            <body>
                <a href="#main-content" class="skip-link">
                    {t.a11y_skip_to_main}
                </a>
                <header class="app-header" role="banner">
                    <h1 class="app-header__brand">"sui-id"</h1>
                </header>
                <main class="app-main app-main--narrow auth-page" id="main-content">
                    <div class="auth-card">{children()}</div>
                </main>
                <Footer lang=lang />
            </body>
        </html>
    }
}

#[component]
fn Nav(
    current: Option<String>,
    lang: sui_id_i18n::Locale,
    csrf_token: String,
    // RFC 144, Refresh: always present — Chrome::Console is the only
    // variant that renders a Nav at all (see Shell). The account menu
    // is on every page that shows the navigation, with no fallback.
    username: String,
) -> impl IntoView {
    let t = lang.strings();
    // RFC 074: "clients" renamed to "apps" in nav label (route unchanged).
    // "me" (Security link) removed — replaced by user-menu dropdown.
    let items: [(&'static str, &'static str, &'static str); 7] = [
        ("dashboard", t.nav_dashboard, "/admin"),
        ("users", t.nav_users, "/admin/users"),
        ("clients", t.nav_apps, "/admin/clients"), // label: Apps
        ("signing-keys", t.nav_signing_keys, "/admin/signing-keys"),
        (
            "federation-providers",
            t.nav_federation_providers,
            "/admin/federation-providers",
        ),
        ("audit", t.nav_audit, "/admin/audit"),
        ("settings", t.nav_settings, "/admin/settings"),
    ];
    view! {
        <nav class="app-nav" aria-label=t.nav_aria_main>
            // RFC 144 N2b's walk, 125% browser font size: this inner
            // wrapper is what scrolls when the links don't fit, so the
            // account menu just below — outside it — stays visible
            // and never slides off the edge with them.
            <div class="app-nav__links">
                {items.into_iter().map(|(key, label, href)| {
                    let aria = if current.as_deref() == Some(key) { Some("page") } else { None };
                    view! {
                        <a class="app-nav__link" href=href aria-current=aria>{label}</a>
                    }
                }).collect::<Vec<_>>()}
            </div>

            // RFC 074: user-menu dropdown — replaces the old flat
            // "Security" link and "Sign out" button. Pure HTML
            // (<details>/<summary>); no JavaScript required.
            <details class="user-menu">
                <summary class="app-nav__link user-menu__toggle">
                    {username} " ▾"
                </summary>
                <div class="user-menu__panel" role="menu">
                    <a class="user-menu__item" href="/me/security/overview"
                       role="menuitem">
                        {t.nav_my_account}
                    </a>
                    <form method="post" action="/admin/logout"
                          class="user-menu__form">
                        <input type="hidden" name="_csrf" value=csrf_token.clone() />
                        <button type="submit" class="user-menu__item"
                                role="menuitem">
                            {t.nav_logout}
                        </button>
                    </form>
                </div>
            </details>
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
            // v0.48.2: the a11y "badges" are informational claims that
            // the app respects keyboard / screen-reader / contrast
            // affordances. They are NOT links or buttons — the previous
            // styling (default body text + tooltip-on-hover) read as
            // "interactive but broken" to operators. Restyled as
            // recessive informational chips with a small leading icon
            // and a `role="note"` so assistive technology treats them
            // as ancillary content. Tooltips remain via `title=`.
            //
            // Future work (post-v1.0): when the docs site adds an
            // a11y chapter, these could grow `href`s and become real
            // links to the corresponding section. Until then, passive
            // badges are the honest representation.
            <ul class="app-footer__a11y" role="note"
                aria-label=t.footer_a11y_group_label>
                <li class="app-footer__a11y-item" title=t.a11y_keyboard>
                    <span class="app-footer__a11y-icon" aria-hidden="true">"⌨"</span>
                    {t.a11y_keyboard}
                </li>
                <li class="app-footer__a11y-item" title=t.a11y_screen_reader>
                    <span class="app-footer__a11y-icon" aria-hidden="true">"⊙"</span>
                    {t.a11y_screen_reader}
                </li>
                <li class="app-footer__a11y-item" title=t.a11y_contrast>
                    <span class="app-footer__a11y-icon" aria-hidden="true">"◐"</span>
                    {t.a11y_contrast}
                </li>
            </ul>
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
        // RFC 092: no-JS fallback note. Hidden by `.js .theme-no-js-note { display:none }`.
        // Visible only when JavaScript is disabled; informs the user that
        // theme follows system preference.
        <p class="theme-no-js-note muted">
            {t.theme_noscript_note}
        </p>
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests;
