//! /me/security passkey tab handlers (RFC 068).

use crate::{csrf, errors::HttpError};
use axum::extract::{Form, Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use sui_id_core::errors::CoreError;

use super::forms::*;
use crate::handlers::admin::with_csrf_cookie;
use crate::handlers::{AppStateExt, CurrentUser, enforce_csrf};
use sui_id_web::pages::{MePasskeyData, MeShellData, MeTab};

pub async fn passkeys_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let lang = req_locale;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let shell = MeShellData {
        username: user.username,
        is_admin: user.is_admin,
        active_tab: MeTab::Passkey,
    };
    let passkeys = sui_id_store::repos::user_webauthn_credentials::list_for_user(&app.db, user_id)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let descriptors = passkeys
        .into_iter()
        .map(|p| sui_id_web::PasskeyDescriptor {
            id: p.id.to_string(),
            nickname: p.nickname,
            created_at: p.created_at,
            last_used_at: None,
        })
        .collect();
    // Check if issuer is HTTPS or localhost
    let origin_eligible = {
        let issuer = app.issuer();
        issuer.starts_with("https://")
            || issuer.starts_with("http://localhost")
            || issuer.starts_with("http://127.0.0.1")
    };
    let flash: Option<sui_id_web::Flash> = None;
    let csrf_tok = csrf::ensure_token(&jar);
    let resp = axum::response::Html(sui_id_web::render_me_passkey(
        MePasskeyData {
            shell,
            passkeys: descriptors,
            origin_eligible,
            csrf_token: csrf_tok.clone(),
        },
        flash,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}

pub async fn passkey_rename_post(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Path(cred_id): Path<String>,
    Form(form): Form<PasskeyRenameForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let new_name = form.nickname.trim();
    if new_name.is_empty() || new_name.len() > 64 {
        return Err(HttpError::html(CoreError::BadRequest(
            "nickname must be 1–64 characters".into(),
        )));
    }
    sui_id_store::repos::user_webauthn_credentials::update_nickname(
        &app.db, &cred_id, user_id, new_name,
    )
    .await
    .map_err(|e| HttpError::html(CoreError::from(e)))?;
    Ok(Redirect::to("/me/security/passkeys").into_response())
}

/// Query parameters for `GET /me/security/language` (RFC 057).
///
/// `saved=1` means the user just successfully saved their preference
/// and should see a confirmation banner. Other values (typo, stale
/// link) are ignored — we never falsely tell the user their save
/// succeeded.
pub async fn passkey_register_start(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Form(form): Form<PasskeyRegisterStartForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let started =
        sui_id_core::webauthn::start_registration(&app.db, &app.clock, app.issuer(), user_id)
            .await
            .map_err(HttpError::html)?;
    let nickname_cookie = {
        use axum_extra::extract::cookie::{Cookie, SameSite};
        let mut c = Cookie::new("sui_id_webauthn_nickname", form.nickname);
        c.set_path("/");
        c.set_http_only(true);
        c.set_same_site(SameSite::Lax);
        c.set_secure(app.config.server.cookie_secure);
        c.set_max_age(cookie::time::Duration::minutes(5));
        c
    };
    let pending_cookie = crate::handlers::webauthn_pending_cookie(
        started.pending_id.to_string(),
        app.config.server.cookie_secure,
    );
    let jar = jar.add(pending_cookie).add(nickname_cookie);
    let challenge_value: serde_json::Value = serde_json::from_str(&started.challenge_json)
        .map_err(|_| HttpError::html(CoreError::Internal))?;
    Ok((jar, axum::Json(challenge_value)).into_response())
}

pub async fn passkey_register_complete(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Form(form): Form<PasskeyRegisterCompleteForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let pending_value = jar
        .get(crate::handlers::WEBAUTHN_PENDING_COOKIE)
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("no pending ceremony".into())))?
        .value()
        .to_owned();
    let pending_id: sui_id_shared::ids::WebauthnPendingId = pending_value
        .parse()
        .map_err(|_| HttpError::html(CoreError::BadRequest("malformed pending id".into())))?;
    let nickname = jar
        .get("sui_id_webauthn_nickname")
        .map(|c| c.value().to_owned())
        .unwrap_or_default();
    let credential: webauthn_rs::prelude::RegisterPublicKeyCredential =
        serde_json::from_str(&form.credential).map_err(|_| {
            HttpError::html(CoreError::BadRequest("malformed credential JSON".into()))
        })?;
    sui_id_core::webauthn::finish_registration(
        &app.db,
        &app.clock,
        app.issuer(),
        pending_id,
        user_id,
        &nickname,
        &credential,
    )
    .await
    .map_err(HttpError::html)?;
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(user_id),
            action: "webauthn.credential.register".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: None,
        },
    )
    .await;
    let jar = jar.add(crate::handlers::clear_webauthn_pending_cookie(
        app.config.server.cookie_secure,
    ));
    Ok((jar, Redirect::to("/me/security/passkeys")).into_response())
}

pub async fn passkey_delete(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(cred_id): Path<String>,
    Form(form): Form<PasskeyDeleteForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    // RFC 058: dangerous self-service action — deleting a passkey
    // removes one factor; pre-phishing-the-survivor is a known
    // attacker pattern. Step-up gates the action.
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/me/security/passkeys").await
    {
        return Ok(redirect);
    }
    let id = cred_id
        .parse::<sui_id_shared::ids::WebauthnCredentialId>()
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid credential id".into())))?;
    sui_id_core::webauthn::delete(&app.db, user_id, id)
        .await
        .map_err(HttpError::html)?;
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(user_id),
            action: "webauthn.credential.delete".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: Some("self".into()),
        },
    )
    .await;
    Ok(Redirect::to("/me/security/passkeys").into_response())
}
