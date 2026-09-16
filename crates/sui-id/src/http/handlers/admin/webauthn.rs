//! Admin handlers for webauthn (RFC 066).

use crate::errors::HttpError;
use crate::handlers::{AppStateExt, PENDING_MFA_NEXT_COOKIE, session_cookie};
use axum::Form;
use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use sui_id_core::errors::CoreError;

#[derive(Debug, Deserialize)]

pub struct WebauthnAuthStartForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// Starts a passkey-authentication ceremony for the user identified by
/// the active `sui_id_pending_mfa` cookie. Returns the
/// `RequestChallengeResponse` JSON for `navigator.credentials.get()`.
pub async fn webauthn_auth_start(
    state_ext: AppStateExt,
    jar: CookieJar,
    Form(form): Form<WebauthnAuthStartForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let pending_value = jar
        .get(crate::handlers::PENDING_MFA_COOKIE)
        .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?
        .value()
        .to_owned();
    let pending_mfa_id = pending_value
        .parse::<sui_id_shared::ids::PendingMfaId>()
        .map_err(|_| HttpError::html(CoreError::Unauthenticated))?;
    // Look up the user via the pending-MFA row.
    let pending = sui_id_store::repos::login_pending_mfa::get(&app.db, pending_mfa_id)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?
        .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
    if pending.expires_at < app.clock.now() {
        return Err(HttpError::html(CoreError::Unauthenticated));
    }
    let started = sui_id_core::webauthn::start_authentication(
        &app.db,
        &app.clock,
        app.issuer(),
        pending.user_id,
    )
    .await
    .map_err(HttpError::html)?;
    let cookie = crate::handlers::webauthn_pending_cookie(
        started.pending_id.to_string(),
        app.config.server.cookie_secure,
    );
    let jar = jar.add(cookie);
    let challenge_value: serde_json::Value = serde_json::from_str(&started.challenge_json)
        .map_err(|_| HttpError::html(CoreError::Internal))?;
    Ok((jar, Json(challenge_value)).into_response())
}

#[derive(Debug, Deserialize)]

pub struct WebauthnAuthCompleteForm {
    pub credential: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn webauthn_auth_complete(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    request_id: Option<axum::Extension<crate::request_id::RequestId>>,
    jar: CookieJar,
    Form(form): Form<WebauthnAuthCompleteForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Login,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;
    // RFC 102 N6/N14: every other error on this step is `Unauthenticated`,
    // a redirect back to sign-in that keeps a pending `next`. The sign-in
    // script submits this form natively, so the browser follows it.
    let next = jar
        .get(PENDING_MFA_NEXT_COOKIE)
        .map(|c| c.value().to_owned())
        .filter(|s| s.starts_with('/') && !s.starts_with("//"));
    let refused = |err: Option<CoreError>| {
        if let Some(err) = err {
            let request_id = request_id.as_ref().map(|e| e.0.0.as_str()).unwrap_or("-");
            tracing::error!(
                request_id,
                error = %err,
                detail = ?err,
                "passkey sign-in failed for a reason other than a rejected assertion; \
                 returning the uniform failure response"
            );
        }
        let to = match &next {
            Some(n) => format!(
                "/admin/login?next={}",
                percent_encoding::utf8_percent_encode(n, percent_encoding::NON_ALPHANUMERIC)
            ),
            None => "/admin/login".to_owned(),
        };
        Ok(Redirect::to(&to).into_response())
    };
    if crate::handlers::enforce_csrf(&jar, Some(&form.csrf)).is_err() {
        return refused(None);
    }
    let Some(pending_mfa_id) = jar
        .get(crate::handlers::PENDING_MFA_COOKIE)
        .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok())
    else {
        return refused(None);
    };
    let Some(webauthn_pending_id) =
        jar.get(crate::handlers::WEBAUTHN_PENDING_COOKIE)
            .and_then(|c| {
                c.value()
                    .parse::<sui_id_shared::ids::WebauthnPendingId>()
                    .ok()
            })
    else {
        return refused(None);
    };
    let pending = match sui_id_store::repos::login_pending_mfa::get(&app.db, pending_mfa_id).await {
        Ok(Some(p)) if p.expires_at >= app.clock.now() => p,
        Ok(_) => return refused(None),
        Err(e) => return refused(Some(CoreError::from(e))),
    };
    let Ok(credential) =
        serde_json::from_str::<webauthn_rs::prelude::PublicKeyCredential>(&form.credential)
    else {
        return refused(None);
    };
    match sui_id_core::webauthn::finish_authentication(
        &app.db,
        &app.clock,
        app.issuer(),
        webauthn_pending_id,
        pending.user_id,
        &credential,
    )
    .await
    {
        Ok(()) => {}
        // A storage or internal failure is not a wrong factor (A9).
        Err(e @ (CoreError::Store(_) | CoreError::Internal)) => return refused(Some(e)),
        // A rejected assertion or ceremony is counted on the user (L07).
        Err(_) => {
            let max_lockout = app.config.security.max_lockout.as_secs();
            if let Err(e) = sui_id_core::mfa::record_second_factor_failure(
                &app.db,
                pending.user_id,
                max_lockout,
            )
            .await
            {
                return refused(Some(e));
            }
            return refused(None);
        }
    }
    // L02 commits the session with `auth.mfa.success`; if it fails, L07 is
    // not run (A9). A lost guard is an ordinary refusal.
    let session = match sui_id_core::mfa::verify_pending_webauthn(
        &app.db,
        &app.clock,
        pending_mfa_id,
        pending.user_id,
    )
    .await
    {
        Ok(session) => session,
        Err(CoreError::Unauthenticated) => return refused(None),
        Err(e) => return refused(Some(e)),
    };
    let next = next.unwrap_or_else(|| "/admin".into());
    let cookie = session_cookie(session.id.to_string(), app.config.server.cookie_secure);
    let jar = jar
        .add(cookie)
        .add(crate::handlers::clear_pending_mfa_cookie(
            app.config.server.cookie_secure,
        ))
        .add(crate::handlers::clear_pending_mfa_next_cookie(
            app.config.server.cookie_secure,
        ))
        .add(crate::handlers::clear_webauthn_pending_cookie(
            app.config.server.cookie_secure,
        ));
    Ok((jar, Redirect::to(&next)).into_response())
}
