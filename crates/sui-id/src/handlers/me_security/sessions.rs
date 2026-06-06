//! /me/security sessions tab handlers (RFC 068).

use crate::{csrf, errors::HttpError};
use axum::extract::{Form, Path, State};
use axum::http::header::LOCATION;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use std::str::FromStr;
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::SessionId;
use sui_id_store::repos::{audit, sessions};

const SESSION_COOKIE: &str = "sui_id_session";

use super::forms::*;
use crate::handlers::{AppStateExt, CurrentUser, enforce_csrf};
use crate::handlers::admin::with_csrf_cookie;
use sui_id_web::pages::{MeShellData, MeTab};
use super::describe_auth_methods;

pub async fn revoke_one(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<String>,
    jar: CookieJar,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;

    let target_id = SessionId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid session id".into())))?;

    // Ownership check: pulling the session row and comparing user_id
    // is the simplest correct way. Skipping this would let a user
    // revoke another user's session by guessing the id.
    let row = match sessions::get(&app.db, target_id).await {
        Ok(r) => r,
        Err(sui_id_store::StoreError::NotFound) => {
            // Treat unknown ids the same as foreign ids — both
            // produce a redirect back to the page, no leak.
            return Ok(Redirect::to("/me/security?msg=unknown").into_response());
        }
        Err(e) => return Err(HttpError::html(e.into())),
    };
    if row.user_id != user_id {
        return Ok(Redirect::to("/me/security?msg=unknown").into_response());
    }

    sessions::revoke(&app.db, target_id).await.map_err(|e| HttpError::html(e.into()))?;

    // If the user just revoked their *own* current session, clear
    // the cookie so the next request is clean. They'll bounce to
    // the login page on the redirect target.
    let raw_session = jar.get(SESSION_COOKIE).map(|c| c.value().to_owned());
    if raw_session.as_deref() == Some(target_id.to_string().as_str()) {
        let mut clear = axum_extra::extract::cookie::Cookie::from(SESSION_COOKIE);
        clear.set_path("/");
        clear.make_removal();
        return Ok((
            jar.remove(clear),
            Redirect::to("/admin/login?msg=session_revoked"),
        )
            .into_response());
    }

    Ok(Redirect::to("/me/security?msg=session_revoked").into_response())
}

/// Revoke every session for the current user *except* the one
/// issuing the request. The "keep" id comes from the cookie, not
/// the form field — the form field is decorative; if it disagrees
/// we honour the cookie.

pub async fn revoke_all_others(
    state_ext: AppStateExt,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Form(form): Form<RevokeAllOthersForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;

    // Step-up gate: signing every other browser out at once is a
    // significant action; require a fresh strong-factor proof.
    // Users without MFA enrolled are passed through (the gate
    // is a no-op for them — see step_up::policy_for_session
    // doc comment for why a password re-prompt would buy
    // nothing). After step-up the operator is bounced back to
    // /me/security and can click the form again.
    if let Err(redirect) = crate::handlers::require_fresh_step_up(
        &app,
        &ctx,
        "/me/security",
    ).await {
        return Ok(redirect);
    }

    let user_id = ctx.user_id;
    let keep = ctx.session_id;

    let n = sessions::revoke_all_for_user_except(&app.db, user_id, keep).await
        .map_err(|e| HttpError::html(e.into()))?;

    // Audit: emit one row capturing how many sessions were swept.
    // The action name follows the dotted naming used elsewhere.
    let _ = audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(user_id),
            action: "auth.sessions.bulk_revoke_self".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: Some(format!("revoked {n} other session(s)")),
        },
    ).await;

    let target = if n == 0 {
        "/me/security?msg=no_other_sessions"
    } else {
        "/me/security?msg=others_revoked"
    };
    let mut resp = Response::default();
    *resp.status_mut() = axum::http::StatusCode::SEE_OTHER;
    resp.headers_mut().insert(
        LOCATION,
        target.parse().expect("static header value"),
    );
    Ok(resp)
}

// ---------- helpers ----------


pub async fn sessions_tab_get(
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
        active_tab: MeTab::Sessions,
    };

    let raw_session = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_owned())
        .unwrap_or_default();
    let current_session_id = raw_session;

    let session_rows = sessions::list_active_for_user(&app.db, user_id)
        .await.map_err(|e| HttpError::html(e.into()).with_lang(lang))?;

    let sessions_view: Vec<sui_id_web::MeSessionDescriptor> = session_rows.into_iter().map(|s| {
        let auth_methods = describe_auth_methods(&s.auth_methods);
        let is_current = s.id.to_string() == current_session_id;
        sui_id_web::MeSessionDescriptor {
            id: s.id.to_string(),
            created_at: s.created_at,
            expires_at: s.expires_at,
            auth_methods,
            is_current,
        }
    }).collect();

    let csrf_tok = csrf::ensure_token(&jar);
    let resp = axum::response::Html(sui_id_web::render_me_sessions(
        sui_id_web::MeSessionsData {
            shell,
            current_session_id,
            sessions: sessions_view,
            csrf_token: csrf_tok.clone(),
        },
        None, app.is_dev_mode, lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}
