//! Admin handlers for signing_keys (RFC 066).

use super::forms::ConfirmedReasonForm;
use super::with_csrf_cookie;
use crate::errors::HttpError;
use crate::handlers::{AppStateExt, CurrentAdmin, CurrentAdminOrAuditor};
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use std::str::FromStr;
use sui_id_core::admin::{self as admin_uc};
use sui_id_core::errors::CoreError;
use sui_id_web::{
    pages::{ConfirmDeleteSigningKeyData, ConfirmRotateSigningKeyData},
    render_confirm_delete_signing_key, render_confirm_rotate_signing_key, render_signing_keys,
};

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
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmDeleteSigningKeyData {
        key_id: id,
        algorithm: "Ed25519".to_string(),
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_delete_signing_key(
        data,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- clients ----------

/// `GET /admin/signing-keys/rotate-confirm` — step-up-gated confirm page
/// before issuing a new signing key (RFC 090).
///
/// P4 (auditor 403): RFC 088 already gates this route via can_write().
/// P5 (no auto-execute): after step-up the user lands here; the rotation
/// only happens when the user submits the confirm POST.
pub async fn signing_keys_rotate_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, ref actor): CurrentAdminOrAuditor,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(crate::errors::HttpError::html_403_auditor());
    }
    let State(app) = state_ext;
    // RFC 089: /admin/signing-keys/ is in the step-up allowlist.
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/signing-keys/rotate-confirm")
            .await
    {
        return Ok(redirect);
    }
    // Count active keys for the impact message.
    let keys = sui_id_core::admin::list_signing_keys(&app.db)
        .await
        .unwrap_or_default();
    let active_key_count = keys.iter().filter(|k| k.is_active).count();
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmRotateSigningKeyData {
        csrf_token: token.clone(),
        active_key_count,
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_rotate_signing_key(
        data,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn signing_keys_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, role, _): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let rows = admin_uc::list_signing_keys(&app.db)
        .await
        .map_err(HttpError::html)?;
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
    let resp = Html(render_signing_keys(
        role.is_admin(),
        summaries,
        None,
        token.clone(),
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn signing_keys_rotate(
    state_ext: AppStateExt,
    CurrentAdmin(_, ref admin_actor): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Form(form): Form<ConfirmedReasonForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    // RFC 090: revalidate step-up on the final confirm POST.
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/signing-keys/rotate-confirm")
            .await
    {
        return Ok(redirect);
    }
    admin_uc::rotate_signing_key(
        &app.db,
        &app.clock,
        app.config.storage.key_file.to_str().unwrap_or_default(),
        admin_actor,
        form.reason_opt(),
        &app.caches,
    )
    .await
    .map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/signing-keys").into_response())
}

pub async fn signing_keys_delete(
    state_ext: AppStateExt,
    CurrentAdmin(_, ref admin_actor): CurrentAdmin,
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
    admin_uc::delete_signing_key(
        &app.db,
        &app.clock,
        admin_actor,
        target,
        form.reason_opt(),
        &app.caches,
    )
    .await
    .map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/signing-keys").into_response())
}
