//! Admin panel and login.
//!
//! All admin pages render via Leptos SSR through `sui-id-web`. State
//! transitions go via core use cases.

use crate::errors::HttpError;
use crate::handlers::{
    clear_session_cookie, session_cookie, AppStateExt, CurrentAdmin, CurrentUser, SESSION_COOKIE,
};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use std::str::FromStr;
use sui_id_core::admin::{self as admin_uc, CreateUserSpec};
use sui_id_core::errors::CoreError;
use sui_id_core::session;
use sui_id_shared::api::{
    AuditLogEntryDto, ClientSummary, UserSummary,
};
use sui_id_shared::ids::{ClientId, SessionId, UserId};
use sui_id_store::repos::{audit, clients, state, users};
use sui_id_web::{
    pages::DashboardData, render_audit, render_clients, render_dashboard, render_login,
    render_signing_keys, render_users, Flash, FlashKind,
};

/// Attach a `Set-Cookie` header for the CSRF token to a response.
fn with_csrf_cookie(mut resp: Response, app: &AppState, token: &str) -> Response {
    let cookie = crate::csrf::csrf_cookie(token.to_owned(), app.config.server.cookie_secure);
    if let Ok(v) = HeaderValue::from_str(&cookie.to_string()) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

// ---------- login ----------

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub next: String,
}

pub async fn login_get(jar: CookieJar, state_ext: AppStateExt) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Already logged in?
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        if let Ok(sid) = SessionId::from_str(cookie.value()) {
            if session::resolve(&app.db, &app.clock, sid).is_ok() {
                return Ok(Redirect::to("/admin").into_response());
            }
        }
    }
    Ok(Html(render_login(None, None)).into_response())
}

pub async fn login_post(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
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
    match session::login(&app.db, &app.clock, form.username.trim(), &form.password) {
        Ok(row) => {
            let cookie = session_cookie(row.id.to_string(), app.config.server.cookie_secure);
            let jar = jar.add(cookie);
            let target = if form.next.starts_with('/') {
                form.next.clone()
            } else {
                "/admin".into()
            };
            Ok((jar, Redirect::to(&target)).into_response())
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
                (StatusCode::UNAUTHORIZED, Html(render_login(Some(flash), next)))
                    .into_response(),
            )
        }
    }
}

pub async fn logout(
    jar: CookieJar,
    state_ext: AppStateExt,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    if let Some(c) = jar.get(SESSION_COOKIE) {
        if let Ok(sid) = SessionId::from_str(c.value()) {
            let _ = session::logout(&app.db, sid);
        }
    }
    let jar = jar.add(clear_session_cookie(app.config.server.cookie_secure));
    Ok((jar, Redirect::to("/admin/login")).into_response())
}

// ---------- dashboard ----------

pub async fn dashboard(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let admin = users::get(&app.db, admin_id).map_err(|e| HttpError::html(CoreError::from(e)))?;
    let users_n = users::list(&app.db)
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let clients_n = clients::list(&app.db)
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let data = DashboardData {
        admin_username: admin.username,
        user_count: users_n,
        client_count: clients_n,
        issuer: app.issuer().to_owned(),
    };
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_dashboard(data, None)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- users ----------

pub async fn users_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let admin = users::get(&app.db, admin_id).map_err(|e| HttpError::html(CoreError::from(e)))?;
    let rows = admin_uc::list_users(&app.db, admin_id).map_err(HttpError::html)?;
    let summaries: Vec<UserSummary> = rows
        .into_iter()
        .map(|r| UserSummary {
            id: r.id,
            username: r.username,
            display_name: r.display_name,
            is_admin: r.is_admin,
            is_disabled: r.is_disabled,
            is_deleted: r.is_deleted,
            created_at: r.created_at,
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_users(summaries, None, admin.username, token.clone())).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]
pub struct CreateUserForm {
    pub username: String,
    #[serde(default)]
    pub display_name: String,
    pub password: String,
    #[serde(default)]
    pub is_admin: Option<String>,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn users_create(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Form(form): Form<CreateUserForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let display = if form.display_name.trim().is_empty() {
        None
    } else {
        Some(form.display_name.as_str())
    };
    let is_admin = form
        .is_admin
        .as_deref()
        .map(|v| matches!(v, "true" | "on" | "1"))
        .unwrap_or(false);
    admin_uc::create_user(
        &app.db,
        &app.clock,
        admin_id,
        CreateUserSpec {
            username: form.username.trim(),
            password: &form.password,
            display_name: display,
            is_admin,
        },
    )
    .map_err(HttpError::html)?;
    let _ = &app; // hush
    Ok(Redirect::to("/admin/users").into_response())
}

#[derive(Debug, Deserialize)]
pub struct DisableForm {
    pub disabled: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn users_set_disabled(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<DisableForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let value = matches!(form.disabled.as_str(), "true" | "on" | "1");
    admin_uc::set_user_disabled(&app.db, admin_id, target, value).map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/users").into_response())
}

/// `_csrf`-only body: confirmation-style POSTs that have no other fields.
#[derive(Debug, Deserialize, Default)]
pub struct CsrfOnlyForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn users_delete(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    admin_uc::delete_user(&app.db, admin_id, target).map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/users").into_response())
}

// ---------- clients ----------

pub async fn clients_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let rows = admin_uc::list_clients(&app.db, admin_id).map_err(HttpError::html)?;
    let summaries: Vec<ClientSummary> = rows
        .into_iter()
        .map(|r| ClientSummary {
            id: r.id,
            name: r.name,
            redirect_uris: r.redirect_uris,
            confidential: r.confidential,
            is_disabled: r.is_disabled,
            is_deleted: r.is_deleted,
            created_at: r.created_at,
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_clients(summaries, None, None, token.clone())).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]
pub struct CreateClientForm {
    pub name: String,
    pub redirect_uris: String,
    #[serde(default)]
    pub confidential: Option<String>,
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
    let confidential = form
        .confidential
        .as_deref()
        .map(|v| matches!(v, "true" | "on" | "1"))
        .unwrap_or(true);
    let created = admin_uc::create_client(
        &app.db,
        &app.clock,
        admin_id,
        form.name.trim(),
        &uris,
        confidential,
    )
    .map_err(HttpError::html)?;

    // Re-list and pass the secret through to the page so it is shown once.
    let rows = admin_uc::list_clients(&app.db, admin_id).map_err(HttpError::html)?;
    let summaries: Vec<ClientSummary> = rows
        .into_iter()
        .map(|r| ClientSummary {
            id: r.id,
            name: r.name,
            redirect_uris: r.redirect_uris,
            confidential: r.confidential,
            is_disabled: r.is_disabled,
            is_deleted: r.is_deleted,
            created_at: r.created_at,
        })
        .collect();

    let secret_payload =
        created.generated_secret.map(|s| (created.row.id.to_string(), s));
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_clients(summaries, None, secret_payload, token.clone())).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn clients_set_disabled(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<DisableForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let value = matches!(form.disabled.as_str(), "true" | "on" | "1");
    admin_uc::set_client_disabled(&app.db, admin_id, target, value).map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/clients").into_response())
}

pub async fn clients_delete(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    admin_uc::delete_client(&app.db, admin_id, target).map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/clients").into_response())
}

// ---------- audit ----------

pub async fn audit_get(
    state_ext: AppStateExt,
    CurrentAdmin(_): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let entries = audit::recent(&app.db, 200)
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let dtos: Vec<AuditLogEntryDto> = entries
        .into_iter()
        .map(|r| AuditLogEntryDto {
            at: r.at,
            actor: r.actor,
            action: r.action,
            target: r.target,
            result: r.result,
            note: r.note,
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_audit(dtos, None)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- signing keys ----------

pub async fn signing_keys_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let rows = admin_uc::list_signing_keys(&app.db, admin_id).map_err(HttpError::html)?;
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
    let resp = Html(render_signing_keys(summaries, None, token.clone())).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn signing_keys_rotate(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    admin_uc::rotate_signing_key(&app.db, &app.clock, admin_id).map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/signing-keys").into_response())
}

pub async fn signing_keys_delete(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let target = sui_id_shared::ids::SigningKeyId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid signing key id".into())))?;
    admin_uc::delete_signing_key(&app.db, admin_id, target).map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/signing-keys").into_response())
}

// ---------- silence dead-code warnings for unused imports ----------

#[allow(dead_code)]
fn _silence_state(_: &CurrentUser) {}
#[allow(dead_code)]
fn _silence_state2() -> Option<bool> {
    let _ = state::is_initialized;
    None
}
