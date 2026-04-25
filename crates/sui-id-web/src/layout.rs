//! Common HTML shell shared by every page.
//!
//! Style choices follow the project ethos: muted, readable, accessible,
//! not flashy. The whole stylesheet is inlined to avoid a second request
//! and to make the binary truly self-contained.

use leptos::prelude::*;

const STYLE: &str = r#"
:root {
  color-scheme: light dark;
  --bg: #f7f6f3;
  --fg: #1c1c1c;
  --muted: #5a5a5a;
  --accent: #4a7a6a;
  --border: #d5d2cb;
  --danger: #8a3a3a;
  --warn: #8a7a3a;
  font-family: system-ui, -apple-system, "Helvetica Neue", sans-serif;
}
@media (prefers-color-scheme: dark) {
  :root { --bg: #1c1f1e; --fg: #ececec; --muted: #a3a3a3; --accent: #8fbfae; --border: #383838; }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg); line-height: 1.55; }
header { padding: 1.25rem 2rem; border-bottom: 1px solid var(--border); display: flex; align-items: baseline; gap: 1.5rem; }
header h1 { margin: 0; font-size: 1.1rem; font-weight: 600; letter-spacing: 0.02em; }
header nav a { color: var(--muted); text-decoration: none; margin-right: 1rem; font-size: 0.95rem; }
header nav a:hover, header nav a[aria-current="page"] { color: var(--fg); }
main { max-width: 56rem; margin: 0 auto; padding: 2rem; }
h2 { font-size: 1.4rem; font-weight: 600; margin-top: 2rem; }
h3 { font-size: 1.1rem; font-weight: 600; }
form { margin-block: 1rem; }
label { display: block; font-size: 0.9rem; color: var(--muted); margin-bottom: 0.25rem; }
input[type=text], input[type=password], input[type=email], input[type=url], textarea {
  width: 100%; padding: 0.55rem 0.75rem; border: 1px solid var(--border);
  border-radius: 4px; background: transparent; color: inherit;
  font-family: inherit; font-size: 1rem; margin-bottom: 1rem;
}
input:focus, textarea:focus { outline: 2px solid var(--accent); outline-offset: 1px; }
button, .button { padding: 0.55rem 1rem; border: 1px solid var(--accent); background: var(--accent); color: white;
  border-radius: 4px; font-size: 0.95rem; cursor: pointer; }
button.secondary, .button.secondary { background: transparent; color: var(--accent); }
button.danger, .button.danger { background: var(--danger); border-color: var(--danger); }
table { width: 100%; border-collapse: collapse; margin-block: 1rem; }
th, td { text-align: left; padding: 0.55rem 0.75rem; border-bottom: 1px solid var(--border); font-size: 0.95rem; vertical-align: top; }
th { color: var(--muted); font-weight: 500; }
.flash { padding: 0.75rem 1rem; border-radius: 4px; margin-block: 1rem; }
.flash.info { background: rgba(74,122,106,0.12); border: 1px solid var(--accent); }
.flash.warn { background: rgba(138,122,58,0.12); border: 1px solid var(--warn); }
.flash.error { background: rgba(138,58,58,0.10); border: 1px solid var(--danger); }
.muted { color: var(--muted); font-size: 0.9rem; }
.code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.9rem;
  background: rgba(0,0,0,0.05); padding: 0.15rem 0.35rem; border-radius: 3px; word-break: break-all; }
@media (prefers-color-scheme: dark) {
  .code { background: rgba(255,255,255,0.06); }
}
footer { color: var(--muted); font-size: 0.85rem; text-align: center; padding: 2rem 1rem; }
"#;

/// Wrap a page body in the standard sui-id chrome.
#[component]
pub fn Shell(
    title: String,
    show_nav: bool,
    current: Option<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <meta name="referrer" content="same-origin" />
                <title>{format!("{title} · sui-id")}</title>
                <style>{STYLE}</style>
            </head>
            <body>
                <header>
                    <h1>"sui-id"</h1>
                    {show_nav.then(|| view! { <Nav current=current.clone() /> })}
                </header>
                <main>{children()}</main>
                <footer>"sui-id · self-hosted, quiet, careful"</footer>
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
        ("audit", "Audit", "/admin/audit"),
    ];
    view! {
        <nav>
            {items.into_iter().map(|(key, label, href)| {
                let aria = if current.as_deref() == Some(key) { Some("page") } else { None };
                view! { <a href=href aria-current=aria>{label}</a> }
            }).collect::<Vec<_>>()}
            <a href="/admin/logout" style="margin-left:auto">"Sign out"</a>
        </nav>
    }
}
