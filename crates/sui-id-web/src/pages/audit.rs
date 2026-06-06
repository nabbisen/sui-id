//! Page renderers for the "audit" screen domain (RFC 065).

use leptos::prelude::*;
use crate::layout::Shell;
use super::common::*;
use sui_id_shared::api::AuditLogEntryDto;

pub(super) fn audit_row_view(t: &'static sui_id_i18n::Strings, e: AuditLogEntryDto) -> impl IntoView {
    let result_badge = match e.result.as_str() {
        "ok" => view! { <span class="badge badge--ok">"ok"</span> }.into_any(),
        "fail" | "error" | "denied" => {
            view! { <span class="badge badge--danger">{e.result.clone()}</span> }.into_any()
        }
        _ => view! { <span class="badge">{e.result.clone()}</span> }.into_any(),
    };
    // RFC 046: stable copyable row identifier — time|actor|action|target
    let row_id = format!(
        "{}|{}|{}|{}",
        e.at.format("%Y-%m-%dT%H:%M:%SZ"),
        e.actor.map(|a| a.to_string()).unwrap_or_else(|| "-".into()),
        e.action,
        e.target.clone().unwrap_or_default(),
    );
    let actor_str = e.actor.map(|a| a.to_string()).unwrap_or_else(|| "-".into());
    view! {
        <tr>
            <td class="muted">{fmt_time(e.at)}</td>
            <td><span class="code">{actor_str}</span></td>
            <td>{e.action}</td>
            <td><span class="code">{e.target.unwrap_or_default()}</span></td>
            <td>{result_badge}</td>
            <td>{copy_btn(t, row_id, t.copy_noun_audit_row_id)}</td>
        </tr>
    }
}


pub fn render_audit(
    entries: Vec<AuditLogEntryDto>,
    chain_ok: bool,
    filter_query: Option<String>,
    flash: Option<Flash>,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
) -> String {
    render(move || {
        let t = lang.strings();
        let entry_count = entries.len();
        let fq = filter_query.clone().unwrap_or_default();
        let csv_href = if fq.is_empty() {
            "/admin/audit.csv".to_string()
        } else {
            format!("/admin/audit.csv?q={}", url_encode(&fq))
        };
        let fq_display = fq.clone();
        let rows: Vec<_> = entries.into_iter().map(|e| audit_row_view(t, e)).collect();
        let chain_banner_view = if chain_ok {
            view! {
                <p class="badge badge--ok mb-3">
                    "✓ " {t.audit_chain_ok}
                </p>
            }.into_any()
        } else {
            view! {
                <p class="badge badge--danger mb-3">
                    "✗ " {t.audit_chain_broken}
                </p>
            }.into_any()
        };
        view! {
            <Shell title=t.audit_title.to_string() show_nav=true current=Some("audit".to_string()) dev_mode=dev_mode lang=lang>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.audit_title}</h1>
                        <p class="page-header__lede">
                            {t.audit_lede}
                            " "
                            {(t.audit_entry_count_caption)(entry_count)}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {chain_banner_view}
                <div class="row" style="gap:var(--space-3);margin-bottom:var(--space-3);align-items:flex-end;flex-wrap:wrap">
                    <form method="get" action="/admin/audit" class="row row-gap2-center">
                        <label for="audit-q" class="fw-500">{t.audit_filter_label}</label>
                        <input id="audit-q" name="q" type="search"
                               placeholder=t.audit_filter_placeholder
                               value=fq_display
                               class="min-w-16rem" />
                        <button type="submit" class="secondary">{t.audit_filter_button}</button>
                    </form>
                    <a href=csv_href class="button secondary">{t.audit_export_csv}</a>
                </div>
                <div class="table-wrap">
                    <table>
                        <thead>
                            <tr>
                                <th>{t.audit_col_when}</th>
                                <th>{t.audit_col_actor}</th>
                                <th>{t.audit_col_action}</th>
                                <th>{t.audit_col_target}</th>
                                <th>{t.audit_col_outcome}</th>
                            </tr>
                        </thead>
                        {if rows.is_empty() {
                            view! {
                                <tbody><tr><td colspan="5" class="muted center-pad-6">
                                    "(no matching entries)"
                                </td></tr></tbody>
                            }.into_any()
                        } else {
                            view! { <tbody>{rows}</tbody> }.into_any()
                        }}
                    </table>
                </div>
            </Shell>
        }
    })
}


fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
