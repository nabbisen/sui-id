//! Page renderers for the "error" screen domain (RFC 065).

use super::common::*;
use leptos::prelude::*;

pub fn render_error(status: u16, request_id: &str, lang: sui_id_i18n::Locale) -> String {
    let t = lang.strings();
    let (title, lede) = match status {
        404 => (t.error_not_found_title, t.error_not_found_lede),
        429 => (
            t.error_too_many_requests_label,
            t.error_too_many_requests_lede,
        ),
        500..=599 => (t.error_internal, t.error_internal_lede),
        _ => (t.error_generic_title, t.error_generic_lede),
    };
    let rid = request_id.to_string();
    let req_id_label = t.error_request_id_label;
    let back_home = t.error_back_home;
    // v0.48.1 (Bug 9 defense-in-depth): when an Unauthorized page
    // does still render (e.g. for a non-Unauthenticated 401 path,
    // or future authentication errors that don't redirect),
    // "Back home" should point at /admin/login rather than `/`.
    // The latter is a root handler that just redirects to /admin
    // again for any initialised installation, producing a loop.
    let back_home_href = if status == 401 { "/admin/login" } else { "/" };
    render(move || {
        view! {
            <crate::layout::AuthShell title=title.to_string() lang=lang>
                <div class="auth-card">
                    <h1>{status.to_string()}</h1>
                    <h2>{title}</h2>
                    <p class="muted">{lede}</p>
                    <p class="muted text-small">
                        {req_id_label} ": "
                        <span class="code">{rid}</span>
                    </p>
                    <p>
                        <a href=back_home_href class="button secondary">{back_home}</a>
                    </p>
                </div>
            </crate::layout::AuthShell>
        }
    })
}
