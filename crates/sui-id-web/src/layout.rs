//! Common HTML shell shared by every page.
//!
//! Composes the design tokens (`tokens.rs`) and component styles
//! (`components.rs`) and adds:
//!
//! - The early inline script that resolves the user's theme choice
//!   from `localStorage` *synchronously* on the document root, before
//!   first paint, to avoid a flash of unthemed content (FOUT).
//! - The footer with the theme toggle (light / dark / auto) and the
//!   accessibility badges.
//! - The admin nav (when `show_nav` is true).

use crate::components::COMPONENTS_CSS;
use crate::tokens::TOKENS_CSS;
use leptos::prelude::*;

/// Inline script that runs *synchronously* in `<head>` before body
/// paint. Reads `localStorage["sui_id_theme"]` (one of "light" /
/// "dark" / "system"; missing or invalid values fall back to
/// "system") and sets `data-theme` on `<html>`. When the saved
/// value is "system" we follow `prefers-color-scheme` and listen
/// for changes so the page tracks OS appearance live.
///
/// This is the only piece of JS we ship for theming. The toggle
/// buttons in the footer also write to localStorage and update
/// `data-theme` immediately; on page navigation the cycle starts
/// fresh from this snippet.
const THEME_INIT_JS: &str = r#"
(function () {
  try {
    var KEY = "sui_id_theme";
    var saved = localStorage.getItem(KEY);
    var mode = (saved === "light" || saved === "dark") ? saved : "system";
    var root = document.documentElement;
    function apply(m) {
      if (m === "system") {
        root.removeAttribute("data-theme");
      } else {
        root.setAttribute("data-theme", m);
      }
    }
    apply(mode);
    if (mode === "system" && window.matchMedia) {
      var mq = window.matchMedia("(prefers-color-scheme: dark)");
      var listener = function () { /* CSS handles it via :not([data-theme]) */ };
      if (mq.addEventListener) mq.addEventListener("change", listener);
    }
    // Expose a tiny helper used by the footer toggle.
    window.__suiIdSetTheme = function (m) {
      if (m !== "light" && m !== "dark" && m !== "system") return;
      try { localStorage.setItem(KEY, m); } catch (e) {}
      apply(m);
      // Update aria-pressed on toggle buttons if present.
      document.querySelectorAll(".theme-toggle__btn").forEach(function (b) {
        b.setAttribute("aria-pressed", b.getAttribute("data-theme-value") === m ? "true" : "false");
      });
    };
    // Initialise aria-pressed on first load (defer until DOM ready).
    var setPressed = function () {
      document.querySelectorAll(".theme-toggle__btn").forEach(function (b) {
        b.setAttribute("aria-pressed", b.getAttribute("data-theme-value") === mode ? "true" : "false");
      });
    };
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", setPressed);
    } else {
      setPressed();
    }
  } catch (e) {}
})();
"#;

/// Inline JS for copy-to-clipboard buttons (RFC 028).
/// Attached as a delegated `click` handler; triggers carry `data-copy="VALUE"`.
const COPY_JS: &str = r#"
(function () {
  if (!navigator.clipboard) return;
  // Mark document so CSS can show .copy-btn elements.
  document.documentElement.classList.add('clipboard-available');
  document.addEventListener('click', function (e) {
    var btn = e.target.closest('[data-copy]');
    if (!btn) return;
    var value = btn.getAttribute('data-copy');
    navigator.clipboard.writeText(value).then(function () {
      var orig = btn.textContent;
      btn.setAttribute('aria-pressed','true');
      // RFC 053: localised "Copied" text carried on each button via
      // data-copy-done. Falls back to a Unicode check + English if
      // missing.
      var done = btn.getAttribute('data-copy-done') || '\u2713 Copied';
      btn.textContent = done;
      setTimeout(function () {
        btn.textContent = orig;
        btn.removeAttribute('aria-pressed');
      }, 1800);
    }).catch(function () {});
  });
})();
"#;

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
                <script>{THEME_INIT_JS}</script>
                <script>{COPY_JS}</script>
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
                <script>{THEME_INIT_JS}</script>
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
            <script>
                r#"(function(){
                    var f=document.getElementById('logout-csrf');
                    if(f&&!f.value){
                        var m=document.cookie.match(/(?:^|; )sui_id_csrf=([^;]*)/);
                        if(m) f.value=decodeURIComponent(m[1]);
                    }
                }())"#
            </script>
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
    // Initial aria-pressed values are filled in by THEME_INIT_JS once
    // localStorage is read; SSR cannot know the user's preference, so
    // we render all three buttons un-pressed and rely on the script.
    view! {
        <div class="theme-toggle" role="group" aria-label=t.theme_toggle_group>
            <button class="theme-toggle__btn"
                    type="button"
                    data-theme-value="light"
                    aria-pressed="false"
                    onclick="window.__suiIdSetTheme && window.__suiIdSetTheme('light')"
                    title=t.theme_toggle_light_title>
                "☀ " {t.theme_toggle_light}
            </button>
            <button class="theme-toggle__btn"
                    type="button"
                    data-theme-value="system"
                    aria-pressed="false"
                    onclick="window.__suiIdSetTheme && window.__suiIdSetTheme('system')"
                    title=t.theme_toggle_auto_title>
                "🖥 " {t.theme_toggle_auto}
            </button>
            <button class="theme-toggle__btn"
                    type="button"
                    data-theme-value="dark"
                    aria-pressed="false"
                    onclick="window.__suiIdSetTheme && window.__suiIdSetTheme('dark')"
                    title=t.theme_toggle_dark_title>
                "☾ " {t.theme_toggle_dark}
            </button>
        </div>
    }
}
