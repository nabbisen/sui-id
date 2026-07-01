//! Admin handlers for signing_keys (RFC 066).

use crate::errors::HttpError;
use crate::handlers::{
    AppStateExt, CurrentAdmin, CurrentAdminOrAuditor,
};
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use std::str::FromStr;
use sui_id_core::admin::{self as admin_uc};
use sui_id_core::errors::CoreError;
use sui_id_web::{
    pages::ConfirmDeleteSigningKeyData,
    render_confirm_delete_signing_key, render_signing_keys,
};
use super::forms::ConfirmedReasonForm;
use super::with_csrf_cookie;

pub async fn signing_keys_delete_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, ref actor): CurrentAdminOrAuditor,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(crate::errors::HttpError::html_403_auditor());
    }
    let State(app) = state_ext;
    let return_to = format!("/admin/signing-keys/{id}/delete-confirm");
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await
    {
        return Ok(redirect);
    }
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmDeleteSigningKeyData {
        key_id: id,
        algorithm: "Ed25519".to_string(),
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_delete_signing_key(data, app.is_dev_mode, lang))
        .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- clients ----------


pub async fn signing_keys_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, role, ref read_actor): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let rows = admin_uc::list_signing_keys(&app.db, read_actor).await.map_err(HttpError::html)?;
    let summaries: Vec<sui_id_shared::api::SigningKeySummary> = rows
        .into_iter()
        .map(|r| sui_id_shared::api::SigningKeySummary {
            id: r.id,
            algorithm: r.algorithm,
            is_active: r.is_active,
            created_at: r.created_at,
            rotated_at: r.rotated_at,
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_signing_keys(role.is_admin(), summaries, None, token.clone(), app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}


pub async fn signing_keys_rotate(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, ref admin_actor): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Form(form): Form<ConfirmedReasonForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/signing-keys").await
    {
        return Ok(redirect);
    }
    admin_uc::rotate_signing_key(&app.db, &app.clock,
        app.config.storage.key_file.to_str().unwrap_or_default(),
        admin_actor, form.reason_opt(), &app.caches)
        .await.map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/signing-keys").into_response())
}


pub async fn signing_keys_delete(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, ref admin_actor): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedReasonForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/signing-keys").await
    {
        return Ok(redirect);
    }
    let target = sui_id_shared::ids::SigningKeyId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid signing key id".into())))?;
    admin_uc::delete_signing_key(&app.db, &app.clock, admin_actor, target,
        form.reason_opt(), &app.caches)
        .await.map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/signing-keys").into_response())
}
