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

/// Wrap a page body in the standard sui-id chrome.
#[component]
pub fn Shell(
    title: String,
    show_nav: bool,
    current: Option<String>,
    children: Children,
) -> impl IntoView {
    let stylesheet = format!("{}\n{}", TOKENS_CSS, COMPONENTS_CSS);
    view! {
        <html lang="en">
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
                    {show_nav.then(|| view! { <Nav current=current.clone() /> })}
                </header>
                <main class="app-main">{children()}</main>
                <Footer />
            </body>
        </html>
    }
}

/// Centred narrow layout for login / setup. Same chrome but the
/// main column is narrowed and vertically centred — produces the
/// "card on a quiet field" look the design memo asks for on
/// auth-style screens.
#[component]
pub fn AuthShell(title: String, children: Children) -> impl IntoView {
    let stylesheet = format!("{}\n{}", TOKENS_CSS, COMPONENTS_CSS);
    view! {
        <html lang="en">
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
                <Footer />
            </body>
        </html>
    }
}

#[component]
fn Nav(current: Option<String>) -> impl IntoView {
    let items = [
        ("dashboard", "Dashboard", "/admin"),
        ("users", "Users", "/admin/users"),
        ("clients", "Clients", "/admin/clients"),
        ("signing-keys", "Keys", "/admin/signing-keys"),
        ("audit", "Audit", "/admin/audit"),
        ("settings", "Settings", "/admin/settings"),
        ("profile", "Profile", "/admin/profile"),
    ];
    view! {
        <nav class="app-nav" aria-label="Main">
            {items.into_iter().map(|(key, label, href)| {
                let aria = if current.as_deref() == Some(key) { Some("page") } else { None };
                view! {
                    <a class="app-nav__link" href=href aria-current=aria>{label}</a>
                }
            }).collect::<Vec<_>>()}
            <a class="app-nav__link app-nav__signout" href="/admin/logout">"Sign out"</a>
        </nav>
    }
}

#[component]
fn Footer() -> impl IntoView {
    view! {
        <footer class="app-footer" role="contentinfo">
            <span class="app-footer__tagline">
                "🌱 sui-id · 静かで、凛として、やさしい ID 基盤を。"
            </span>
            <span class="app-footer__a11y" aria-label="Accessibility features">
                <span title="Keyboard accessible">"⌨ Keyboard"</span>
                <span title="Screen reader friendly">"⊙ Screen reader"</span>
                <span title="High contrast">"◐ Contrast"</span>
            </span>
            <ThemeToggle />
            <span class="app-footer__version">{format!("v{}", env!("CARGO_PKG_VERSION"))}</span>
        </footer>
    }
}

#[component]
fn ThemeToggle() -> impl IntoView {
    // Initial aria-pressed values are filled in by THEME_INIT_JS once
    // localStorage is read; SSR cannot know the user's preference, so
    // we render all three buttons un-pressed and rely on the script.
    view! {
        <div class="theme-toggle" role="group" aria-label="Theme">
            <button class="theme-toggle__btn"
                    type="button"
                    data-theme-value="light"
                    aria-pressed="false"
                    onclick="window.__suiIdSetTheme && window.__suiIdSetTheme('light')"
                    title="Light theme">
                "☀ Light"
            </button>
            <button class="theme-toggle__btn"
                    type="button"
                    data-theme-value="system"
                    aria-pressed="false"
                    onclick="window.__suiIdSetTheme && window.__suiIdSetTheme('system')"
                    title="Follow system">
                "🖥 Auto"
            </button>
            <button class="theme-toggle__btn"
                    type="button"
                    data-theme-value="dark"
                    aria-pressed="false"
                    onclick="window.__suiIdSetTheme && window.__suiIdSetTheme('dark')"
                    title="Dark theme">
                "☾ Dark"
            </button>
        </div>
    }
}
