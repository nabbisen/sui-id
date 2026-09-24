//! Admin handlers for users (RFC 066).

use super::forms::{ConfirmedReasonForm, DisableForm};
use super::with_csrf_cookie;
use crate::errors::HttpError;
use crate::handlers::{AppStateExt, CurrentAdmin, CurrentAdminOrAuditor};
use axum::Form;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
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
        ConfirmDeleteUserData, ConfirmDisableData, ConfirmRecoveryLinkData, ConfirmResetMfaData,
        RecoveryLinkIssuedData, RecoveryLinkRefusedData, RecoveryRefusedKind, UserDetailData,
        UserDetailSession,
    },
    render_confirm_delete_user, render_confirm_disable_user, render_confirm_recovery_link,
    render_confirm_reset_mfa, render_recovery_link_issued, render_recovery_link_refused,
    render_user_detail, render_users, render_users_new,
};

pub async fn users_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, role, ref read_actor): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let admin = users::get(&app.db, admin_id)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let rows = admin_uc::list_users(&app.db, read_actor)
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
    CurrentAdminOrAuditor(admin_id, _role, ref actor): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    // RFC 088: auditors reach a 403 page, not a login redirect.
    if !actor.can_write() {
        return Err(crate::errors::HttpError::html_403_auditor());
    }
    let State(app) = state_ext;
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
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
    #[serde(default)]
    pub is_admin: Option<String>,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn users_create(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, ref admin_actor): CurrentAdmin,
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
    // RFC 115 D2/D3: the account is created with no password, and the
    // administrator goes straight to issuing the recovery link that lets its
    // holder choose one. Two commands in sequence (U01, then U37 on the next
    // request), never one transaction: U37 keeps its own step-up, target and
    // throttle checks, and there is one copy of them. If the second never
    // happens there is a user with no credential and nothing that can sign
    // in; the administrator issues the link from the user's page.
    let create_result = admin_uc::create_user(
        &app.db,
        &app.clock,
        admin_actor,
        CreateUserSpec {
            username: form.username.trim(),
            display_name: display,
            email,
            is_admin,
        },
    )
    .await;

    match create_result {
        Ok(created) => Ok(Redirect::to(&format!(
            "/admin/users/{}/recovery-link-confirm",
            created.id
        ))
        .into_response()),
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
    CurrentAdmin(_, ref admin_actor): CurrentAdmin,
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
    if let Err(e) =
        admin_uc::set_user_disabled(&app.db, &app.clock, admin_actor, target, value, reason_opt)
            .await
    {
        return crate::handlers::gated_command_error(e, "/admin/users");
    }
    Ok(Redirect::to("/admin/users").into_response())
}

pub async fn users_delete(
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
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/users").await
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    if let Err(e) =
        admin_uc::delete_user(&app.db, &app.clock, admin_actor, target, form.reason_opt()).await
    {
        return crate::handlers::gated_command_error(e, "/admin/users");
    }
    Ok(Redirect::to("/admin/users").into_response())
}

/// Forcibly remove every MFA factor for a target user. Recovery path
/// for users who lost their second factor entirely.
pub async fn users_mfa_reset(
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
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/users").await
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    if let Err(e) =
        admin_uc::admin_reset_mfa(&app.db, &app.clock, admin_actor, target, form.reason_opt()).await
    {
        return crate::handlers::gated_command_error(e, "/admin/users");
    }
    Ok(Redirect::to("/admin/users").into_response())
}

pub async fn users_detail_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, role, _): CurrentAdminOrAuditor,
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

    // RFC 115 D10, display only: an administrator who has never held a
    // credential and never signed in is being activated, not captured, so the
    // button is offered for them. U37 re-reads and decides in its transaction.
    let never_activated_admin = if user.role.is_admin() && user.last_login_at.is_none() {
        match sui_id_store::repos::credentials::get(&app.db, user.id).await {
            Ok(_) => false,
            Err(sui_id_store::StoreError::NotFound) => true,
            Err(e) => return Err(HttpError::html(CoreError::from(e))),
        }
    } else {
        false
    };

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
        // Display only (RFC 103 D5): U37 enforces every rule in its transaction.
        // The viewer is an administrator here. An administrator target is
        // refused unless it has never been activated (RFC 115 D10), which the
        // viewer's own account never satisfies (they are signed in), so D5's
        // "not the issuer" rule needs no separate condition.
        can_issue_recovery: role.is_admin()
            && user.source.is_local()
            && !user.is_disabled
            && !user.is_deleted
            && (!user.role.is_admin() || never_activated_admin),
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
    CurrentAdminOrAuditor(admin_id, _role, ref actor): CurrentAdminOrAuditor,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(crate::errors::HttpError::html_403_auditor());
    }
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
    CurrentAdminOrAuditor(admin_id, _role, ref actor): CurrentAdminOrAuditor,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(crate::errors::HttpError::html_403_auditor());
    }
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
    CurrentAdminOrAuditor(admin_id, _role, ref actor): CurrentAdminOrAuditor,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(crate::errors::HttpError::html_403_auditor());
    }
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

// ---------- administrator-issued account recovery (RFC 103) ----------

/// The page a refusal renders: its own message, the given status, and nothing
/// changed. Used for every `RecoveryRefusal` and for D6's no-second-factor
/// case.
async fn recovery_refused_page(
    app: &crate::AppState,
    admin_id: UserId,
    jar: &CookieJar,
    target_id: &str,
    kind: RecoveryRefusedKind,
) -> Response {
    let token = crate::csrf::ensure_token(jar);
    let lang = crate::handlers::resolve_admin_locale(app, admin_id).await;
    let status = match kind {
        RecoveryRefusedKind::ReasonRequired
        | RecoveryRefusedKind::ReasonTooLong
        | RecoveryRefusedKind::ReasonHasControlCharacters => StatusCode::BAD_REQUEST,
        RecoveryRefusedKind::TargetUnknown => StatusCode::NOT_FOUND,
        RecoveryRefusedKind::TargetIsSelf
        | RecoveryRefusedKind::TargetIsAdmin
        | RecoveryRefusedKind::NeedsSecondFactor => StatusCode::FORBIDDEN,
        RecoveryRefusedKind::TargetNonLocal
        | RecoveryRefusedKind::TargetDisabled
        | RecoveryRefusedKind::TargetDeleted => StatusCode::CONFLICT,
        RecoveryRefusedKind::Throttled => StatusCode::TOO_MANY_REQUESTS,
    };
    let html = render_recovery_link_refused(
        RecoveryLinkRefusedData {
            user_id: target_id.to_owned(),
            kind,
            csrf_token: token.clone(),
        },
        app.is_dev_mode,
        lang,
    );
    with_csrf_cookie((status, Html(html)).into_response(), app, &token)
}

fn refused_kind(why: sui_id_store::errors::RecoveryRefusal) -> RecoveryRefusedKind {
    use sui_id_store::errors::RecoveryRefusal as Why;
    match why {
        Why::ReasonRequired => RecoveryRefusedKind::ReasonRequired,
        Why::ReasonTooLong => RecoveryRefusedKind::ReasonTooLong,
        Why::ReasonHasControlCharacters => RecoveryRefusedKind::ReasonHasControlCharacters,
        Why::TargetUnknown => RecoveryRefusedKind::TargetUnknown,
        Why::TargetIsSelf => RecoveryRefusedKind::TargetIsSelf,
        Why::TargetIsAdmin => RecoveryRefusedKind::TargetIsAdmin,
        Why::TargetNonLocal => RecoveryRefusedKind::TargetNonLocal,
        Why::TargetDisabled => RecoveryRefusedKind::TargetDisabled,
        Why::TargetDeleted => RecoveryRefusedKind::TargetDeleted,
        Why::Throttled => RecoveryRefusedKind::Throttled,
    }
}

/// GET /admin/users/{id}/recovery-link-confirm: the confirm screen for
/// issuing a recovery link (RFC 103 D6). It needs a fresh step-up like the
/// other dangerous operations, and says up front when the viewing
/// administrator holds no second factor, which D6 requires. Display only:
/// U37 re-checks both in its transaction.
pub async fn users_recovery_link_confirm_get(
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
    let return_to = format!("/admin/users/{id}/recovery-link-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let user = users::get(&app.db, target)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    if !sui_id_core::step_up::user_has_mfa(&app.db, admin_id)
        .await
        .map_err(HttpError::html)?
    {
        return Ok(recovery_refused_page(
            &app,
            admin_id,
            &jar,
            &id,
            RecoveryRefusedKind::NeedsSecondFactor,
        )
        .await);
    }
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmRecoveryLinkData {
        user_id: id,
        username: user.username,
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_recovery_link(data, app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// POST /admin/users/{id}/recovery-link: issue the link (RFC 103 D5, D6, D8,
/// D10). The gates run in the order the other dangerous operations use: CSRF,
/// `_confirmed=1`, fresh step-up. Then `recovery_link::issue_as_admin`, which
/// re-checks the step-up, the target and the throttle in its own transaction.
///
/// **The link and the token are rendered directly in this response**, with
/// `Cache-Control: no-store` and `Referrer-Policy: no-referrer`, and are never
/// placed in a redirect, a URL, a cookie or a log line.
pub async fn users_recovery_link(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, ref admin_actor): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    request_id: Option<axum::Extension<crate::request_id::RequestId>>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedReasonForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    let confirm_url = format!("/admin/users/{id}/recovery-link-confirm");
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &confirm_url).await {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    // The page needs the username, so read it before issuing: a lookup that
    // fails after issuing would leave a live link nobody could be shown.
    let user = match users::get(&app.db, target).await {
        Ok(u) => u,
        Err(sui_id_store::StoreError::NotFound) => {
            return Ok(recovery_refused_page(
                &app,
                admin_id,
                &jar,
                &id,
                RecoveryRefusedKind::TargetUnknown,
            )
            .await);
        }
        Err(e) => return Err(HttpError::html(CoreError::from(e))),
    };

    let request_id = request_id.as_ref().map(|e| e.0.0.as_str()).unwrap_or("-");
    let link = match sui_id_core::recovery_link::issue_as_admin(
        &app.db,
        &app.clock,
        admin_actor,
        target,
        &form.reason,
    )
    .await
    {
        Ok(link) => link,
        Err(CoreError::Store(sui_id_store::StoreError::StepUpRequired)) => {
            // The step-up lapsed between the gate and the commit, or the
            // administrator has no second factor to step up with (D6). A
            // redirect to the step-up page would be a dead end for the
            // second case, so say so instead.
            if sui_id_core::step_up::user_has_mfa(&app.db, admin_id)
                .await
                .map_err(HttpError::html)?
            {
                return Ok(crate::handlers::step_up_redirect(&confirm_url));
            }
            return Ok(recovery_refused_page(
                &app,
                admin_id,
                &jar,
                &id,
                RecoveryRefusedKind::NeedsSecondFactor,
            )
            .await);
        }
        Err(CoreError::Store(sui_id_store::StoreError::RecoveryRefused(why))) => {
            if matches!(why, sui_id_store::errors::RecoveryRefusal::Throttled) {
                // RFC 103 D8, RFC 102 C1 form: request id, command, acting
                // user and method. Never the target's link or any credential.
                tracing::warn!(
                    request_id,
                    command = "U37",
                    user = %admin_id,
                    via = "web",
                    "recovery link refused: the hourly limit was reached"
                );
            }
            return Ok(recovery_refused_page(&app, admin_id, &jar, &id, refused_kind(why)).await);
        }
        Err(e) => {
            tracing::error!(
                request_id,
                command = "U37",
                user = %admin_id,
                via = "web",
                error = %e,
                "recovery link issuance failed; nothing was changed"
            );
            return Err(HttpError::html(e));
        }
    };

    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let html = render_recovery_link_issued(
        RecoveryLinkIssuedData {
            user_id: id,
            username: user.username,
            url: sui_id_core::recovery_link::completion_url(app.issuer(), &link.token),
            token: link.token.expose().to_owned(),
            expires_at: link.expires_at,
            invalidated: link.invalidated,
            csrf_token: token.clone(),
        },
        app.is_dev_mode,
        lang,
    );
    let mut resp = Html(html).into_response();
    let headers = resp.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// POST /admin/users/{id}/role — change a user's access role (RFC 071).
/// Admin-only; enforces the last-admin safeguard before any demotion.
pub async fn users_set_role(
    State(app): AppStateExt,
    CurrentAdmin(admin_id, ref admin_actor): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfRoleForm>,
) -> Result<Response, HttpError> {
    crate::handlers::enforce_csrf(&jar, Some(form._csrf.as_str()))?;

    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;

    let new_role = sui_id_store::models::Role::from_db_str(form.role.as_str())
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("invalid role value".into())))?;

    // Last-admin safeguard: consult the RFC 082 authorization core.
    // We resolve `target_is_last_admin` here (the environmental fact)
    // and pass it into the pure decision table.
    let target_user = users::get(&app.db, target)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let target_is_last_admin = if target_user.role.is_admin() && !new_role.is_admin() {
        sui_id_store::repos::users::count_admins(&app.db)
            .await
            .unwrap_or(1)
            <= 1
    } else {
        false
    };
    if sui_id_core::authz::authorize(
        admin_actor.actor().role(),
        sui_id_core::authz::Action::AdminChangeUserRole {
            target_is_last_admin,
        },
    ) == sui_id_core::authz::Decision::Deny
    {
        let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
        return Err(HttpError::html(CoreError::BadRequest(
            lang.strings().user_detail_role_last_admin.to_owned(),
        )));
    }

    // RFC 094 U05: the mutation and the `user.role_change` audit event
    // commit in one Class-A transaction. The last-admin decision above
    // stays here (non-racy: it decides whether *this actor* may attempt
    // the change and produces the localized rejection message); the guard
    // inside `change_user_role` re-reads the admin count from the same
    // transaction that performs the demotion and is what actually closes
    // the race a pre-transaction count can't — see its own doc comment.
    // A `StoreError::Conflict` here means that guard fired: the rare case
    // where a concurrent change made this request's own pre-check stale.
    sui_id_store::commands::change_user_role(&app.db, admin_id, target, new_role)
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
