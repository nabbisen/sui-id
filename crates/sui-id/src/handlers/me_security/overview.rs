//! /me/security overview tab handlers (RFC 068).

use crate::{csrf, errors::HttpError};
use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use std::str::FromStr;
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::SessionId;
use sui_id_store::repos::{audit, sessions, user_totp, users};

const SESSION_COOKIE: &str = "sui_id_session";
const RECENT_EVENT_LIMIT: i64 = 30;

use crate::handlers::{AppStateExt, CurrentUser};
use crate::handlers::admin::with_csrf_cookie;
use sui_id_web::pages::{MeShellData, MeTab, MeOverviewData};
use super::{describe_auth_methods, flash_from_query};

pub async fn page_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    // The `CurrentUser` extractor has already resolved the cookie,
    // but we need the raw cookie value to identify *which* session
    // is the one issuing this request. There can be more than one
    // active session for the same user, and we want to mark the
    // current row as "current".
    let raw_session = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
    let current_session_id = SessionId::from_str(&raw_session)
        .map_err(|_| HttpError::html(CoreError::Unauthenticated))?;

    let user = users::get(&app.db, user_id).await.map_err(|e| HttpError::html(CoreError::from(e)))?;

    let totp_enabled = user_totp::get(&app.db, user_id).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?
        .map(|r| r.enabled)
        .unwrap_or(false);

    let passkey_count = sui_id_core::webauthn::list_for_user(&app.db, user_id).await
        .map_err(HttpError::html)?
        .len();

    let session_rows =
        sessions::list_active_for_user(&app.db, user_id).await.map_err(|e| HttpError::html(e.into()))?;
    let mut sessions_view = Vec::with_capacity(session_rows.len());
    for s in session_rows {
        let auth_methods = describe_auth_methods(&s.auth_methods);
        let is_current = s.id == current_session_id;
        sessions_view.push(sui_id_web::MeSessionDescriptor {
            id: s.id.to_string(),
            created_at: s.created_at,
            expires_at: s.expires_at,
            auth_methods,
            is_current,
        });
    }

    let event_rows = audit::recent_for_user(&app.db, user_id, RECENT_EVENT_LIMIT).await
        .map_err(|e| HttpError::html(e.into()))?;
    let events_view: Vec<_> = event_rows
        .into_iter()
        .map(|e| sui_id_web::MeAuditEntry {
            at: e.at,
            action: e.action,
            result: e.result,
            note: e.note,
        })
        .collect();

    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_me_security(
        sui_id_web::MeSecurityData {
            username: user.username,
            is_admin: user.is_admin,
            totp_enabled,
            passkey_count,
            current_session_id: current_session_id.to_string(),
            sessions: sessions_view,
            recent_events: events_view,
        },
        flash_from_query(&jar),
        token.clone(),
        lang,
    );
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// Revoke a single session belonging to the current user.
///
/// The `id` path parameter is parsed and then cross-checked against
/// the database: the row must exist *and* belong to the current
/// user. We don't tell the caller which check failed — both refuse
/// with the same 404-shaped redirect, so there's no oracle for
/// guessing other people's session ids.

pub async fn overview_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let lang = req_locale;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let shell = MeShellData {
        username: user.username,
        is_admin: user.is_admin,
        active_tab: MeTab::Overview,
    };
    let totp_enabled = user_totp::get(&app.db, user_id)
        .await.ok().flatten()
        .map(|r| r.enabled).unwrap_or(false);
    let passkey_count = sui_id_store::repos::user_webauthn_credentials::count_for_user(
        &app.db, user_id
    ).await.unwrap_or(0);
    let active_session_count = sessions::list_active_for_user(&app.db, user_id)
        .await.map(|v| v.len()).unwrap_or(0);
    let recent_events: Vec<sui_id_web::MeAuditEntry> =
        audit::recent_for_user(&app.db, user_id, 10)
        .await.unwrap_or_default()
        .into_iter()
        .map(|r| sui_id_web::MeAuditEntry {
            at: r.at,
            action: r.action,
            result: r.result,
            note: r.note,
        })
        .collect();
    let csrf_tok = csrf::ensure_token(&jar);
    let resp = axum::response::Html(sui_id_web::render_me_overview(
        MeOverviewData { shell, totp_enabled, passkey_count, active_session_count, recent_events, csrf_token: csrf_tok.clone() },
        app.is_dev_mode, lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}
