//! Forgot-password / reset-password endpoints.
//!
//! User-facing flow:
//!
//!   GET  /forgot-password           — render request form
//!   POST /forgot-password           — issue token + send email,
//!                                       always 200 + neutral
//!                                       message (user-enumeration
//!                                       neutral)
//!   GET  /reset-password            — render new-password form; the token
//!                                       arrives in the link's fragment
//!                                       (`#t=<token>`) or is pasted
//!   POST /reset-password            — verify token, set new password, redirect
//!
//! RFC 103 D10: the token never travels in a URL the server sees. A
//! `GET /reset-password?token=…` (an old-format link) is not processed; it
//! renders the "request a new link" page.
//!
//! All four are unauthenticated (no session required); the second
//! pair is gated by token possession.
//!
//! ## Feature gating
//!
//! When SMTP is unconfigured / disabled, all four endpoints return
//! 404. The forgot-password form simply does not exist in that
//! deployment. This avoids the awkward mid-state where the form
//! renders but submitting it silently no-ops.

use crate::errors::HttpError;
use crate::handlers::{AppStateExt, ClientIp};
use crate::{csrf, handlers::admin::with_csrf_cookie};
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sui_id_core::errors::CoreError;
use sui_id_web::{Flash, FlashKind};

async fn smtp_active(db: &sui_id_store::Database) -> Result<bool, HttpError> {
    let cfg = sui_id_store::repos::smtp_config::get(db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    Ok(cfg.map(|c| c.enabled).unwrap_or(false))
}

fn smtp_required_or_404(active: bool) -> Result<(), HttpError> {
    if !active {
        Err(HttpError::not_found_html())
    } else {
        Ok(())
    }
}

// ---------- GET /forgot-password ----------

pub async fn forgot_password_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    smtp_required_or_404(smtp_active(&app.db).await?)?;
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_forgot_password(token.clone(), None, lang);
    Ok(with_csrf_cookie(Html(html).into_response(), &app, &token))
}

// ---------- POST /forgot-password ----------

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub email: String,
}

pub async fn forgot_password_post(
    state_ext: AppStateExt,
    ClientIp(ip): ClientIp,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::Form(form): axum::Form<ForgotPasswordForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    smtp_required_or_404(smtp_active(&app.db).await?)?;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::ForgotPassword,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;

    // Best-effort. Internal failures audit-logged inside.
    let ip_str = ip.to_string();
    let _ = sui_id_core::forgot_password::request_reset(
        &app.db,
        &app.clock,
        app.mailer.as_ref(),
        app.issuer(),
        &form.email,
        Some(&ip_str),
    )
    .await;

    // Always return the same neutral acknowledgement.
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_forgot_password_sent(lang);
    Ok(with_csrf_cookie(Html(html).into_response(), &app, &token))
}

// ---------- GET /reset-password ----------

#[derive(Debug, Deserialize)]
pub struct ResetTokenQuery {
    #[serde(default)]
    pub token: SecretString,
}

pub async fn reset_password_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Query(q): Query<ResetTokenQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // RFC 103: completion needs no mail. An administrator- or operator-issued
    // link must be redeemable on an instance with no SMTP configured, and
    // with SMTP off no email-origin token can exist (`/forgot-password` keeps
    // its gate, because it sends mail).
    let token = csrf::ensure_token(&jar);
    // RFC 103 D10: a token in the query string has already reached every
    // access log on the way here. Do not process it; ask for a new link.
    if !q.token.expose_secret().is_empty() {
        let html = sui_id_web::render_reset_password_invalid(lang);
        return Ok(with_csrf_cookie(Html(html).into_response(), &app, &token));
    }
    let html = sui_id_web::render_reset_password(String::new(), token.clone(), None, lang);
    Ok(with_csrf_cookie(Html(html).into_response(), &app, &token))
}

// ---------- POST /reset-password ----------

#[derive(Debug, Deserialize)]
pub struct ResetPasswordForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub token: SecretString,
    pub password: SecretString,
    pub confirm_password: SecretString,
}

pub async fn reset_password_post(
    state_ext: AppStateExt,
    ClientIp(ip): ClientIp,
    request_id: Option<axum::Extension<crate::request_id::RequestId>>,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::Form(form): axum::Form<ResetPasswordForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // No SMTP requirement here either: see `reset_password_get`.
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let t = lang.strings();

    if form.password.expose_secret() != form.confirm_password.expose_secret() {
        let token = csrf::ensure_token(&jar);
        let flash = Flash {
            kind: FlashKind::Warn,
            text: t.password_mismatch_flash.into(),
        };
        let html = sui_id_web::render_reset_password(
            form.token.expose_secret().to_owned(),
            token.clone(),
            Some(flash),
            lang,
        );
        return Ok(with_csrf_cookie(
            (axum::http::StatusCode::BAD_REQUEST, Html(html)).into_response(),
            &app,
            &token,
        ));
    }

    // RFC 003: load HIBP settings for this request.
    let hibp_mode = sui_id_store::repos::server_settings::get(&app.db)
        .await
        .map(|s| s.hibp_mode)
        .unwrap_or_default();

    let ip_str = ip.to_string();
    match sui_id_core::forgot_password::consume_and_reset_password(
        &app.db,
        &app.clock,
        app.mailer.as_ref(),
        Some(app.hibp_client.as_ref()),
        hibp_mode,
        form.token.expose_secret(),
        form.password.expose_secret(),
        Some(&ip_str),
        crate::handlers::password_min_len(&app),
    )
    .await
    {
        Ok(()) => Ok(Redirect::to("/admin/login?reset=ok").into_response()),
        // Password-policy and breach refusals happen before the token is
        // looked up, so the link is still good: re-show the form with the
        // reason and let the user choose another password.
        Err(other @ CoreError::BadRequest(_)) => {
            let token = csrf::ensure_token(&jar);
            let flash = Flash {
                kind: FlashKind::Warn,
                text: friendly(&other, lang),
            };
            let html = sui_id_web::render_reset_password(
                form.token.expose_secret().to_owned(),
                token.clone(),
                Some(flash),
                lang,
            );
            Ok(with_csrf_cookie(
                (axum::http::StatusCode::BAD_REQUEST, Html(html)).into_response(),
                &app,
                &token,
            ))
        }
        // RFC 103 D13: every other completion failure — unknown, used or
        // expired token, ineligible user, storage error — gets the same
        // invalid-link response. The cause is logged; the token is not.
        // An unusable link is an ordinary outcome anyone can trigger and
        // repeat, so it is logged at info; a storage or internal failure
        // stays at error.
        Err(err) => {
            let request_id = request_id.as_ref().map(|e| e.0.0.as_str()).unwrap_or("-");
            if matches!(err, CoreError::InvalidCredentials) {
                tracing::info!(
                    request_id,
                    "password reset completion refused: the link is unknown, used or expired"
                );
            } else {
                tracing::error!(
                    request_id,
                    error = %err,
                    detail = ?err,
                    "password reset completion refused"
                );
            }
            let html = sui_id_web::render_reset_password_invalid(lang);
            Ok((axum::http::StatusCode::BAD_REQUEST, Html(html)).into_response())
        }
    }
}

fn friendly(e: &CoreError, lang: sui_id_i18n::Locale) -> String {
    let t = lang.strings();
    match e {
        CoreError::BadRequest(msg) => msg.clone(),
        _ => t.reset_password_failed_flash.into(),
    }
}
