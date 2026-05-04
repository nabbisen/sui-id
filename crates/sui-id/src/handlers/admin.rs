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
    pages::DashboardData, render_audit, render_clients, render_dashboard, render_login,
    render_signing_keys, render_users, Flash, FlashKind,
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
            if session::resolve(&app.db, &app.clock, sid).is_ok() {
                return Ok(Redirect::to("/admin").into_response());
            }
        }
    }
    Ok(Html(render_login(None, None, lang)).into_response())
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
    ) {
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
                (StatusCode::UNAUTHORIZED, Html(render_login(Some(flash), next, lang)))
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
    let has_passkey = jar
        .get(crate::handlers::PENDING_MFA_COOKIE)
        .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok())
        .and_then(|pid| sui_id_store::repos::login_pending_mfa::get(&app.db, pid).ok().flatten())
        .map(|row| {
            sui_id_core::webauthn::has_credentials(&app.db, row.user_id).unwrap_or(false)
        })
        .unwrap_or(false);
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
    match sui_id_core::mfa::verify_pending(&app.db, &app.clock, pending_id, &form.code) {
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
            );
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
            );
            let has_passkey = jar
                .get(crate::handlers::PENDING_MFA_COOKIE)
                .and_then(|c| c.value().parse::<sui_id_shared::ids::PendingMfaId>().ok())
                .and_then(|pid| {
                    sui_id_store::repos::login_pending_mfa::get(&app.db, pid)
                        .ok()
                        .flatten()
                })
                .map(|row| {
                    sui_id_core::webauthn::has_credentials(&app.db, row.user_id).unwrap_or(false)
                })
                .unwrap_or(false);
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
    axum::extract::Query(q): axum::extract::Query<DashboardQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let admin = users::get(&app.db, admin_id).map_err(|e| HttpError::html(CoreError::from(e)))?;
    let users_n = users::list(&app.db)
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let clients_n = clients::list(&app.db)
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    // Range comes from ?range=24h|7d|30d. Unknown / missing falls
    // back to the default (Last7Days).
    let range = q
        .range
        .as_deref()
        .and_then(sui_id_core::dashboard::SparklineRange::from_query)
        .unwrap_or_default();
    let activity = sui_id_core::dashboard::login_activity(&app.db, &app.clock, range)
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

    let data = DashboardData {
        admin_username: admin.username,
        user_count: users_n,
        client_count: clients_n,
        issuer: app.issuer().to_owned(),
        sparkline,
    };
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_dashboard(data, None)).into_response();
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
    let admin = users::get(&app.db, admin_id).map_err(|e| HttpError::html(CoreError::from(e)))?;
    let rows = admin_uc::list_users(&app.db, admin_id).map_err(HttpError::html)?;
    let summaries: Vec<UserSummary> = rows
        .into_iter()
        .map(|r| {
            // We tolerate MFA-lookup errors per row by treating them as
            // "MFA off" — the worst that does is hide the Reset button,
            // which is recoverable. Failing the whole list page on a
            // single read error would be hostile.
            let mfa_enabled =
                sui_id_core::mfa::is_mfa_enabled(&app.db, r.id).unwrap_or(false);
            UserSummary {
                id: r.id,
                username: r.username,
                display_name: r.display_name,
                is_admin: r.is_admin,
                is_disabled: r.is_disabled,
                is_deleted: r.is_deleted,
                mfa_enabled,
                created_at: r.created_at,
            }
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
    admin_uc::create_user(
        &app.db,
        &app.clock,
        admin_id,
        CreateUserSpec {
            username: form.username.trim(),
            password: &form.password,
            display_name: display,
            email,
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
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/users")
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    admin_uc::delete_user(&app.db, admin_id, target).map_err(HttpError::html)?;
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
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/users")
    {
        return Ok(redirect);
    }
    let target = UserId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid user id".into())))?;
    admin_uc::admin_reset_mfa(&app.db, admin_id, target).map_err(HttpError::html)?;
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
            allowed_scopes: r.allowed_scopes,
            post_logout_redirect_uris: r.post_logout_redirect_uris,
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
    // Default policy: openid + profile if the operator left the field
    // blank. Empty-after-trim is also accepted as "permit any" but only
    // when the operator explicitly types whitespace; the form's default
    // value is a sensible policy.
    let raw_scopes = form.allowed_scopes.trim();
    let allowed_scopes = if raw_scopes.is_empty() {
        "openid profile"
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
            allowed_scopes: r.allowed_scopes,
            post_logout_redirect_uris: r.post_logout_redirect_uris,
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
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/clients")
    {
        return Ok(redirect);
    }
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    admin_uc::delete_client(&app.db, admin_id, target).map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/clients").into_response())
}

pub async fn clients_edit_get(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id): CurrentAdmin,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let target = ClientId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;
    let row = admin_uc::get_client(&app.db, admin_id, target).map_err(HttpError::html)?;
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
        },
        None,
        token.clone(),
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
    admin_uc::update_client_basic(&app.db, admin_id, target, form.name.trim(), &uris)
        .map_err(HttpError::html)?;
    admin_uc::set_client_allowed_scopes(
        &app.db,
        admin_id,
        target,
        form.allowed_scopes.trim(),
    )
    .map_err(HttpError::html)?;
    admin_uc::set_client_post_logout_redirect_uris(
        &app.db,
        admin_id,
        target,
        &post_logout_uris,
    )
    .map_err(HttpError::html)?;
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
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/signing-keys")
    {
        return Ok(redirect);
    }
    admin_uc::rotate_signing_key(&app.db, &app.clock, admin_id).map_err(HttpError::html)?;
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
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/signing-keys")
    {
        return Ok(redirect);
    }
    let target = sui_id_shared::ids::SigningKeyId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid signing key id".into())))?;
    admin_uc::delete_signing_key(&app.db, admin_id, target).map_err(HttpError::html)?;
    Ok(Redirect::to("/admin/signing-keys").into_response())
}

// ---------- profile / MFA enrolment ----------

pub async fn profile_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let user = users::get(&app.db, user_id).map_err(|e| HttpError::html(CoreError::from(e)))?;
    let totp_enabled = sui_id_store::repos::user_totp::get(&app.db, user_id)
        .map_err(|e| HttpError::html(CoreError::from(e)))?
        .map(|r| r.enabled)
        .unwrap_or(false);
    let passkeys = sui_id_core::webauthn::list_for_user(&app.db, user_id)
        .map_err(HttpError::html)?
        .into_iter()
        .map(|d| sui_id_web::PasskeyDescriptor {
            id: d.id.to_string(),
            nickname: d.nickname,
            created_at: d.created_at,
            last_used_at: d.last_used_at,
        })
        .collect();
    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(sui_id_web::render_profile(
        sui_id_web::ProfileData {
            username: user.username,
            totp_enabled,
            fresh_recovery_codes: None,
            passkeys,
            preferred_lang: user.preferred_lang.clone(),
        },
        None,
        token.clone(),
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// Update the signed-in user's preferred display language.
///
/// Form fields:
///   - `_csrf`: standard CSRF token
///   - `lang`: BCP-47 tag, or empty string to clear (= "follow
///     browser default", which un-sets the user-tier of the
///     locale resolution chain)
///
/// On success, sets the `sui_id_lang` cookie to the same value
/// (cleared when `lang` is empty) so non-authenticated pages
/// (e.g. logout, forgot-password) immediately reflect the choice
/// too. The cookie is `SameSite=Lax`, **not** `HttpOnly` (it is
/// not sensitive — the same value is in the form select), with a
/// long max-age (one year) since language preference is rarely
/// changed.
pub async fn profile_lang_post(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Form(form): Form<ProfileLangForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;

    // Validate the tag is one we recognise — or empty (=clear).
    let lang_to_store: Option<&str> = if form.lang.trim().is_empty() {
        None
    } else if sui_id_i18n::Locale::parse(&form.lang).is_some() {
        Some(form.lang.as_str())
    } else {
        return Err(HttpError::html(CoreError::BadRequest(
            "unknown language tag".into(),
        )));
    };

    let now = app.clock.now();
    sui_id_store::repos::users::set_preferred_lang(&app.db, user_id, lang_to_store, now)
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    // Mirror the choice into the lang cookie so pages without
    // an authenticated user pick up the change immediately.
    let cookie = {
        let mut c = axum_extra::extract::cookie::Cookie::new(
            crate::handlers::LANG_COOKIE,
            lang_to_store.unwrap_or("").to_owned(),
        );
        c.set_path("/");
        c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
        c.set_secure(app.config.server.cookie_secure);
        if lang_to_store.is_some() {
            c.set_max_age(cookie::time::Duration::days(365));
        } else {
            // "Browser default" — clear the cookie so resolution
            // skips the cookie tier and reads Accept-Language.
            c.set_max_age(cookie::time::Duration::seconds(0));
        }
        c
    };
    let jar = jar.add(cookie);
    Ok((jar, Redirect::to("/admin/profile")).into_response())
}

#[derive(Debug, Deserialize)]
pub struct ProfileLangForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    #[serde(default)]
    pub lang: String,
}

pub async fn profile_mfa_enroll_start(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<crate::handlers::admin::CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let user = users::get(&app.db, user_id).map_err(|e| HttpError::html(CoreError::from(e)))?;
    let ticket = sui_id_core::mfa::start_enrollment(
        &app.db,
        app.issuer(),
        user_id,
        &user.username,
    )
    .map_err(HttpError::html)?;

    // Render QR as SVG via the qrcode crate.
    let qr_svg = render_qr_svg(&ticket.otpauth_uri);
    let secret_b32 = sui_id_core::totp::base32_encode(&ticket.secret);
    let otpauth_uri = ticket.otpauth_uri;
    // The raw secret bytes drop with `ticket` here. sui_id_core::mfa::
    // start_enrollment keeps no caller-visible copy beyond what it
    // returns in the ticket.
    drop(ticket.secret);

    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(sui_id_web::render_mfa_setup(
        sui_id_web::MfaSetupData {
            otpauth_uri,
            qr_svg,
            secret_b32,
        },
        None,
        token.clone(),
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]
pub struct MfaConfirmForm {
    pub code: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn profile_mfa_enroll_confirm(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<MfaConfirmForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let code: u32 = form
        .code
        .trim()
        .parse()
        .map_err(|_| HttpError::html(CoreError::BadRequest("verification code must be 6 digits".into())))?;
    let codes = sui_id_core::mfa::confirm_enrollment(&app.db, &app.clock, user_id, code)
        .map_err(HttpError::html)?;
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(user_id),
            action: "mfa.enable".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: None,
        },
    );
    let user = users::get(&app.db, user_id).map_err(|e| HttpError::html(CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let t = lang.strings();
    let resp = Html(sui_id_web::render_profile(
        sui_id_web::ProfileData {
            username: user.username,
            totp_enabled: true,
            fresh_recovery_codes: Some(codes),
            passkeys: sui_id_core::webauthn::list_for_user(&app.db, user_id)
                .map_err(HttpError::html)?
                .into_iter()
                .map(|d| sui_id_web::PasskeyDescriptor {
                    id: d.id.to_string(),
                    nickname: d.nickname,
                    created_at: d.created_at,
                    last_used_at: d.last_used_at,
                })
                .collect(),
            preferred_lang: user.preferred_lang.clone(),
        },
        Some(Flash {
            kind: FlashKind::Info,
            text: t.profile_mfa_enrolled_flash.into(),
        }),
        token.clone(),
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn profile_mfa_disable(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Form(form): Form<crate::handlers::admin::CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    sui_id_core::mfa::disable(&app.db, user_id).map_err(HttpError::html)?;
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(user_id),
            action: "mfa.disable".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: None,
        },
    );
    Ok(Redirect::to("/admin/profile").into_response())
}

pub async fn profile_mfa_regenerate_recovery(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<crate::handlers::admin::CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let codes = sui_id_core::mfa::regenerate_recovery_codes(&app.db, user_id)
        .map_err(HttpError::html)?;
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(user_id),
            action: "mfa.recovery_codes_regenerate".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: None,
        },
    );
    let user = users::get(&app.db, user_id).map_err(|e| HttpError::html(CoreError::from(e)))?;
    let token = crate::csrf::ensure_token(&jar);
    let t = lang.strings();
    let resp = Html(sui_id_web::render_profile(
        sui_id_web::ProfileData {
            username: user.username,
            totp_enabled: true,
            fresh_recovery_codes: Some(codes),
            passkeys: sui_id_core::webauthn::list_for_user(&app.db, user_id)
                .map_err(HttpError::html)?
                .into_iter()
                .map(|d| sui_id_web::PasskeyDescriptor {
                    id: d.id.to_string(),
                    nickname: d.nickname,
                    created_at: d.created_at,
                    last_used_at: d.last_used_at,
                })
                .collect(),
            preferred_lang: user.preferred_lang.clone(),
        },
        Some(Flash {
            kind: FlashKind::Info,
            text: t.profile_recovery_regenerated_flash.into(),
        }),
        token.clone(),
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
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

// ---------- WebAuthn / passkey enrolment ----------

use axum::Json;

#[derive(Debug, Deserialize)]
pub struct WebauthnRegisterStartForm {
    pub nickname: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// Start a passkey-registration ceremony. Returns the
/// `CreationChallengeResponse` JSON for `navigator.credentials.create()`
/// and sets a `sui_id_webauthn_pending` cookie that the matching
/// completion endpoint reads. Nickname is buffered in a query param for
/// the completion call (we keep server state minimal — the pending row
/// already holds the in-flight ceremony state).
pub async fn webauthn_register_start(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Form(form): Form<WebauthnRegisterStartForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let started = sui_id_core::webauthn::start_registration(
        &app.db,
        &app.clock,
        app.issuer(),
        user_id,
    )
    .map_err(HttpError::html)?;
    // Stash the nickname in a second cookie so the completion call
    // can pick it up without requiring the JS to echo it back.
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
    // Return the challenge JSON as application/json so the browser's
    // fetch() can parse it directly.
    let challenge_value: serde_json::Value =
        serde_json::from_str(&started.challenge_json).map_err(|_| HttpError::html(CoreError::Internal))?;
    Ok((jar, Json(challenge_value)).into_response())
}

#[derive(Debug, Deserialize)]
pub struct WebauthnRegisterCompleteForm {
    /// JSON-stringified `RegisterPublicKeyCredential`.
    pub credential: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn webauthn_register_complete(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Form(form): Form<WebauthnRegisterCompleteForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let pending_value = jar
        .get(crate::handlers::WEBAUTHN_PENDING_COOKIE)
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("no pending ceremony".into())))?
        .value()
        .to_owned();
    let pending_id: sui_id_shared::ids::WebauthnPendingId =
        pending_value.parse().map_err(|_| {
            HttpError::html(CoreError::BadRequest("malformed pending id".into()))
        })?;
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
    );
    let jar = jar
        .add(crate::handlers::clear_webauthn_pending_cookie(
            app.config.server.cookie_secure,
        ));
    Ok((jar, Redirect::to("/admin/profile")).into_response())
}

#[derive(Debug, Deserialize)]
pub struct WebauthnDeleteForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn webauthn_delete(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Path(cred_id): Path<String>,
    Form(form): Form<WebauthnDeleteForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let id = cred_id.parse::<sui_id_shared::ids::WebauthnCredentialId>().map_err(|_| {
        HttpError::html(CoreError::BadRequest("invalid credential id".into()))
    })?;
    sui_id_core::webauthn::delete(&app.db, user_id, id).map_err(HttpError::html)?;
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(user_id),
            action: "webauthn.credential.delete".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: None,
        },
    );
    Ok(Redirect::to("/admin/profile").into_response())
}

// ---------- WebAuthn login challenge ----------

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
    let pending = sui_id_store::repos::login_pending_mfa::get(&app.db, pending_mfa_id)
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
    )
    .map_err(HttpError::html)?;
    let session = sui_id_core::mfa::verify_pending_webauthn(
        &app.db,
        &app.clock,
        pending_mfa_id,
        pending.user_id,
    )
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
    );
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
