//! Admin handlers for audit (RFC 066).

use super::with_csrf_cookie;
use crate::errors::HttpError;
use crate::handlers::{AppStateExt, CurrentAdminOrAuditor};
use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use sui_id_core::errors::CoreError;
use sui_id_shared::api::AuditLogEntryDto;
use sui_id_store::repos::audit;
use sui_id_web::render_audit;

#[derive(Debug, serde::Deserialize, Default)]

pub struct AuditQuery {
    #[serde(default)]
    pub q: String,
}

pub async fn audit_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_, _role, _): CurrentAdminOrAuditor,
    jar: CookieJar,
    axum::extract::Query(query): axum::extract::Query<AuditQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let filter = if query.q.is_empty() {
        None
    } else {
        Some(query.q.clone())
    };
    let entries = audit::recent_filtered(&app.db, 200, filter.clone())
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let chain = audit::verify_chain_tail(&app.db, 500).await.unwrap_or(
        sui_id_store::repos::audit::ChainVerifyReport {
            checked: 0,
            broken_at_seq: None,
            legacy_unhashed: 0,
        },
    );
    let chain_ok = chain.broken_at_seq.is_none();
    let dtos: Vec<AuditLogEntryDto> = entries
        .into_iter()
        .map(|r| AuditLogEntryDto {
            at: r.at,
            actor: r.actor,
            action: r.action,
            target: r.target,
            result: r.result,
            note: r.note,
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_audit(
        dtos,
        chain_ok,
        filter,
        None,
        token.clone(),
        app.is_dev_mode,
        sui_id_i18n::Locale::Ja,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn audit_csv_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_, _role, _): CurrentAdminOrAuditor,
    axum::extract::Query(query): axum::extract::Query<AuditQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let filter = if query.q.is_empty() {
        None
    } else {
        Some(query.q.clone())
    };
    let entries = audit::recent_filtered(&app.db, 2000, filter)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    let mut csv = String::from("when,actor,action,target,result,note\n");
    for r in entries {
        fn esc(s: &str) -> String {
            format!("\"{}\"", s.replace('"', "\"\"\""))
        }
        let actor_str = r.actor.map(|id| id.to_string()).unwrap_or_default();
        let target_str = r.target.unwrap_or_default();
        let note_str = r.note.unwrap_or_default();
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            r.at.to_rfc3339(),
            esc(&actor_str),
            esc(&r.action),
            esc(&target_str),
            esc(&r.result),
            esc(&note_str),
        ));
    }
    let mut resp = axum::response::Response::new(axum::body::Body::from(csv));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        axum::http::HeaderValue::from_static("attachment; filename=audit.csv"),
    );
    Ok(resp)
}
