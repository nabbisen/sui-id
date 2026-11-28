//! Pagination (RFC 149 Q5 §1/§2): "Showing X–Y of Z", and Previous/Next
//! links that work without JavaScript — every link is a plain `<a>`
//! to a `?page=N`/`?before=<seq>`/`?after=<seq>` URL, nothing a script
//! has to wire up.
//!
//! Two shapes, not one, because the two paging strategies this stage
//! uses don't share a cursor type: [`offset_pager_view`] (users, apps)
//! knows its absolute page number and can jump to any page; the audit
//! log's own [`seq_pager_view`] only knows "older than my last row" /
//! "newer than my first row" (RFC 149 Q5 §2's own reason: `OFFSET`
//! paging on a table new rows constantly arrive at would shift every
//! page under the reader, and a deep `OFFSET` scans every row before
//! it). Forcing both through one shape would mean one of them lying
//! about what it actually knows.

use leptos::prelude::*;

/// An offset-paged list's own page info — users, apps.
pub struct OffsetPageInfo {
    /// 1-based.
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
}

impl OffsetPageInfo {
    /// 1-based. `total <= 0` still returns `1` — a last page always
    /// exists to land a past-the-end request on, even on an empty list.
    pub fn last_page(&self) -> u32 {
        let per_page = i64::from(self.per_page.max(1));
        u32::try_from((self.total.max(1) - 1) / per_page).unwrap_or(0) + 1
    }

    /// The 1-based `(from, to)` ordinals this page covers. `(0, 0)` if
    /// the list is empty.
    pub fn range(&self) -> (i64, i64) {
        if self.total <= 0 {
            return (0, 0);
        }
        let per_page = i64::from(self.per_page.max(1));
        let from = i64::from(self.page.saturating_sub(1)) * per_page + 1;
        let to = (from + per_page - 1).min(self.total);
        (from, to)
    }
}

/// `base_url` is the path plus any filter query string, with no
/// trailing `?`/`&` and no `page` parameter of its own — e.g.
/// `/admin/users` — this appends `?page=N`/`&page=N` itself.
pub fn offset_pager_view(
    t: &'static sui_id_i18n::Strings,
    base_url: &str,
    info: &OffsetPageInfo,
) -> impl IntoView + use<> {
    let last = info.last_page();
    let sep = if base_url.contains('?') { "&" } else { "?" };
    let prev = (info.page > 1).then(|| format!("{base_url}{sep}page={}", info.page - 1));
    let next = (info.page < last).then(|| format!("{base_url}{sep}page={}", info.page + 1));
    view! {
        <nav class="pager" aria-label="Pagination">
            {prev.map(|href| view! { <a href=href class="button secondary">{t.pager_previous}</a> })}
            {next.map(|href| view! { <a href=href class="button secondary">{t.pager_next}</a> })}
        </nav>
    }
}

/// The audit log's own seq-cursor nav. `older_href`/`newer_href` are
/// `None` exactly when there is nothing further in that direction —
/// the caller already knows this from whether the page it read came
/// back shorter than a full page (older) or empty (newer), so this
/// function only renders what it's given, the same division of work
/// [`offset_pager_view`]'s own caller keeps.
pub fn seq_pager_view(
    t: &'static sui_id_i18n::Strings,
    newer_href: Option<String>,
    older_href: Option<String>,
) -> impl IntoView + use<> {
    view! {
        <nav class="pager" aria-label="Pagination">
            {newer_href.map(|href| view! { <a href=href class="button secondary">{t.pager_previous}</a> })}
            {older_href.map(|href| view! { <a href=href class="button secondary">{t.pager_next}</a> })}
        </nav>
    }
}

pub const PAGER_CSS: &str = r#"
/* ------------------------------------------------------------------ */
/* Pagination (RFC 149 Q5)                                            */
/* ------------------------------------------------------------------ */
.pager {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-3);
}
"#;
