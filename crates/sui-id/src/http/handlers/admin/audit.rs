//! Admin handlers for audit (RFC 066, RFC 149 Q5: paging and the CSV
//! export).

use super::with_csrf_cookie;
use crate::errors::HttpError;
use crate::handlers::{AppStateExt, CurrentAdminOrAuditor};
use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use bytes::Bytes;
use futures_util::StreamExt;
use sui_id_core::errors::CoreError;
use sui_id_shared::api::AuditLogEntryDto;
use sui_id_store::db::Database;
use sui_id_store::repos::audit;
use sui_id_web::render_audit;

/// RFC 149 Q5 §2: the audit log pages 50 at a time, newest first,
/// keyset-paged by `seq` rather than `OFFSET` — new rows arrive
/// constantly, which would shift every `OFFSET` page under the
/// reader, and a deep `OFFSET` scans every row before it.
const PAGE_SIZE: i64 = 50;

#[derive(Debug, serde::Deserialize, Default)]
pub struct AuditQuery {
    #[serde(default)]
    pub q: String,
    /// The "older" cursor: show rows with `seq` strictly less than
    /// this. Mutually exclusive with `after` — see
    /// [`audit::page_filtered`].
    pub before: Option<i64>,
    /// The "newer" cursor: show rows with `seq` strictly greater
    /// than this.
    pub after: Option<i64>,
}

fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Build the `href` for an "older"/"newer" link, keeping the filter
/// across pages (RFC 149 Q5 §2/§5 test 5 — the filter must not drop
/// off the `before`/`after` link).
fn audit_page_href(filter: Option<&str>, before: Option<i64>, after: Option<i64>) -> String {
    let mut qs = Vec::new();
    if let Some(f) = filter.filter(|f| !f.is_empty()) {
        qs.push(format!("q={}", url_encode(f)));
    }
    if let Some(b) = before {
        qs.push(format!("before={b}"));
    }
    if let Some(a) = after {
        qs.push(format!("after={a}"));
    }
    if qs.is_empty() {
        "/admin/audit".to_owned()
    } else {
        format!("/admin/audit?{}", qs.join("&"))
    }
}

pub async fn audit_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_, _role, _, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::extract::Query(query): axum::extract::Query<AuditQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let filter = if query.q.is_empty() {
        None
    } else {
        Some(query.q.clone())
    };
    // RFC 149 Q5 §1/§3: the caption counts every matching row, not
    // the ones this one page happens to render.
    let total = audit::count_filtered(&app.db, filter.clone())
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let entries = audit::page_filtered(
        &app.db,
        filter.clone(),
        query.before,
        query.after,
        PAGE_SIZE,
    )
    .await
    .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let newer_href = (query.before.is_some() || query.after.is_some())
        .then(|| {
            entries
                .first()
                .map(|r| audit_page_href(filter.as_deref(), None, Some(r.seq)))
        })
        .flatten();
    let older_href = entries
        .last()
        .map(|r| audit_page_href(filter.as_deref(), Some(r.seq), None));
    // RFC 121: the one shared check, not a re-decision of what an error means.
    let chain = crate::handlers::chain_status(&app, 500).await;
    let dtos: Vec<AuditLogEntryDto> = entries
        .into_iter()
        .map(|r| AuditLogEntryDto {
            at: r.entry.at,
            actor: r.entry.actor,
            action: r.entry.action,
            target: r.entry.target,
            result: r.entry.result,
            note: r.entry.note,
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_audit(
        dtos,
        total,
        PAGE_SIZE as usize,
        chain,
        filter,
        newer_href,
        older_href,
        None,
        token.clone(),
        app.is_dev_mode,
        lang,
        username,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// One CSV field, quoted as RFC 4180 requires: a `"` inside is doubled.
///
/// **And neutralised against formula injection** (CWE-1236): a field that
/// begins with `=`, `+`, `-`, `@`, a tab or a carriage return is prefixed
/// with `'`, so a spreadsheet shows it as text instead of evaluating it.
/// The audit log holds text an unauthenticated visitor chose: a failed
/// sign-in records the submitted username as its target.
fn csv_escape(s: &str) -> String {
    let neutralised = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_owned()
    };
    format!("\"{}\"", neutralised.replace('"', "\"\""))
}

fn csv_line(e: &sui_id_store::models::AuditLogRow) -> String {
    let actor_str = e.actor.map(|id| id.to_string()).unwrap_or_default();
    let target_str = e.target.as_deref().unwrap_or_default();
    let note_str = e.note.as_deref().unwrap_or_default();
    format!(
        "{},{},{},{},{},{}\n",
        e.at.to_rfc3339(),
        csv_escape(&actor_str),
        csv_escape(&e.action),
        csv_escape(target_str),
        csv_escape(&e.result),
        csv_escape(note_str),
    )
}

/// RFC 149 Q5 §2: the CSV export streams every matching row, in pages
/// keyset-paged by `seq` the same way the HTML view is, rather than
/// materialising the whole export (once capped at 2,000 rows with no
/// statement that it was a cap) in memory. Each `unfold` step holds
/// only one page's own rows at a time — the server's peak memory for
/// this response stays flat regardless of how many rows match.
async fn csv_body_stream(
    db: Database,
    filter: Option<String>,
) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>> {
    /// A page at a time: `None` cursor means "not started"; `Some(None)`
    /// means "the last page came back short — done".
    enum Cursor {
        Start,
        Before(i64),
        Done,
    }
    futures_util::stream::unfold(Cursor::Start, move |cursor| {
        let db = db.clone();
        let filter = filter.clone();
        async move {
            let before = match cursor {
                Cursor::Start => None,
                Cursor::Before(seq) => Some(seq),
                Cursor::Done => return None,
            };
            const CSV_PAGE_SIZE: i64 = 1000;
            let page = audit::page_filtered(&db, filter.clone(), before, None, CSV_PAGE_SIZE).await;
            match page {
                Ok(rows) if rows.is_empty() => None,
                Ok(rows) => {
                    let next = match rows.last() {
                        Some(last) if (rows.len() as i64) >= CSV_PAGE_SIZE => {
                            Cursor::Before(last.seq)
                        }
                        _ => Cursor::Done,
                    };
                    let mut chunk = String::new();
                    for r in &rows {
                        chunk.push_str(&csv_line(&r.entry));
                    }
                    Some((Ok(Bytes::from(chunk)), next))
                }
                Err(e) => {
                    tracing::error!(error = %e, "audit CSV export: page read failed");
                    Some((
                        Err(std::io::Error::other("audit export read failed")),
                        Cursor::Done,
                    ))
                }
            }
        }
    })
}

pub async fn audit_csv_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_, _role, _, _): CurrentAdminOrAuditor,
    axum::extract::Query(query): axum::extract::Query<AuditQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let filter = if query.q.is_empty() {
        None
    } else {
        Some(query.q.clone())
    };
    // RFC 149 Q5 §2: the export says when it was made, and whether a
    // filter narrowed it, in its filename. Its first line stays the
    // header row, so a CSV reader parses it as written (a comment line
    // first would be read as the header).
    let header_line = String::from("when,actor,action,target,result,note\n");
    let filename = format!(
        "attachment; filename=\"audit-{}{}.csv\"",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        if filter.is_some() { "-filtered" } else { "" },
    );
    let body_stream = csv_body_stream(app.db.clone(), filter).await;
    let stream =
        futures_util::stream::once(async move { Ok(Bytes::from(header_line)) }).chain(body_stream);
    let mut resp = axum::response::Response::new(axum::body::Body::from_stream(stream));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        // Digits and fixed ASCII only, so this cannot fail; the fallback
        // keeps the header valid without an `expect`.
        axum::http::HeaderValue::from_str(&filename).unwrap_or_else(|_| {
            axum::http::HeaderValue::from_static("attachment; filename=\"audit.csv\"")
        }),
    );
    Ok(resp)
}
