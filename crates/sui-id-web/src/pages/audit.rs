//! Page renderers for the "audit" screen domain (RFC 065).

use super::common::*;
use crate::components::empty_state;
use crate::layout::{Chrome, NavItem, Shell};
use leptos::prelude::*;
use sui_id_shared::api::AuditLogEntryDto;

/// The number of cells [`audit_row_view`] renders, and so the number of
/// columns [`audit_table_head`] heads. A table that shows audit rows takes
/// both from here, so its header cannot disagree with its rows (the user
/// detail page once headed six cells with three).
///
/// Five, not six (RFC 149 Q5 §3): the row's own copy button moved into
/// the target cell, beside the truncated value it copies, rather than
/// owning a column of its own — at 1280px, six columns (one of them a
/// bare action button) scrolled the table sideways; five fit.
pub(super) const AUDIT_COLUMNS: usize = 5;

/// The header for a table of [`audit_row_view`] rows: one `<th>` per cell.
/// `sticky`, inside the table's own scrolling box (`.table-wrap--scroll`
/// on the caller's wrapper), so it stays in view while a long page
/// scrolls (RFC 149 Q5 §3).
pub(super) fn audit_table_head(t: &'static sui_id_i18n::Strings) -> impl IntoView {
    view! {
        <thead class="table-head--sticky">
            <tr>
                <th class="cell-nowrap">{t.audit_col_when}</th>
                <th class="cell-nowrap">{t.audit_col_actor}</th>
                <th>{t.audit_col_action}</th>
                <th>{t.audit_col_target}</th>
                <th class="cell-nowrap">{t.audit_col_outcome}</th>
            </tr>
        </thead>
    }
}

pub(super) fn audit_row_view(
    t: &'static sui_id_i18n::Strings,
    e: AuditLogEntryDto,
) -> impl IntoView {
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
    // RFC-MI-031: cell-nowrap on time/actor (stable columns),
    // cell-id on target (opaque UUID or resource slug) — RFC 149 Q5
    // §3: that one cell now also carries the copy button, beside the
    // truncated value, rather than a `.cell-actions` column of its own.
    view! {
        <tr>
            <td class="muted cell-nowrap">{fmt_time(e.at)}</td>
            <td class="cell-nowrap"><span class="code">{actor_str}</span></td>
            <td class="cell-wrap">{e.action}</td>
            <td class="cell-wrap">
                <span class="code cell-id-inline">{e.target.unwrap_or_default()}</span>
                {copy_btn(t, row_id, t.copy_noun_audit_row_id)}
            </td>
            <td class="cell-nowrap">{result_badge}</td>
        </tr>
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_audit(
    entries: Vec<AuditLogEntryDto>,
    total: i64,
    per_page: usize,
    chain: ChainStatus,
    filter_query: Option<String>,
    newer_href: Option<String>,
    older_href: Option<String>,
    flash: Option<Flash>,
    csrf_token: String,
    dev_mode: bool,
    lang: sui_id_i18n::Locale,
    username: String,
) -> String {
    render(move || {
        let t = lang.strings();
        let pager = crate::components::pager::seq_pager_view(t, newer_href, older_href);
        let fq = filter_query.clone().unwrap_or_default();
        let csv_href = if fq.is_empty() {
            "/admin/audit.csv".to_string()
        } else {
            format!("/admin/audit.csv?q={}", url_encode(&fq))
        };
        let fq_display = fq.clone();
        let rows: Vec<_> = entries.into_iter().map(|e| audit_row_view(t, e)).collect();
        // RFC 121 D1, D2: the label comes from the one place every surface
        // that shows a chain result reads it from, so this page and the
        // settings logs page cannot describe the same outcome differently.
        let (prefix, class) = match chain {
            ChainStatus::Intact { .. } => ("✓ ", "badge badge--ok mb-3"),
            ChainStatus::Broken { .. } => ("✗ ", "badge badge--danger mb-3"),
            ChainStatus::CouldNotVerify { .. } => ("⚠ ", "badge badge--warn mb-3"),
        };
        let label = chain_status_label(t, &chain);
        let chain_banner_view = view! {
            <div class="row gap1-center">
                <p class=class>{prefix} {label}</p>
                {crate::components::help_details(
                    t,
                    t.help_about_chain_check.to_string(),
                    t.help_chain_check_body,
                    concat!(
                        crate::github_doc_url!("docs/src/guides/operators.md"),
                        "#audit-log-integrity",
                    ),
                )}
            </div>
        }
        .into_any();
        view! {
            <Shell title=t.audit_title.to_string() nav=Chrome::Console{username, item: NavItem::AuditLog} dev_mode=dev_mode lang=lang csrf_token=csrf_token.clone()>
                <header class="page-header">
                    <div>
                        <h1 class="page-header__title">{t.audit_title}</h1>
                        <p class="page-header__lede">
                            {t.audit_lede}
                            " "
                            {(t.audit_entry_count_caption)(total, per_page)}
                        </p>
                    </div>
                </header>
                {flash_banner(flash)}
                {chain_banner_view}
                <div class="filter-bar">
                    <form method="get" action="/admin/audit" class="row row-gap2-center">
                        <label for="audit-q" class="fw-500">{t.audit_filter_label}</label>
                        <input id="audit-q" name="q" type="search"
                               value=fq_display
                               aria-describedby="audit-q-hint"
                               class="min-w-16rem" />
                        <span id="audit-q-hint" class="field__hint">{t.audit_filter_hint}</span>
                        <button type="submit" class="secondary">{t.audit_filter_button}</button>
                    </form>
                    <a href=csv_href class="button secondary">{t.audit_export_csv}</a>
                </div>
                <div class="table-wrap table-wrap--scroll">
                    <table>
                        {audit_table_head(t)}
                        {if rows.is_empty() {
                            // No CTA for audit log — read-only surface.
                            view! {
                                <tbody><tr><td colspan=AUDIT_COLUMNS.to_string()>
                                    {empty_state(t.empty_audit, None)}
                                </td></tr></tbody>
                            }.into_any()
                        } else {
                            view! { <tbody>{rows}</tbody> }.into_any()
                        }}
                    </table>
                </div>
                {pager}
            </Shell>
        }
    })
}

fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
