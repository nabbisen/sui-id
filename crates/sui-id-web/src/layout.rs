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
//! - The admin console's grouped sidebar (RFC 149 §1), with the
//!   account menu and the version at its foot, for [`Chrome::Console`];
//!   a slim top bar with the account menu alone, for [`Chrome::Account`].
//!   RFC 149 §4 removes the old footer (tagline, a11y badges, theme
//!   toggle) entirely; the theme selector moves into the account menu
//!   (§3), and the version is shown only with the console's chrome.

use crate::components::components_css;
use crate::tokens::TOKENS_CSS;
use leptos::prelude::*;

/// RFC 149 §1: one destination, one variant. The sidebar is rendered
/// from [`NavItem::ALL`], grouped by [`NavItem::group`] — an
/// exhaustive match, so a new variant cannot compile without a group,
/// and cannot be left out of the sidebar (which renders the whole
/// list, not a hand-kept subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavItem {
    Dashboard,
    Users,
    Apps,
    FederationProviders,
    SignInSettings,
    SigningKeys,
    SessionsAndLockout,
    AuditLog,
    General,
    Email,
    Logs,
    AboutThisServer,
}

/// RFC 149 §1's six groups, in the order an operator meets them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavGroup {
    Overview,
    People,
    Applications,
    SignIn,
    Security,
    System,
}

impl NavItem {
    /// Every destination, in the exact order §1's table lists them —
    /// the sidebar's own order, and the one source [`crate::layout`]'s
    /// tests pin the render against.
    pub const ALL: [NavItem; 12] = [
        NavItem::Dashboard,
        NavItem::Users,
        NavItem::Apps,
        NavItem::FederationProviders,
        NavItem::SignInSettings,
        NavItem::SigningKeys,
        NavItem::SessionsAndLockout,
        NavItem::AuditLog,
        NavItem::General,
        NavItem::Email,
        NavItem::Logs,
        NavItem::AboutThisServer,
    ];

    pub fn group(self) -> NavGroup {
        match self {
            NavItem::Dashboard => NavGroup::Overview,
            NavItem::Users => NavGroup::People,
            NavItem::Apps => NavGroup::Applications,
            NavItem::FederationProviders | NavItem::SignInSettings => NavGroup::SignIn,
            NavItem::SigningKeys | NavItem::SessionsAndLockout | NavItem::AuditLog => {
                NavGroup::Security
            }
            NavItem::General | NavItem::Email | NavItem::Logs | NavItem::AboutThisServer => {
                NavGroup::System
            }
        }
    }

    pub fn href(self) -> &'static str {
        match self {
            NavItem::Dashboard => "/admin",
            NavItem::Users => "/admin/users",
            NavItem::Apps => "/admin/clients",
            NavItem::FederationProviders => "/admin/federation-providers",
            NavItem::SignInSettings => "/admin/settings/authentication",
            NavItem::SigningKeys => "/admin/signing-keys",
            NavItem::SessionsAndLockout => "/admin/settings/security",
            NavItem::AuditLog => "/admin/audit",
            NavItem::General => "/admin/settings/basic",
            NavItem::Email => "/admin/settings/email",
            NavItem::Logs => "/admin/settings/logs",
            NavItem::AboutThisServer => "/admin/settings/other",
        }
    }

    pub fn label(self, t: &'static sui_id_i18n::Strings) -> &'static str {
        match self {
            NavItem::Dashboard => t.nav_dashboard,
            NavItem::Users => t.nav_users,
            NavItem::Apps => t.nav_apps,
            NavItem::FederationProviders => t.nav_federation_providers,
            NavItem::SignInSettings => t.nav_item_sign_in_settings,
            NavItem::SigningKeys => t.nav_signing_keys,
            NavItem::SessionsAndLockout => t.nav_item_sessions_and_lockout,
            NavItem::AuditLog => t.nav_audit,
            NavItem::General => t.settings_tab_general,
            NavItem::Email => t.settings_tab_email,
            NavItem::Logs => t.settings_tab_logs,
            NavItem::AboutThisServer => t.nav_item_about_this_server,
        }
    }
}

impl NavGroup {
    pub const ALL: [NavGroup; 6] = [
        NavGroup::Overview,
        NavGroup::People,
        NavGroup::Applications,
        NavGroup::SignIn,
        NavGroup::Security,
        NavGroup::System,
    ];

    pub fn label(self, t: &'static sui_id_i18n::Strings) -> &'static str {
        match self {
            NavGroup::Overview => t.nav_group_overview,
            NavGroup::People => t.nav_group_people,
            NavGroup::Applications => t.nav_group_applications,
            NavGroup::SignIn => t.nav_group_sign_in,
            NavGroup::Security => t.nav_group_security,
            NavGroup::System => t.nav_group_system,
        }
    }
}

/// RFC 144, *Refresh*: the navigation **is** the signed-in person's
/// name, or there is no navigation — there is no state where a page
/// shows navigation to someone the type system cannot name. Replaces
/// the former `show_nav: bool` + `admin_username: Option<String>` pair
/// on [`Shell`], which let 38 of 39 pages fall through to a bare *Sign
/// out* with no account menu (measured 2026-11-15; see RFC 144's
/// refresh and N2a's review, F5).
///
/// RFC 144 N2c: the account menu is "about identity, not a place to
/// go", so it does not imply the console links — [`Chrome::for_person`]
/// picks between [`Chrome::Console`] and [`Chrome::Account`] by the same
/// predicate the console's own gate uses, so the navigation offered and
/// the access allowed cannot disagree.
#[derive(Debug, Clone)]
pub enum Chrome {
    /// An admin console page: the sidebar, with `item` marked current,
    /// and the account menu for `username`. RFC 149 §1: the item lives
    /// in the variant, so an `/admin` page cannot render the console's
    /// chrome without naming its place in the navigation.
    Console { username: String, item: NavItem },
    /// The console's chrome on the person's own pages (`/me`, and the
    /// forced MFA-setup and password-change pages) for someone who can
    /// read the console: the same sidebar, with no item current, since
    /// these pages belong to the account menu, not to a group.
    ConsolePersonal { username: String },
    /// RFC 144 N2c: a signed-in person outside the console (`role`
    /// cannot read the admin console) — the account menu alone, with no
    /// console links to a page the server would then refuse.
    Account { username: String },
    /// No navigation: sign-in, setup, error pages.
    Bare,
}

impl Chrome {
    /// Chosen by the same predicate the console's own gate uses
    /// ([`sui_id_store::models::Role::can_read_admin`],
    /// `CurrentAdminOrAuditor`'s check), so the navigation offered and
    /// the access allowed cannot disagree.
    pub fn for_person(username: String, role: sui_id_store::models::Role) -> Self {
        if role.can_read_admin() {
            Self::ConsolePersonal { username }
        } else {
            Self::Account { username }
        }
    }
}

/// Wrap a page body in the standard sui-id chrome.
///
/// `lang` controls the `<html lang="…">` attribute and propagates
/// down to `Sidebar`, `AccountNav`, and the account menu's theme
/// selector so every label in the admin chrome reads in the user's
/// locale (RFC 050).
#[component]
pub fn Shell(
    title: String,
    nav: Chrome,
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
                {match nav {
                    Chrome::Console { username, item } => view! {
                        <ConsoleShell current=Some(item) lang=lang
                                      csrf_token=csrf_token.clone()
                                      username=username>
                            {children()}
                        </ConsoleShell>
                    }.into_any(),
                    Chrome::ConsolePersonal { username } => view! {
                        <ConsoleShell current=None lang=lang
                                      csrf_token=csrf_token.clone()
                                      username=username>
                            {children()}
                        </ConsoleShell>
                    }.into_any(),
                    Chrome::Account { username } => view! {
                        <header class="app-header" role="banner">
                            <span class="app-header__brand">"sui-id"</span>
                            <AccountNav lang=lang
                                        csrf_token=csrf_token.clone()
                                        username=username />
                        </header>
                        <main class="app-main" id="main-content">{children()}</main>
                    }.into_any(),
                    Chrome::Bare => view! {
                        <header class="app-header" role="banner">
                            <span class="app-header__brand">"sui-id"</span>
                        </header>
                        <main class="app-main" id="main-content">{children()}</main>
                    }.into_any(),
                }}
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
                    <span class="app-header__brand">"sui-id"</span>
                </header>
                <main class="app-main app-main--narrow auth-page" id="main-content">
                    <div class="auth-card">{children()}</div>
                </main>
            </body>
        </html>
    }
}

/// RFC 149 §1/§2: the console's grouped sidebar, wide-screen, and the
/// same list folded into a `Menu` `<details>` for narrow screens — two
/// renders of [`sidebar_content`], toggled by a media query so exactly
/// one is in the accessibility tree and the tab order at any width
/// (§2: a `<details>` cannot be held open on wide screens without a
/// script or an unsupported pseudo-element, so one copy per width is
/// the shape this dispatch's own escape clause requires an alternative
/// to beat with a measurement, not assume away).
#[component]
fn ConsoleShell(
    current: Option<NavItem>,
    lang: sui_id_i18n::Locale,
    csrf_token: String,
    username: String,
    children: Children,
) -> impl IntoView {
    let t = lang.strings();
    view! {
        <div class="app-shell">
            <div class="app-sidebar app-sidebar--wide">
                {sidebar_content(current, t, username.clone(), csrf_token.clone())}
            </div>
            <details class="app-sidebar app-sidebar--narrow">
                <summary class="app-sidebar__menu-toggle">{t.nav_menu_toggle}</summary>
                <div class="app-sidebar__menu-panel">
                    {sidebar_content(current, t, username, csrf_token)}
                </div>
            </details>
            <div class="app-shell__body">
                <main class="app-main" id="main-content">{children()}</main>
            </div>
        </div>
    }
}

/// The sidebar's own content: the brand, every group with its items
/// (from [`NavItem::ALL`], grouped by [`NavItem::group`] — the single
/// source the sidebar renders from, not a hand-kept list), the account
/// menu, and the version — rendered twice by [`ConsoleShell`], once
/// per screen width.
fn sidebar_content(
    current: Option<NavItem>,
    t: &'static sui_id_i18n::Strings,
    username: String,
    csrf_token: String,
) -> AnyView {
    view! {
        <p class="app-sidebar__brand">"sui-id"</p>
        <nav class="app-sidebar__nav" aria-label=t.nav_aria_main>
            {NavGroup::ALL.into_iter().map(|group| {
                let items: Vec<_> = NavItem::ALL
                    .into_iter()
                    .filter(|item| item.group() == group)
                    .collect();
                view! {
                    <p class="app-sidebar__group">{group.label(t)}</p>
                    {items.into_iter().map(|item| {
                        let aria = (current == Some(item)).then_some("page");
                        view! {
                            <a class="app-sidebar__link" href=item.href() aria-current=aria>
                                {item.label(t)}
                            </a>
                        }
                    }).collect::<Vec<_>>()}
                }
            }).collect::<Vec<_>>()}
        </nav>
        <div class="app-sidebar__account">
            {account_menu(t, username, csrf_token)}
            <span class="app-sidebar__version">
                {format!("sui-id v{}", env!("CARGO_PKG_VERSION"))}
            </span>
        </div>
    }
    .into_any()
}

/// RFC 144 N2c: the account menu alone, in a slim top bar, for
/// [`Chrome::Account`] — a signed-in person with no console to
/// navigate. No version (RFC 149 §4: shown only with the console's
/// chrome).
#[component]
fn AccountNav(lang: sui_id_i18n::Locale, csrf_token: String, username: String) -> impl IntoView {
    let t = lang.strings();
    view! {
        <nav class="app-nav" aria-label=t.nav_aria_main>
            {account_menu(t, username, csrf_token)}
        </nav>
    }
}

/// RFC 149 §3: the account menu — *My account*, *Language*, the theme
/// selector (moved here from the footer), and *Sign out*, separated
/// from the rest. Plain links and buttons in a list: no `role="menu"`
/// or `role="menuitem"` (a menu role promises arrow-key navigation,
/// and no script provides it).
///
/// Used by both [`sidebar_content`] (at the sidebar's foot) and
/// [`AccountNav`] (in the top bar) — the same contents either way;
/// only where it sits differs. Signed-out pages render neither, so they
/// have no theme selector (§3).
///
/// Returns [`AnyView`], erasing the nested element types: rendered inside
/// [`sidebar_content`] twice, a typed tree here overflowed the compiler's
/// query depth on the project's oldest supported toolchain (1.95) while
/// building on stable (Q2's landing, CI run 45910338462).
fn account_menu(t: &'static sui_id_i18n::Strings, username: String, csrf_token: String) -> AnyView {
    view! {
        <details class="user-menu">
            <summary class="app-nav__link user-menu__toggle">
                {username} " ▾"
            </summary>
            <ul class="user-menu__panel">
                <li>
                    <a class="user-menu__item" href="/me/security/overview">
                        {t.nav_my_account}
                    </a>
                </li>
                <li>
                    <a class="user-menu__item" href="/me/security/language">
                        {t.me_tab_language}
                    </a>
                </li>
                {theme_selector_item(t)}
                <li class="user-menu__signout">
                    <form method="post" action="/admin/logout"
                          class="user-menu__form">
                        <input type="hidden" name="_csrf" value=csrf_token.clone() />
                        <button type="submit" class="user-menu__item">
                            {t.nav_logout}
                        </button>
                    </form>
                </li>
            </ul>
        </details>
    }
    .into_any()
}

/// RFC 149 §3: the theme selector, as one account-menu row — a visible
/// name (*Theme*), then the three buttons in a labelled group, so
/// *Auto* is never read without saying what is automatic. No `title=`
/// (the visible label is the name; a `title` is a hover-only tooltip),
/// and *Auto* is never relabelled *System* (that name is now a sidebar
/// group, and the two would be confused). Click handlers and
/// `aria-pressed` are attached by `/static/theme-init.js`, keyed off
/// `data-theme-value` — unchanged by this move. With JavaScript off,
/// the no-js note (hidden once `js` loads) replaces the buttons.
fn theme_selector_item(t: &'static sui_id_i18n::Strings) -> AnyView {
    view! {
        <li class="user-menu__theme">
            <span class="user-menu__theme-label" aria-hidden="true">
                {t.theme_toggle_group}
            </span>
            <div class="user-menu__theme-buttons" role="group" aria-label=t.theme_toggle_group>
                <button class="user-menu__item theme-toggle__btn"
                        type="button"
                        data-theme-value="light"
                        aria-pressed="false">
                    "☀ " {t.theme_toggle_light}
                </button>
                <button class="user-menu__item theme-toggle__btn"
                        type="button"
                        data-theme-value="system"
                        aria-pressed="false">
                    "🖥 " {t.theme_toggle_auto}
                </button>
                <button class="user-menu__item theme-toggle__btn"
                        type="button"
                        data-theme-value="dark"
                        aria-pressed="false">
                    "☾ " {t.theme_toggle_dark}
                </button>
            </div>
            // RFC 092: no-JS fallback note. Hidden by `.js .theme-no-js-note { display:none }`.
            <p class="theme-no-js-note muted">{t.theme_noscript_note}</p>
        </li>
    }
    .into_any()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests;
