//! Admin handlers for auth (RFC 066).

use crate::errors::HttpError;
use crate::handlers::{
    clear_pending_mfa_cookie, clear_pending_mfa_next_cookie, clear_session_cookie,
    pending_mfa_cookie, pending_mfa_next_cookie, session_cookie, AppStateExt, PENDING_MFA_COOKIE, PENDING_MFA_NEXT_COOKIE, SESSION_COOKIE,
};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use std::str::FromStr;
use sui_id_core::session;
use sui_id_shared::ids::SessionId;
use sui_id_web::{
    render_login, Flash, FlashKind,
};
use super::forms::CsrfOnlyForm;
use super::with_csrf_cookie;

#[derive(Debug, Deserialize)]

pub struct LoginForm {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub next: String,
}


/// Query parameters accepted by `GET /admin/login`.
/// The `next` value is URL-encoded by the `/oauth2/authorize` endpoint
/// so the login form can redirect back after a successful sign-in.
#[derive(Debug, Deserialize, Default)]
pub struct LoginGetQuery {
    #[serde(default)]
    pub next: String,
}


pub async fn login_get(
    jar: CookieJar,
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    Query(q): Query<LoginGetQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Already logged in? Forward to `next` if present, otherwise /admin.
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        if let Ok(sid) = SessionId::from_str(cookie.value()) {
            if session::resolve(&app.db, &app.clock, sid).await.is_ok() {
                let dest = if q.next.starts_with('/') {
                    q.next.clone()
                } else {
                    "/admin".into()
                };
                return Ok(Redirect::to(&dest).into_response());
            }
        }
    }
    // Thread `next` into the form so login_post can redirect to it.
    let next = if q.next.is_empty() { None } else { Some(q.next) };
    Ok(Html(render_login(None, next, lang, false)).into_response())
}


pub async fn login_post(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Login,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;
    match session::login_with_mfa(
        &app.db,
        &app.clock,
        form.username.trim(),
        &form.password,
        app.config.security.max_lockout.as_secs(),
    ).await {
        Ok(session::LoginOutcome::SessionEstablished(row)) => {
            let cookie = session_cookie(row.id.to_string(), app.config.server.cookie_secure);
            let jar = jar.add(cookie);
            let target = if form.next.starts_with('/') {
                form.next.clone()
            } else {
                "/admin".into()
            };
            Ok((jar, Redirect::to(&target)).into_response())
        }
        Ok(session::LoginOutcome::MfaRequired { pending }) => {
            // Drop the user a short-lived cookie pointing at the
            // pending row, and bounce them into the MFA challenge page.
            let cookie = pending_mfa_cookie(
                pending.id.to_string(),
                app.config.server.cookie_secure,
            );
            let next_cookie = if !form.next.is_empty() {
                Some(pending_mfa_next_cookie(
                    form.next.clone(),
                    app.config.server.cookie_secure,
                ))
            } else {
                None
            };
            let jar = jar.add(cookie);
            let jar = match next_cookie {
                Some(c) => jar.add(c),
                None => jar,
            };
            Ok((jar, Redirect::to("/admin/login/mfa")).into_response())
        }
        Err(_) => {
            let flash = Flash {
                kind: FlashKind::Error,
                text: "Sign-in failed. Check your username and password.".into(),
            };
            let next = if form.next.is_empty() {
                None
            } else {
                Some(form.next)
            };
            Ok(
                (StatusCode::UNAUTHORIZED, Html(render_login(Some(flash), next, lang, false)))
                    .into_response(),
            )
        }
    }
}


pub async fn mfa_challenge_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Best-effort: if we can resolve the pending-MFA row, look up
    // whether the user has any passkeys so the page can offer that
    // path. If we can't (cookie missing or row gone), default to
    // hiding the passkey button — the user can still type a TOTP code.
    let has_passkey = {
        let pid_opt = jar
            .get(crate::handlers::PENDING_MFA_COOKIE)
            .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok());
        if let Some(pid) = pid_opt {
            let row_opt = sui_id_store::repos::login_pending_mfa::get(&app.db, pid)
                .await.ok().flatten();
            if let Some(row) = row_opt {
                sui_id_core::webauthn::has_credentials(&app.db, row.user_id).await.unwrap_or(false)
            } else { false }
        } else { false }
    };
    let token = crate::csrf::ensure_token(&jar);
    let resp =
        Html(sui_id_web::render_mfa_challenge(None, token.clone(), has_passkey, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]

pub struct MfaChallengeForm {
    pub code: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}


pub async fn mfa_challenge_post(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<MfaChallengeForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Same rate-limit bucket as password attempts: a user who is past
    // the password step still uses a single login budget.
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Login,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let pending_value = match jar.get(PENDING_MFA_COOKIE) {
        Some(c) => c.value().to_owned(),
        None => {
            return Ok(Redirect::to("/admin/login").into_response());
        }
    };
    let pending_id = match pending_value.parse::<sui_id_shared::ids::PendingMfaId>() {
        Ok(id) => id,
        Err(_) => return Ok(Redirect::to("/admin/login").into_response()),
    };
    match sui_id_core::mfa::verify_pending(&app.db, &app.clock, pending_id, &form.code).await {
        Ok(session) => {
            let cookie =
                session_cookie(session.id.to_string(), app.config.server.cookie_secure);
            // Compose the redirect target from the optional next cookie.
            let next_target = jar
                .get(PENDING_MFA_NEXT_COOKIE)
                .map(|c| c.value().to_owned())
                .filter(|s| s.starts_with('/'))
                .unwrap_or_else(|| "/admin".into());
            // Audit the MFA success.
            let _ = sui_id_store::repos::audit::append(
                &app.db,
                &sui_id_store::models::AuditLogRow {
                    at: app.clock.now(),
                    actor: Some(session.user_id),
                    action: "auth.mfa.success".into(),
                    target: Some(session.user_id.to_string()),
                    result: "ok".into(),
                    note: None,
                },
            ).await;
            let jar = jar
                .add(cookie)
                .add(clear_pending_mfa_cookie(app.config.server.cookie_secure))
                .add(clear_pending_mfa_next_cookie(app.config.server.cookie_secure));
            Ok((jar, Redirect::to(&next_target)).into_response())
        }
        Err(_) => {
            let t = lang.strings();
            let flash = Flash {
                kind: FlashKind::Error,
                text: t.mfa_challenge_failed_flash.into(),
            };
            let _ = sui_id_store::repos::audit::append(
                &app.db,
                &sui_id_store::models::AuditLogRow {
                    at: app.clock.now(),
                    actor: None,
                    action: "auth.mfa.failure".into(),
                    target: None,
                    result: "denied".into(),
                    note: None,
                },
            ).await;
            let has_passkey = {
                let pid_opt2 = jar
                    .get(crate::handlers::PENDING_MFA_COOKIE)
                    .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok());
                if let Some(pid) = pid_opt2 {
                    let row_opt2 = sui_id_store::repos::login_pending_mfa::get(&app.db, pid)
                        .await.ok().flatten();
                    if let Some(row) = row_opt2 {
                        sui_id_core::webauthn::has_credentials(&app.db, row.user_id).await.unwrap_or(false)
                    } else { false }
                } else { false }
            };
            let token = crate::csrf::ensure_token(&jar);
            let resp = (
                StatusCode::UNAUTHORIZED,
                Html(sui_id_web::render_mfa_challenge(
                    Some(flash),
                    token.clone(),
                    has_passkey,
                    lang,
                )),
            )
                .into_response();
            Ok(with_csrf_cookie(resp, &app, &token))
        }
    }
}


pub async fn logout(
    jar: CookieJar,
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    axum::Form(form): axum::Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Validate CSRF to prevent logout-CSRF attacks.
    // On failure, redirect to login rather than returning 403 —
    // a stale CSRF token (e.g. after a previous logout) should not
    // leave the operator staring at an error page.
    if crate::handlers::enforce_csrf(&jar, Some(&form.csrf)).is_err() {
        return Ok(Redirect::to("/admin/login").into_response());
    }
    if let Some(c) = jar.get(SESSION_COOKIE) {
        if let Ok(sid) = SessionId::from_str(c.value()) {
            let _ = session::logout(&app.db, sid).await;
        }
    }
    let jar = jar.add(clear_session_cookie(app.config.server.cookie_secure));
    // Render the login page with a "Signed out" confirmation.
    let flash = Flash {
        kind: FlashKind::Info,
        text: lang.strings().signed_out_flash.into(),
    };
    Ok((
        jar,
        Html(render_login(Some(flash), None, lang, false)),
    )
        .into_response())
}
