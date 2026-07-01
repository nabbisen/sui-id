//! Admin handlers for users (RFC 066).

use super::forms::{ConfirmedReasonForm, DisableForm};
use super::with_csrf_cookie;
use crate::errors::HttpError;
use crate::handlers::{AppStateExt, CurrentAdmin, CurrentAdminOrAuditor};
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use std::str::FromStr;
use sui_id_core::admin::{self as admin_uc, CreateUserSpec};
use sui_id_core::errors::CoreError;
use sui_id_shared::api::{AuditLogEntryDto, UserSummary};
use sui_id_shared::ids::UserId;
use sui_id_store::repos::users;
use sui_id_web::{
    Flash, FlashKind,
    pages::{
        ConfirmDeleteUserData, ConfirmDisableData, ConfirmResetMfaData, UserDetailData,
        UserDetailSession,
    },
    render_confirm_delete_user, render_confirm_disable_user, render_confirm_reset_mfa,
    render_user_detail, render_users, render_users_new,
};

pub async fn users_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, role): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let admin = users::get(&app.db, admin_id)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let rows = admin_uc::list_users(&app.db, admin_id)
        .await
        .map_err(HttpError::html)?;
    let mut summaries = Vec::with_capacity(rows.len());
    for r in rows {
        let mfa_enabled = sui_id_core::mfa::is_mfa_enabled(&app.db, r.id)
            .await
            .unwrap_or(false);
        summaries.push(UserSummary {
            id: r.id,
            username: r.username,
            display_name: r.display_name,
            is_admin: r.is_admin,
            is_disabled: r.is_disabled,
            is_deleted: r.is_deleted,
            mfa_enabled,
            created_at: r.created_at,
        });
    }
    // summaries already collected in for loop above
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_users(
        role.is_admin(),
        summaries,
        None,
        admin.username,
        token.clone(),
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// `GET /admin/users/new` — isolated create-user form.
pub async fn users_new_get(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, _admin_id).await;
    let resp = Html(render_users_new(None, token.clone(), app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]

pub struct CreateUserForm {
    pub username: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: String,
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
    let email = if form.email.trim().is_empty() {
        None
    } else {
        Some(form.email.as_str())
    };
    let is_admin = form
        .is_admin
        .as_deref()
        .map(|v| matches!(v, "true" | "on" | "1"))
        .unwrap_or(false);
    let create_result = admin_uc::create_user(
        &app.db,
        &app.clock,
        Some(app.hibp_client.as_ref()),
        {
            sui_id_store::repos::server_settings::get(&app.db)
                .await
                .map(|s| s.hibp_mode)
                .unwrap_or_default()
        },
        admin_id,
        CreateUserSpec {
            username: form.username.trim(),
            password: &form.password,
            display_name: display,
            email,
            is_admin,
            min_password_len: crate::handlers::password_min_len(&app),
        },
    )
    .await;

    match create_result {
        Ok(_) => Ok(Redirect::to("/admin/users").into_response()),
        Err(CoreError::Conflict(msg)) => {
            // Duplicate username: re-render the create form with the error
            // so the admin can correct it without re-entering everything.
            let token = crate::csrf::ensure_token(&jar);
            let flash = Flash {
                kind: FlashKind::Error,
                text: msg,
            };
            let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
            let resp = Html(render_users_new(
                Some(flash),
                token.clone(),
                app.is_dev_mode,
                lang,
            ))
            .into_response();
            Ok((
                axum::http::StatusCode::CONFLICT,
                with_csrf_cookie(resp, &app, &token),
            )
                .into_response())
        }
        Err(e) => Err(HttpError::html(e)),
    }
}

pub async fn users_set_disabled(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<DisableForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    // RFC 060 bug fix: this handler previously accepted POSTs that
    // skipped the confirm screen. The confirm screen at
    // `users_disable_confirm_get` emits `_confirmed=1`; we now reject
    // direct POSTs without it.
    crate::handlers::require_confirmed(&form.confirmed)?;
    // RFC 058: step-up immediately after CSRF + confirm gate.
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/users").await
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let value = matches!(form.disabled.as_str(), "true" | "on" | "1");
    let reason_opt = if form.reason.trim().is_empty() {
        None
    } else {
        Some(form.reason.trim().to_string())
    };
    admin_uc::set_user_disabled(&app.db, admin_id, target, value, reason_opt)
        .await
        .map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/users").into_response())
}

pub async fn users_delete(
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
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/users").await
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    admin_uc::delete_user(&app.db, admin_id, target, form.reason_opt())
        .await
        .map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/users").into_response())
}

/// Forcibly remove every MFA factor for a target user. Recovery path
/// for users who lost their second factor entirely.

pub async fn users_mfa_reset(
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
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/users").await
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    admin_uc::admin_reset_mfa(&app.db, admin_id, target, form.reason_opt())
        .await
        .map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/users").into_response())
}

pub async fn users_detail_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, role): CurrentAdminOrAuditor,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let user = users::get(&app.db, target)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    // MFA state
    let totp = sui_id_store::repos::user_totp::get(&app.db, target)
        .await
        .unwrap_or(None);
    let totp_enabled = totp.map(|r| r.enabled).unwrap_or(false);
    let passkey_count =
        sui_id_store::repos::user_webauthn_credentials::count_for_user(&app.db, target)
            .await
            .unwrap_or(0);

    // Active sessions
    let sessions_raw = sui_id_store::repos::sessions::list_active_for_user(&app.db, target)
        .await
        .unwrap_or_default();

    let sessions: Vec<UserDetailSession> = sessions_raw
        .into_iter()
        .map(|s| {
            let factors = s
                .auth_methods
                .iter()
                .map(|m| format!("{m:?}").to_lowercase())
                .collect::<Vec<_>>()
                .join(", ");
            UserDetailSession {
                started: s.created_at,
                expires: s.expires_at,
                factors: if factors.is_empty() {
                    "password".into()
                } else {
                    factors
                },
            }
        })
        .collect();

    // Recent audit events (actor or target)
    let audit_rows = sui_id_store::repos::audit::recent_for_user(&app.db, target, 20)
        .await
        .unwrap_or_default();

    let recent_audit: Vec<AuditLogEntryDto> = audit_rows
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
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let data = UserDetailData {
        user_id: id,
        username: user.username,
        display_name: user.display_name,
        email: user.email,
        is_admin: user.is_admin,
        role: user.role, // RFC 071
        is_disabled: user.is_disabled,
        totp_enabled,
        passkey_count,
        sessions,
        recent_audit,
        dev_mode: app.is_dev_mode,
        csrf_token: token.clone(),
    };

    let resp = Html(render_user_detail(role.is_admin(), data, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- dangerous-op confirmation GET handlers (RFC 030) ----------

pub async fn users_disable_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role): CurrentAdminOrAuditor,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let user = users::get(&app.db, target)
        .await
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmDisableData {
        user_id: id,
        username: user.username,
        is_disabled: user.is_disabled,
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_disable_user(data, app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn users_delete_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role): CurrentAdminOrAuditor,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let return_to = format!("/admin/users/{id}/delete-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let user = users::get(&app.db, target)
        .await
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmDeleteUserData {
        user_id: id,
        username: user.username,
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_delete_user(data, app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn users_mfa_reset_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role): CurrentAdminOrAuditor,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let return_to = format!("/admin/users/{id}/mfa-reset-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let user = users::get(&app.db, target)
        .await
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmResetMfaData {
        user_id: id,
        username: user.username,
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_reset_mfa(data, app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// POST /admin/users/{id}/role — change a user's access role (RFC 071).
/// Admin-only; enforces the last-admin safeguard before any demotion.
pub async fn users_set_role(
    State(app): AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfRoleForm>,
) -> Result<Response, HttpError> {
    crate::handlers::enforce_csrf(&jar, Some(form._csrf.as_str()))?;

    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;

    let new_role = sui_id_store::models::Role::from_db_str(form.role.as_str())
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("invalid role value".into())))?;

    // Last-admin safeguard: refuse to demote the last remaining admin.
    if !new_role.is_admin() {
        let target_user = users::get(&app.db, target)
            .await
            .map_err(|e| HttpError::html(CoreError::from(e)))?;
        if target_user.role.is_admin() {
            let count = sui_id_store::repos::users::count_admins(&app.db)
                .await
                .unwrap_or(1);
            if count <= 1 {
                return Err(HttpError::html(CoreError::BadRequest(
                    {
                        let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
                        lang.strings().user_detail_role_last_admin.to_owned()
                    }
                    .into(),
                )));
            }
        }
    }

    sui_id_store::repos::users::set_role(&app.db, &target, new_role)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    Ok(Redirect::to(&format!("/admin/users/{id}")).into_response())
}

#[derive(serde::Deserialize)]
pub struct CsrfRoleForm {
    pub _csrf: String,
    pub role: String,
}

// Satisfy enforce_csrf which takes Option<&str>
impl CsrfRoleForm {
    pub fn csrf_str(&self) -> &str {
        &self._csrf
    }
}
