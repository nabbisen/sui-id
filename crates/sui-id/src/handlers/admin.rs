//! Admin panel and login.
//!
//! All admin pages render via Leptos SSR through `sui-id-web`. State
//! transitions go via core use cases.

use crate::errors::HttpError;
use crate::handlers::{
    clear_pending_mfa_cookie, clear_pending_mfa_next_cookie, clear_session_cookie,
    pending_mfa_cookie, pending_mfa_next_cookie, session_cookie, AppStateExt, CurrentAdmin,
    CurrentUser, PENDING_MFA_COOKIE, PENDING_MFA_NEXT_COOKIE, SESSION_COOKIE,
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
    pages::{
        ConfirmDeleteClientData, ConfirmDeleteSigningKeyData, ConfirmDeleteUserData,
        ConfirmDisableData, ConfirmResetMfaData, DashboardData,
        UserDetailData, UserDetailSession,
    },
    render_audit, render_clients, render_confirm_delete_client,
    render_confirm_delete_signing_key, render_confirm_delete_user,
    render_confirm_disable_user, render_confirm_reset_mfa,
    render_dashboard, render_login, render_signing_keys, render_user_detail,
    render_users, Flash, FlashKind,
};

/// Attach a `Set-Cookie` header for the CSRF token to a response.
pub(crate) fn with_csrf_cookie(mut resp: Response, app: &AppState, token: &str) -> Response {
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

pub async fn login_get(
    jar: CookieJar,
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Already logged in?
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        if let Ok(sid) = SessionId::from_str(cookie.value()) {
            if session::resolve(&app.db, &app.clock, sid).await.is_ok() {
                return Ok(Redirect::to("/admin").into_response());
            }
        }
    }
    Ok(Html(render_login(None, None, lang, false)).into_response())
}

pub async fn login_post(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
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
    match session::login_with_mfa(
        &app.db,
        &app.clock,
        form.username.trim(),
        &form.password,
        app.config.security.max_lockout.as_secs(),
    ).await {
        Ok(session::LoginOutcome::SessionEstablished(row)) => {
            let cookie = session_cookie(row.id.to_string(), app.config.server.cookie_secure);
            let jar = jar.add(cookie);
            let target = if form.next.starts_with('/') {
                form.next.clone()
            } else {
                "/admin".into()
            };
            Ok((jar, Redirect::to(&target)).into_response())
        }
        Ok(session::LoginOutcome::MfaRequired { pending }) => {
            // Drop the user a short-lived cookie pointing at the
            // pending row, and bounce them into the MFA challenge page.
            let cookie = pending_mfa_cookie(
                pending.id.to_string(),
                app.config.server.cookie_secure,
            );
            let next_cookie = if !form.next.is_empty() {
                Some(pending_mfa_next_cookie(
                    form.next.clone(),
                    app.config.server.cookie_secure,
                ))
            } else {
                None
            };
            let jar = jar.add(cookie);
            let jar = match next_cookie {
                Some(c) => jar.add(c),
                None => jar,
            };
            Ok((jar, Redirect::to("/admin/login/mfa")).into_response())
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
                (StatusCode::UNAUTHORIZED, Html(render_login(Some(flash), next, lang, false)))
                    .into_response(),
            )
        }
    }
}

pub async fn mfa_challenge_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Best-effort: if we can resolve the pending-MFA row, look up
    // whether the user has any passkeys so the page can offer that
    // path. If we can't (cookie missing or row gone), default to
    // hiding the passkey button — the user can still type a TOTP code.
    let has_passkey = {
        let pid_opt = jar
            .get(crate::handlers::PENDING_MFA_COOKIE)
            .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok());
        if let Some(pid) = pid_opt {
            let row_opt = sui_id_store::repos::login_pending_mfa::get(&app.db, pid)
                .await.ok().flatten();
            if let Some(row) = row_opt {
                sui_id_core::webauthn::has_credentials(&app.db, row.user_id).await.unwrap_or(false)
            } else { false }
        } else { false }
    };
    let token = crate::csrf::ensure_token(&jar);
    let resp =
        Html(sui_id_web::render_mfa_challenge(None, token.clone(), has_passkey, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]
pub struct MfaChallengeForm {
    pub code: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn mfa_challenge_post(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<MfaChallengeForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Same rate-limit bucket as password attempts: a user who is past
    // the password step still uses a single login budget.
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Login,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let pending_value = match jar.get(PENDING_MFA_COOKIE) {
        Some(c) => c.value().to_owned(),
        None => {
            return Ok(Redirect::to("/admin/login").into_response());
        }
    };
    let pending_id = match pending_value.parse::<sui_id_shared::ids::PendingMfaId>() {
        Ok(id) => id,
        Err(_) => return Ok(Redirect::to("/admin/login").into_response()),
    };
    match sui_id_core::mfa::verify_pending(&app.db, &app.clock, pending_id, &form.code).await {
        Ok(session) => {
            let cookie =
                session_cookie(session.id.to_string(), app.config.server.cookie_secure);
            // Compose the redirect target from the optional next cookie.
            let next_target = jar
                .get(PENDING_MFA_NEXT_COOKIE)
                .map(|c| c.value().to_owned())
                .filter(|s| s.starts_with('/'))
                .unwrap_or_else(|| "/admin".into());
            // Audit the MFA success.
            let _ = sui_id_store::repos::audit::append(
                &app.db,
                &sui_id_store::models::AuditLogRow {
                    at: app.clock.now(),
                    actor: Some(session.user_id),
                    action: "auth.mfa.success".into(),
                    target: Some(session.user_id.to_string()),
                    result: "ok".into(),
                    note: None,
                },
            ).await;
            let jar = jar
                .add(cookie)
                .add(clear_pending_mfa_cookie(app.config.server.cookie_secure))
                .add(clear_pending_mfa_next_cookie(app.config.server.cookie_secure));
            Ok((jar, Redirect::to(&next_target)).into_response())
        }
        Err(_) => {
            let t = lang.strings();
            let flash = Flash {
                kind: FlashKind::Error,
                text: t.mfa_challenge_failed_flash.into(),
            };
            let _ = sui_id_store::repos::audit::append(
                &app.db,
                &sui_id_store::models::AuditLogRow {
                    at: app.clock.now(),
                    actor: None,
                    action: "auth.mfa.failure".into(),
                    target: None,
                    result: "denied".into(),
                    note: None,
                },
            ).await;
            let has_passkey = {
                let pid_opt2 = jar
                    .get(crate::handlers::PENDING_MFA_COOKIE)
                    .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok());
                if let Some(pid) = pid_opt2 {
                    let row_opt2 = sui_id_store::repos::login_pending_mfa::get(&app.db, pid)
                        .await.ok().flatten();
                    if let Some(row) = row_opt2 {
                        sui_id_core::webauthn::has_credentials(&app.db, row.user_id).await.unwrap_or(false)
                    } else { false }
                } else { false }
            };
            let token = crate::csrf::ensure_token(&jar);
            let resp = (
                StatusCode::UNAUTHORIZED,
                Html(sui_id_web::render_mfa_challenge(
                    Some(flash),
                    token.clone(),
                    has_passkey,
                    lang,
                )),
            )
                .into_response();
            Ok(with_csrf_cookie(resp, &app, &token))
        }
    }
}

pub async fn logout(
    jar: CookieJar,
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    axum::Form(form): axum::Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    // Validate CSRF to prevent logout-CSRF attacks.
    // On failure, redirect to login rather than returning 403 —
    // a stale CSRF token (e.g. after a previous logout) should not
    // leave the operator staring at an error page.
    if crate::handlers::enforce_csrf(&jar, Some(&form.csrf)).is_err() {
        return Ok(Redirect::to("/admin/login").into_response());
    }
    if let Some(c) = jar.get(SESSION_COOKIE) {
        if let Ok(sid) = SessionId::from_str(c.value()) {
            let _ = session::logout(&app.db, sid).await;
        }
    }
    let jar = jar.add(clear_session_cookie(app.config.server.cookie_secure));
    // Render the login page with a "Signed out" confirmation.
    let flash = Flash {
        kind: FlashKind::Info,
        text: lang.strings().signed_out_flash.into(),
    };
    Ok((
        jar,
        Html(render_login(Some(flash), None, lang, false)),
    )
        .into_response())
}

// ---------- dashboard ----------

pub async fn dashboard(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<DashboardQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let admin = users::get(&app.db, admin_id).await.map_err(|e| HttpError::html(CoreError::from(e)))?;
    let users_n = users::list(&app.db).await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let clients_n = clients::list(&app.db).await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    // Range comes from ?range=24h|7d|30d. Unknown / missing falls
    // back to the default (Last7Days).
    let range = q
        .range
        .as_deref()
        .and_then(sui_id_core::dashboard::SparklineRange::from_query)
        .unwrap_or_default();
    let activity = sui_id_core::dashboard::login_activity(&app.db, &app.clock, range).await
        .map_err(HttpError::html)?;

    // Format bucket labels per the bucket size — 1-hour buckets get
    // a hour-precision label, day buckets get a date-only label.
    let label_fmt = match range {
        sui_id_core::dashboard::SparklineRange::Last24Hours => "%Y-%m-%d %H:%M",
        _ => "%Y-%m-%d",
    };
    let buckets: Vec<sui_id_web::DashboardSparkBucket> = activity
        .buckets
        .iter()
        .map(|b| sui_id_web::DashboardSparkBucket {
            label: b.bucket_start.format(label_fmt).to_string(),
            success: b.success,
            failure: b.failure,
        })
        .collect();

    let range_options = sui_id_core::dashboard::SparklineRange::all()
        .iter()
        .map(|r| (r.as_query().to_string(), r.label_ja().to_string()))
        .collect::<Vec<_>>();

    let sparkline = sui_id_web::DashboardSparkline {
        active_range_query: range.as_query().to_string(),
        range_options,
        total_success: activity.total_success,
        total_failure: activity.total_failure,
        buckets,
    };

    let session_count = sui_id_store::repos::sessions::count_active_total(&app.db)
        .await.unwrap_or(0);
    // HibpMode: Off = show warning; anything else = configured
    let hibp_is_off = sui_id_store::repos::server_settings::get(&app.db).await
        .map(|s| matches!(s.hibp_mode, sui_id_store::models::HibpMode::Off))
        .unwrap_or(true);  // assume Off if settings missing
    let smtp_configured = sui_id_store::repos::smtp_config::get(&app.db)
        .await.map(|o| o.is_some()).unwrap_or(false);

    // RFC 043: fetch last 5 important audit events for the dashboard card.
    let audit_rows = sui_id_store::repos::audit::recent_important(&app.db, 5)
        .await.unwrap_or_default();
    // Best-effort: resolve actor IDs to usernames.
    let actor_ids: Vec<_> = audit_rows.iter()
        .filter_map(|r| r.actor)
        .collect::<std::collections::HashSet<_>>()
        .into_iter().collect();
    let actor_map = sui_id_store::repos::users::resolve_usernames(&app.db, &actor_ids)
        .await.unwrap_or_default();
    let recent_important: Vec<sui_id_web::DashboardEventRow> = audit_rows
        .into_iter()
        .map(|r| sui_id_web::DashboardEventRow {
            at: r.at,
            action: r.action,
            actor_label: r.actor
                .and_then(|id| actor_map.get(&id).cloned())
                .unwrap_or_default(),
            result: r.result,
        })
        .collect();

    let data = DashboardData {
        admin_username: admin.username,
        user_count: users_n,
        client_count: clients_n,
        active_session_count: session_count,
        issuer: app.issuer().to_owned(),
        sparkline,
        warn_smtp_not_configured: !smtp_configured,
        warn_hibp_off: hibp_is_off,
        warn_cookie_insecure: !app.config.server.cookie_secure,
        recent_important,
    };
    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_dashboard(data, None, app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, serde::Deserialize, Default)]
pub struct DashboardQuery {
    /// `?range=24h` / `?range=7d` / `?range=30d`. Anything else
    /// (or absence) means "use the default".
    pub range: Option<String>,
}

// ---------- users ----------

pub async fn users_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let admin = users::get(&app.db, admin_id).await.map_err(|e| HttpError::html(CoreError::from(e)))?;
    let rows = admin_uc::list_users(&app.db, admin_id).await.map_err(HttpError::html)?;
    let mut summaries = Vec::with_capacity(rows.len());
    for r in rows {
        let mfa_enabled =
            sui_id_core::mfa::is_mfa_enabled(&app.db, r.id).await.unwrap_or(false);
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
    let resp = Html(render_users(summaries, None, admin.username, token.clone(), app.is_dev_mode, lang)).into_response();
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
            sui_id_store::repos::server_settings::get(&app.db).await
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
        },
    ).await;

    match create_result {
        Ok(_) => Ok(Redirect::to("/admin/users").into_response()),
        Err(CoreError::Conflict(msg)) => {
            // Duplicate username: stay on the users page and show the
            // conflict message in-line rather than rendering a bare error page.
            let rows = admin_uc::list_users(&app.db, admin_id).await.map_err(HttpError::html)?;
            let admin = users::get(&app.db, admin_id).await.map_err(|e| HttpError::html(CoreError::from(e)))?;
            let token = crate::csrf::ensure_token(&jar);
            let mut summaries = Vec::with_capacity(rows.len());
            for r in rows {
                let mfa_enabled = sui_id_core::mfa::is_mfa_enabled(&app.db, r.id).await.unwrap_or(false);
                summaries.push(UserSummary {
                    id: r.id, username: r.username, display_name: r.display_name,
                    is_admin: r.is_admin, is_disabled: r.is_disabled, is_deleted: r.is_deleted,
                    mfa_enabled, created_at: r.created_at,
                });
            }
            let flash = Flash { kind: FlashKind::Error, text: msg };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
            let resp = Html(render_users(summaries, Some(flash), admin.username, token.clone(), app.is_dev_mode, lang)).into_response();
            Ok((axum::http::StatusCode::CONFLICT, with_csrf_cookie(resp, &app, &token)).into_response())
        }
        Err(e) => Err(HttpError::html(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct DisableForm {
    pub disabled: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    /// Optional reason for disabling the user (RFC 045). Stored in audit note.
    #[serde(default)]
    pub reason: String,
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
    let reason_opt = if form.reason.trim().is_empty() {
        None
    } else {
        Some(form.reason.trim().to_string())
    };
    admin_uc::set_user_disabled(&app.db, admin_id, target, value, reason_opt)
        .await.map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/users").into_response())
}

/// `_csrf`-only body: confirmation-style POSTs that have no other fields.
#[derive(Debug, Deserialize, Default)]
pub struct CsrfOnlyForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// Body for dangerous-operation POSTs that require both a CSRF token and an
/// explicit `_confirmed=1` field (RFC 030). The confirmation screen supplies
/// this field; direct-POST attacks without it are rejected.
#[derive(Debug, Deserialize, Default)]
pub struct ConfirmedForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    #[serde(rename = "_confirmed", default)]
    pub confirmed: String,
}

pub async fn users_delete(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<ConfirmedForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/users").await
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    admin_uc::delete_user(&app.db, admin_id, target).await.map_err(HttpError::html)?;
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
    Form(form): Form<ConfirmedForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/users").await
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    admin_uc::admin_reset_mfa(&app.db, admin_id, target).await.map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/users").into_response())
}



pub async fn users_detail_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
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
    let totp = sui_id_store::repos::user_totp::get(&app.db, target).await
        .unwrap_or(None);
    let totp_enabled = totp.map(|r| r.enabled).unwrap_or(false);
    let passkey_count = sui_id_store::repos::user_webauthn_credentials::count_for_user(
        &app.db, target
    ).await.unwrap_or(0);

    // Active sessions
    let sessions_raw = sui_id_store::repos::sessions::list_active_for_user(
        &app.db, target
    ).await.unwrap_or_default();

    let sessions: Vec<UserDetailSession> = sessions_raw.into_iter().map(|s| {
        let factors = s.auth_methods.iter()
            .map(|m| format!("{m:?}").to_lowercase())
            .collect::<Vec<_>>()
            .join(", ");
        UserDetailSession {
            started: s.created_at,
            expires: s.expires_at,
            factors: if factors.is_empty() { "password".into() } else { factors },
        }
    }).collect();

    // Recent audit events (actor or target)
    let audit_rows = sui_id_store::repos::audit::recent_for_user(
        &app.db, target, 20
    ).await.unwrap_or_default();

    let recent_audit: Vec<AuditLogEntryDto> = audit_rows.into_iter().map(|r| {
        AuditLogEntryDto {
            at: r.at, actor: r.actor, action: r.action,
            target: r.target, result: r.result, note: r.note,
        }
    }).collect();

    let token = crate::csrf::ensure_token(&jar);
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let data = UserDetailData {
        user_id: id,
        username: user.username,
        display_name: user.display_name,
        email: user.email,
        is_admin: user.is_admin,
        is_disabled: user.is_disabled,
        totp_enabled,
        passkey_count,
        sessions,
        recent_audit,
        dev_mode: app.is_dev_mode,
        csrf_token: token.clone(),
    };

    let resp = Html(render_user_detail(data, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- dangerous-op confirmation GET handlers (RFC 030) ----------

pub async fn users_disable_confirm_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let user = users::get(&app.db, target).await
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmDisableData {
        user_id: id,
        username: user.username,
        is_disabled: user.is_disabled,
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_disable_user(data, app.is_dev_mode, lang))
        .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn users_delete_confirm_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let return_to = format!("/admin/users/{id}/delete-confirm");
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let user = users::get(&app.db, target).await
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmDeleteUserData {
        user_id: id,
        username: user.username,
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_delete_user(data, app.is_dev_mode, lang))
        .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn users_mfa_reset_confirm_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let return_to = format!("/admin/users/{id}/mfa-reset-confirm");
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    let user = users::get(&app.db, target).await
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let data = ConfirmResetMfaData {
        user_id: id,
        username: user.username,
        csrf_token: token.clone(),
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let resp = Html(render_confirm_reset_mfa(data, app.is_dev_mode, lang))
        .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

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

pub async fn signing_keys_delete_confirm_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
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
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<DisableForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let value = matches!(form.disabled.as_str(), "true" | "on" | "1");
    admin_uc::set_client_disabled(&app.db, &app.clock, admin_id, target, value, &app.caches).await.map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/clients").into_response())
}

pub async fn clients_delete(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/clients").await
    {
        return Ok(redirect);
    }
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    admin_uc::delete_client(&app.db, admin_id, target, &app.caches).await.map_err(HttpError::html)?;
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

#[derive(Debug, serde::Deserialize, Default)]
pub struct AuditQuery {
    #[serde(default)]
    pub q: String,
}

pub async fn audit_get(
    state_ext: AppStateExt,
    CurrentAdmin(_): CurrentAdmin,
    jar: CookieJar,
    axum::extract::Query(query): axum::extract::Query<AuditQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let filter = if query.q.is_empty() { None } else { Some(query.q.clone()) };
    let entries = audit::recent_filtered(&app.db, 200, filter.clone()).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let chain = audit::verify_chain_tail(&app.db, 500).await
        .unwrap_or(sui_id_store::repos::audit::ChainVerifyReport {
            checked: 0, broken_at_seq: None, legacy_unhashed: 0
        });
    let chain_ok = chain.broken_at_seq.is_none();
    let dtos: Vec<AuditLogEntryDto> = entries
        .into_iter()
        .map(|r| AuditLogEntryDto {
            at: r.at, actor: r.actor, action: r.action,
            target: r.target, result: r.result, note: r.note,
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_audit(
        dtos, chain_ok, filter, None, app.is_dev_mode, sui_id_i18n::Locale::Ja,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn audit_csv_get(
    state_ext: AppStateExt,
    CurrentAdmin(_): CurrentAdmin,
    axum::extract::Query(query): axum::extract::Query<AuditQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let filter = if query.q.is_empty() { None } else { Some(query.q.clone()) };
    let entries = audit::recent_filtered(&app.db, 2000, filter).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    let mut csv = String::from("when,actor,action,target,result,note\n");
    for r in entries {
        fn esc(s: &str) -> String {
            format!("\"{}\"", s.replace('"', "\"\"\""))
        }
        let actor_str = r.actor.map(|id| id.to_string()).unwrap_or_default();
        let target_str = r.target.unwrap_or_default();
        let note_str = r.note.unwrap_or_default();
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            r.at.to_rfc3339(),
            esc(&actor_str),
            esc(&r.action),
            esc(&target_str),
            esc(&r.result),
            esc(&note_str),
        ));
    }
    let mut resp = axum::response::Response::new(axum::body::Body::from(csv));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        axum::http::HeaderValue::from_static("attachment; filename=audit.csv"),
    );
    Ok(resp)
}

// ---------- signing keys ----------

pub async fn signing_keys_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let rows = admin_uc::list_signing_keys(&app.db, admin_id).await.map_err(HttpError::html)?;
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
    let resp = Html(render_signing_keys(summaries, None, token.clone(), app.is_dev_mode, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn signing_keys_rotate(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/signing-keys").await
    {
        return Ok(redirect);
    }
    admin_uc::rotate_signing_key(&app.db, &app.clock, app.config.storage.key_file.to_str().unwrap_or_default(), admin_id, &app.caches).await.map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/signing-keys").into_response())
}

pub async fn signing_keys_delete(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/signing-keys").await
    {
        return Ok(redirect);
    }
    let target = sui_id_shared::ids::SigningKeyId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid signing key id".into())))?;
    admin_uc::delete_signing_key(&app.db, &app.clock, admin_id, target, &app.caches).await.map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/signing-keys").into_response())
}

fn render_qr_svg(uri: &str) -> String {
    use qrcode::render::svg;
    use qrcode::QrCode;
    match QrCode::new(uri.as_bytes()) {
        Ok(code) => code
            .render::<svg::Color>()
            .min_dimensions(220, 220)
            .quiet_zone(true)
            .build(),
        Err(_) => format!(
            "<p class=\"muted\">QR rendering failed; use the secret key below instead.</p>"
        ),
    }
}

/// Public re-export of the QR rendering helper for the
/// /me/security/mfa enrollment handler (RFC 055).
pub fn render_qr_svg_pub(uri: &str) -> String {
    render_qr_svg(uri)
}

// ---------- WebAuthn login challenge ----------

use axum::Json;

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
    let pending = sui_id_store::repos::login_pending_mfa::get(&app.db, pending_mfa_id).await
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
    ).await
    .map_err(HttpError::html)?;
    let cookie = crate::handlers::webauthn_pending_cookie(
        started.pending_id.to_string(),
        app.config.server.cookie_secure,
    );
    let jar = jar.add(cookie);
    let challenge_value: serde_json::Value =
        serde_json::from_str(&started.challenge_json)
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
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;

    let pending_mfa_id = jar
        .get(crate::handlers::PENDING_MFA_COOKIE)
        .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok())
        .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
    let webauthn_pending_id = jar
        .get(crate::handlers::WEBAUTHN_PENDING_COOKIE)
        .and_then(|c| c.value().parse::<sui_id_shared::ids::WebauthnPendingId>().ok())
        .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
    let pending = sui_id_store::repos::login_pending_mfa::get(&app.db, pending_mfa_id).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?
        .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
    if pending.expires_at < app.clock.now() {
        return Err(HttpError::html(CoreError::Unauthenticated));
    }
    let credential: webauthn_rs::prelude::PublicKeyCredential =
        serde_json::from_str(&form.credential).map_err(|_| {
            HttpError::html(CoreError::BadRequest("malformed credential JSON".into()))
        })?;
    sui_id_core::webauthn::finish_authentication(
        &app.db,
        &app.clock,
        app.issuer(),
        webauthn_pending_id,
        pending.user_id,
        &credential,
    ).await
    .map_err(HttpError::html)?;
    let session = sui_id_core::mfa::verify_pending_webauthn(
        &app.db,
        &app.clock,
        pending_mfa_id,
        pending.user_id,
    ).await
    .map_err(HttpError::html)?;
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(session.user_id),
            action: "auth.mfa.success".into(),
            target: Some(session.user_id.to_string()),
            result: "ok".into(),
            note: Some("webauthn".into()),
        },
    ).await;
    let next = jar
        .get(PENDING_MFA_NEXT_COOKIE)
        .map(|c| c.value().to_owned())
        .filter(|s| s.starts_with('/'))
        .unwrap_or_else(|| "/admin".into());
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

#[allow(dead_code)]
fn _silence_state(_: &CurrentUser) {}
#[allow(dead_code)]
fn _silence_state2() -> Option<bool> {
    let _ = state::is_initialized;
    None
}

/// POST /admin/clients/{id}/rotate-secret — rotate client secret (RFC 047).
///
/// The new plaintext secret is returned in the redirect response via a
/// dedicated query parameter so the client edit page can display it once.
/// After the page renders the secret, the query parameter is gone; the
/// plaintext is never stored in any server-side state.
pub async fn clients_rotate_secret_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/clients").await
    {
        return Ok(redirect);
    }
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let new_secret = admin_uc::rotate_client_secret(
        &app.db, &app.clock, admin_id, target
    ).await.map_err(HttpError::html)?;
    // Redirect to edit page with the new secret in the query string.
    // The secret is URL-encoded; the edit page displays it once and the
    // browser history entry is replaced by the subsequent navigation.
    let encoded = percent_encoding::utf8_percent_encode(
        &new_secret, percent_encoding::NON_ALPHANUMERIC
    ).to_string();
    Ok(Redirect::to(&format!("/admin/clients/{id}/edit?rotated_secret={encoded}")).into_response())
}
