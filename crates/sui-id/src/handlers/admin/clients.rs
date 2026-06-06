//! Admin handlers for clients (RFC 066).

use crate::errors::HttpError;
use crate::handlers::{
    AppStateExt, CurrentAdmin,
};
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use std::str::FromStr;
use sui_id_core::admin::{self as admin_uc};
use sui_id_core::errors::CoreError;
use sui_id_shared::api::ClientSummary;
use sui_id_shared::ids::ClientId;
use sui_id_store::repos::clients;
use sui_id_web::{
    pages::ConfirmDeleteClientData, render_clients, render_confirm_delete_client,
};
use super::forms::{DisableForm, ConfirmedReasonForm};
use super::with_csrf_cookie;

pub async fn clients_delete_confirm_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let return_to = format!("/admin/clients/{id}/delete-confirm");
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await
    {
        return Ok(redirect);
    }
    let cid = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let client = clients::get(&app.db, cid).await
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmDeleteClientData {
        client_id: id,
        client_name: client.name,
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_delete_client(data, app.is_dev_mode, lang))
        .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}


pub async fn clients_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let rows = admin_uc::list_clients(&app.db, admin_id).await.map_err(HttpError::html)?;
    let summaries: Vec<ClientSummary> = rows
        .into_iter()
        .map(|r| ClientSummary {
            id: r.id,
            name: r.name,
            redirect_uris: r.redirect_uris,
            allowed_scopes: r.allowed_scopes,
            post_logout_redirect_uris: r.post_logout_redirect_uris,
            confidential: r.confidential,
            is_disabled: r.is_disabled,
            is_deleted: r.is_deleted,
            consent_policy: r.consent_policy.as_str().to_string(),
            created_at: r.created_at,
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_clients(summaries, None, None, token.clone(), app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]

pub struct CreateClientForm {
    pub name: String,
    pub redirect_uris: String,
    #[serde(default)]
    pub confidential: Option<String>,
    #[serde(default)]
    pub allowed_scopes: String,
    #[serde(default)]
    pub post_logout_redirect_uris: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}


pub async fn clients_create(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Form(form): Form<CreateClientForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let uris: Vec<String> = form
        .redirect_uris
        .lines()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let post_logout_uris: Vec<String> = form
        .post_logout_redirect_uris
        .lines()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let confidential = form
        .confidential
        .as_deref()
        .map(|v| matches!(v, "true" | "on" | "1"))
        .unwrap_or(true);
    // Default policy: openid + profile + email if the operator left
    // the field blank. This covers the three scopes needed for most
    // basic OIDC integrations. Operators can restrict or extend the
    // list by editing the client after creation.
    //
    // Background: RFC 027. The original default was "" (no scopes),
    // which caused every first-time OIDC integration attempt to fail
    // with "scope not permitted" before the operator knew the field
    // existed.
    let raw_scopes = form.allowed_scopes.trim();
    let allowed_scopes = if raw_scopes.is_empty() {
        "openid profile email"
    } else {
        raw_scopes
    };
    let created = admin_uc::create_client(
        &app.db,
        &app.clock,
        admin_id,
        sui_id_core::admin::CreateClientSpec {
            name: form.name.trim(),
            redirect_uris: &uris,
            confidential,
            allowed_scopes,
            post_logout_redirect_uris: &post_logout_uris,
        },
        &app.caches,
    ).await
    .map_err(HttpError::html)?;

    // Re-list and pass the secret through to the page so it is shown once.
    let rows = admin_uc::list_clients(&app.db, admin_id).await.map_err(HttpError::html)?;
    let summaries: Vec<ClientSummary> = rows
        .into_iter()
        .map(|r| ClientSummary {
            id: r.id,
            name: r.name,
            redirect_uris: r.redirect_uris,
            allowed_scopes: r.allowed_scopes,
            post_logout_redirect_uris: r.post_logout_redirect_uris,
            confidential: r.confidential,
            is_disabled: r.is_disabled,
            is_deleted: r.is_deleted,
            consent_policy: r.consent_policy.as_str().to_string(),
            created_at: r.created_at,
        })
        .collect();

    let secret_payload =
        created.generated_secret.map(|s| (created.row.id.to_string(), s));
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_clients(summaries, None, secret_payload, token.clone(), app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}


pub async fn clients_set_disabled(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<DisableForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    // RFC 060 bug fix: also gate on `_confirmed=1`.
    crate::handlers::require_confirmed(&form.confirmed)?;
    // RFC 058: step-up immediately after CSRF + confirm gate.
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/clients").await
    {
        return Ok(redirect);
    }
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let value = matches!(form.disabled.as_str(), "true" | "on" | "1");
    let reason_opt = if form.reason.trim().is_empty() {
        None
    } else {
        Some(form.reason.trim().to_string())
    };
    admin_uc::set_client_disabled(&app.db, &app.clock, admin_id, target, value,
        reason_opt, &app.caches).await.map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/clients").into_response())
}


pub async fn clients_delete(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedReasonForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    // RFC 060: enforce the confirm screen's `_confirmed=1` token.
    crate::handlers::require_confirmed(&form.confirmed)?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/clients").await
    {
        return Ok(redirect);
    }
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    admin_uc::delete_client(&app.db, admin_id, target, form.reason_opt(), &app.caches)
        .await.map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/clients").into_response())
}

#[derive(Debug, serde::Deserialize, Default)]

pub struct ClientEditQuery {
    /// Present after a successful secret rotation — contains the new
    /// plaintext secret to display once (RFC 047).
    pub rotated_secret: Option<String>,
}


pub async fn clients_edit_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<ClientEditQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let row = admin_uc::get_client(&app.db, admin_id, target).await.map_err(HttpError::html)?;
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(sui_id_web::render_client_edit(
        sui_id_web::ClientEditData {
            id: row.id.to_string(),
            name: row.name,
            redirect_uris: row.redirect_uris,
            allowed_scopes: row.allowed_scopes,
            post_logout_redirect_uris: row.post_logout_redirect_uris,
            confidential: row.confidential,
            is_disabled: row.is_disabled,
            consent_policy: row.consent_policy.as_str().to_string(),
            freshly_rotated_secret: q.rotated_secret,
        },
        None,
        token.clone(),
        crate::handlers::resolve_admin_locale(&app, admin_id).await,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]

pub struct EditClientForm {
    pub name: String,
    pub redirect_uris: String,
    #[serde(default)]
    pub allowed_scopes: String,
    #[serde(default)]
    pub consent_policy: String,
    #[serde(default)]
    pub post_logout_redirect_uris: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}


pub async fn clients_edit_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<EditClientForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let uris: Vec<String> = form
        .redirect_uris
        .lines()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let post_logout_uris: Vec<String> = form
        .post_logout_redirect_uris
        .lines()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    // Apply all three updates. Each is admin-gated and audit-logged
    // separately; the operator sees three audit-log entries per save,
    // which is desirable — it makes it possible to track exactly which
    // facet of a client changed when.
    admin_uc::update_client_basic(&app.db, admin_id, target, form.name.trim(), &uris, &app.caches).await
        .map_err(HttpError::html)?;
    admin_uc::set_client_allowed_scopes(
        &app.db,
        admin_id,
        target,
        form.allowed_scopes.trim(),
    ).await
    .map_err(HttpError::html)?;
    admin_uc::set_client_post_logout_redirect_uris(
        &app.db,
        admin_id,
        target,
        &post_logout_uris,
    ).await
    .map_err(HttpError::html)?;
    // Update consent policy (RFC 038)
    let policy = sui_id_store::models::ConsentPolicy::parse(form.consent_policy.trim());
    sui_id_store::repos::clients::update_consent_policy(
        &app.db, target, policy, app.clock.now()
    ).await.map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    Ok(Redirect::to("/admin/clients").into_response())
}

// ---------- audit ----------

pub async fn clients_rotate_secret_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedReasonForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/clients").await
    {
        return Ok(redirect);
    }
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let new_secret = admin_uc::rotate_client_secret(
        &app.db, &app.clock, admin_id, target, form.reason_opt()
    ).await.map_err(HttpError::html)?;
    // Redirect to edit page with the new secret in the query string.
    // The secret is URL-encoded; the edit page displays it once and the
    // browser history entry is replaced by the subsequent navigation.
    let encoded = percent_encoding::utf8_percent_encode(
        &new_secret, percent_encoding::NON_ALPHANUMERIC
    ).to_string();
    Ok(Redirect::to(&format!("/admin/clients/{id}/edit?rotated_secret={encoded}")).into_response())
}
