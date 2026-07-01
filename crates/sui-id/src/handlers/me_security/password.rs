//! /me/security password tab handlers (RFC 068).

use crate::{csrf, errors::HttpError};
use axum::extract::{Form, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use std::str::FromStr;
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::SessionId;
use sui_id_store::repos::users;

const SESSION_COOKIE: &str = "sui_id_session";

use super::forms::*;
use crate::handlers::{AppStateExt, CurrentUser, SessionContext};
use sui_id_core::actor::SelfActor;
use crate::handlers::admin::with_csrf_cookie;

pub async fn password_change_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let user = users::get(&app.db, user_id).await.map_err(|e| HttpError::html(CoreError::from(e)))?;
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_password_change(
        sui_id_web::PasswordChangeData {
            username: user.username,
            revoke_others_default: true,
        },
        None,
        token.clone(),
        lang,
    );
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}


pub async fn password_change_post(
    state_ext: AppStateExt,
    SessionContext { user_id, session_id: _session_id }: SessionContext,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    crate::handlers::RequestLocale(_lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<PasswordChangeForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    // Order of checks:
    //
    // 1. CSRF first — cheap and protects every other check from
    //    being driven from a hostile origin.
    // 2. Rate limit second, sharing the Login bucket. Even though
    //    the caller already has a valid session, we don't want
    //    someone with a stolen cookie to grind the
    //    `current_password` field at unbounded rate.
    // 3. UI-level mismatch check (new vs confirm) before we go to
    //    the database — pure form ergonomics.
    // 4. The actual password change in `core::me_security`, which
    //    verifies `current_password` against the stored hash and
    //    enforces the policy on the new one.
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Login,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;

    if form.new_password != form.confirm_password {
        return Err(HttpError::html(CoreError::BadRequest(
            "new password and confirmation do not match".into(),
        )));
    }

    let revoke_others = form.revoke_others.is_some();

    // Pull the current session id from the cookie so we can keep
    // it alive across the sweep. If the cookie isn't there we'd
    // have failed `CurrentUser` already, but be explicit.
    let raw_session = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
    let keep = SessionId::from_str(&raw_session)
        .map_err(|_| HttpError::html(CoreError::Unauthenticated))?;

    // RFC 003: load HIBP settings from the DB for this request.
    let hibp_mode = sui_id_store::repos::server_settings::get(&app.db).await
        .map(|s| s.hibp_mode)
        .unwrap_or_default();

    // RFC 081: SelfActor scopes the call to this user only.
    let self_actor = sui_id_core::actor::Actor::from_session(
        user_id,
        // Role doesn't matter for SelfActor; User is a safe floor.
        sui_id_store::models::Role::User,
        keep,
    )
    .into_self();

    let report = sui_id_core::me_security::change_password_self(
        &app.db,
        &app.clock,
        Some(app.hibp_client.as_ref()),
        hibp_mode,
        &self_actor,
        &form.current_password,
        &form.new_password,
        Some(keep),
        revoke_others,
        crate::handlers::password_min_len(&app),
    ).await
    .map_err(HttpError::html)?;

    let _ = report; // counts are in the audit event already; nothing to surface

    // Post-change notification mail (best-effort). The user just
    // changed their password from a known-authenticated session;
    // sending a confirmation to their email is the standard
    // self-defence against an attacker who silently cycled
    // someone else's password. Failures here do not roll the
    // change back — the audit log records both the change and
    // the notification outcome separately.
    //
    // Sent inline (await the result rather than tokio::spawn).
    // Inline keeps the test path deterministic and also means a
    // notification timeout is bounded by our SMTP timeout
    // (single-digit seconds in production), which is
    // operationally fine for a self-service action that already
    // holds a database write.
    if let Ok(Some(user_row)) =
        sui_id_store::repos::users::find_by_id_opt(&app.db, user_id).await
    {
        if let Some(email) = user_row.email.as_deref() {
            // Recipient's preferred locale, falling through to
            // the server default if unset. Resolved here rather
            // than inside `notify_password_changed` so that
            // function stays a pure builder.
            let recipient_locale = user_row
                .preferred_lang
                .as_deref()
                .and_then(sui_id_i18n::Locale::parse)
                .unwrap_or_default();
            if let Err(e) = sui_id_core::forgot_password::notify_password_changed(
                app.mailer.as_ref(),
                email,
                &user_row.display_name,
                recipient_locale,
            ).await
            {
                tracing::warn!(
                    error = %e,
                    "failed to send password-change notification"
                );
            }
        }
    }
    Ok(Redirect::to("/me/security?msg=password_changed").into_response())
}

// =====================================================================
// RFC 040 — /me/security tabbed pages
// =====================================================================

