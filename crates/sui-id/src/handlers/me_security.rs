//! `/me/security` — self-service security surface.
//!
//! This is the *user-facing* security control panel. The admin pages
//! (`/admin/users/*`) are for someone managing other people's accounts;
//! this surface is for someone managing their own:
//! seeing where they're signed in, revoking sessions they don't
//! recognise, enrolling MFA, registering passkeys, changing their
//! password, and choosing a UI language.
//!
//! Routes (mounted from `router.rs`):
//!
//! ### Tab views
//! - `GET  /me/security` — redirect to overview
//! - `GET  /me/security/overview` — landing tab
//! - `GET  /me/security/mfa` — TOTP + passkeys summary
//! - `GET  /me/security/passkeys` — passkey list + register
//! - `GET  /me/security/sessions` — active sessions
//! - `GET  /me/security/language` — language preference
//!
//! ### Sessions
//! - `POST /me/security/sessions/{id}/revoke`
//! - `POST /me/security/sessions/revoke-all-others`
//!
//! ### Password
//! - `GET+POST /me/security/password`
//!
//! ### Language
//! - `POST /me/security/language`
//!
//! ### MFA mutative (RFC 055, v0.44.0)
//! - `POST /me/security/mfa/enroll/start`
//! - `POST /me/security/mfa/enroll/confirm`
//! - `POST /me/security/mfa/disable`
//! - `POST /me/security/mfa/recovery-codes/regenerate`
//!
//! ### Passkey mutative (RFC 055, v0.44.0)
//! - `POST /me/security/passkeys/register/start`
//! - `POST /me/security/passkeys/register/complete`
//! - `POST /me/security/passkeys/{id}/rename`
//! - `POST /me/security/passkeys/{id}/delete`
//!
//! Prior to v0.44.0 the MFA and passkey mutative routes lived under
//! `/admin/profile/*` and were used by a parallel `render_profile`
//! page. RFC 055 consolidated everything onto `/me/security/*`.

use crate::handlers::admin::with_csrf_cookie;
use crate::handlers::{enforce_csrf, AppStateExt, CurrentUser};
use crate::{csrf, errors::HttpError};
use axum::extract::{Form, Path, State};
use axum::http::header::LOCATION;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use std::str::FromStr;
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::SessionId;
use sui_id_store::repos::{audit, sessions, user_totp, users};

const SESSION_COOKIE: &str = "sui_id_session";

/// How many recent activity rows to show. Small enough to keep the
/// page easy to scan; users who want a deeper history have a
/// reason to talk to the operator (and the operator has the full
/// `/admin/audit`).
const RECENT_EVENT_LIMIT: i64 = 30;

#[derive(Debug, Deserialize)]
pub struct CsrfOnlyForm {
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

#[derive(Debug, Deserialize)]
pub struct RevokeAllOthersForm {
    #[serde(rename = "_csrf")]
    pub csrf: String,
    /// The session id of the request itself, posted from a hidden
    /// field. We don't trust it on its own — we cross-check against
    /// the cookie — but having it in the form means the keep-set is
    /// explicit and auditable.
    pub current_session: String,
}

pub async fn page_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    // The `CurrentUser` extractor has already resolved the cookie,
    // but we need the raw cookie value to identify *which* session
    // is the one issuing this request. There can be more than one
    // active session for the same user, and we want to mark the
    // current row as "current".
    let raw_session = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
    let current_session_id = SessionId::from_str(&raw_session)
        .map_err(|_| HttpError::html(CoreError::Unauthenticated))?;

    let user = users::get(&app.db, user_id).await.map_err(|e| HttpError::html(CoreError::from(e)))?;

    let totp_enabled = user_totp::get(&app.db, user_id).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?
        .map(|r| r.enabled)
        .unwrap_or(false);

    let passkey_count = sui_id_core::webauthn::list_for_user(&app.db, user_id).await
        .map_err(HttpError::html)?
        .len();

    let session_rows =
        sessions::list_active_for_user(&app.db, user_id).await.map_err(|e| HttpError::html(e.into()))?;
    let mut sessions_view = Vec::with_capacity(session_rows.len());
    for s in session_rows {
        let auth_methods = describe_auth_methods(&s.auth_methods);
        let is_current = s.id == current_session_id;
        sessions_view.push(sui_id_web::MeSessionDescriptor {
            id: s.id.to_string(),
            created_at: s.created_at,
            expires_at: s.expires_at,
            auth_methods,
            is_current,
        });
    }

    let event_rows = audit::recent_for_user(&app.db, user_id, RECENT_EVENT_LIMIT).await
        .map_err(|e| HttpError::html(e.into()))?;
    let events_view: Vec<_> = event_rows
        .into_iter()
        .map(|e| sui_id_web::MeAuditEntry {
            at: e.at,
            action: e.action,
            result: e.result,
            note: e.note,
        })
        .collect();

    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_me_security(
        sui_id_web::MeSecurityData {
            username: user.username,
            is_admin: user.is_admin,
            totp_enabled,
            passkey_count,
            current_session_id: current_session_id.to_string(),
            sessions: sessions_view,
            recent_events: events_view,
        },
        flash_from_query(&jar),
        token.clone(),
        lang,
    );
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// Revoke a single session belonging to the current user.
///
/// The `id` path parameter is parsed and then cross-checked against
/// the database: the row must exist *and* belong to the current
/// user. We don't tell the caller which check failed — both refuse
/// with the same 404-shaped redirect, so there's no oracle for
/// guessing other people's session ids.
pub async fn revoke_one(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    Path(id): Path<String>,
    jar: CookieJar,
    Form(form): Form<CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;

    let target_id = SessionId::from_str(&id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid session id".into())))?;

    // Ownership check: pulling the session row and comparing user_id
    // is the simplest correct way. Skipping this would let a user
    // revoke another user's session by guessing the id.
    let row = match sessions::get(&app.db, target_id).await {
        Ok(r) => r,
        Err(sui_id_store::StoreError::NotFound) => {
            // Treat unknown ids the same as foreign ids — both
            // produce a redirect back to the page, no leak.
            return Ok(Redirect::to("/me/security?msg=unknown").into_response());
        }
        Err(e) => return Err(HttpError::html(e.into())),
    };
    if row.user_id != user_id {
        return Ok(Redirect::to("/me/security?msg=unknown").into_response());
    }

    sessions::revoke(&app.db, target_id).await.map_err(|e| HttpError::html(e.into()))?;

    // If the user just revoked their *own* current session, clear
    // the cookie so the next request is clean. They'll bounce to
    // the login page on the redirect target.
    let raw_session = jar.get(SESSION_COOKIE).map(|c| c.value().to_owned());
    if raw_session.as_deref() == Some(target_id.to_string().as_str()) {
        let mut clear = axum_extra::extract::cookie::Cookie::from(SESSION_COOKIE);
        clear.set_path("/");
        clear.make_removal();
        return Ok((
            jar.remove(clear),
            Redirect::to("/admin/login?msg=session_revoked"),
        )
            .into_response());
    }

    Ok(Redirect::to("/me/security?msg=session_revoked").into_response())
}

/// Revoke every session for the current user *except* the one
/// issuing the request. The "keep" id comes from the cookie, not
/// the form field — the form field is decorative; if it disagrees
/// we honour the cookie.
pub async fn revoke_all_others(
    state_ext: AppStateExt,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Form(form): Form<RevokeAllOthersForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;

    // Step-up gate: signing every other browser out at once is a
    // significant action; require a fresh strong-factor proof.
    // Users without MFA enrolled are passed through (the gate
    // is a no-op for them — see step_up::policy_for_session
    // doc comment for why a password re-prompt would buy
    // nothing). After step-up the operator is bounced back to
    // /me/security and can click the form again.
    if let Err(redirect) = crate::handlers::require_fresh_step_up(
        &app,
        &ctx,
        "/me/security",
    ).await {
        return Ok(redirect);
    }

    let user_id = ctx.user_id;
    let keep = ctx.session_id;

    let n = sessions::revoke_all_for_user_except(&app.db, user_id, keep).await
        .map_err(|e| HttpError::html(e.into()))?;

    // Audit: emit one row capturing how many sessions were swept.
    // The action name follows the dotted naming used elsewhere.
    let _ = audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(user_id),
            action: "auth.sessions.bulk_revoke_self".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: Some(format!("revoked {n} other session(s)")),
        },
    ).await;

    let target = if n == 0 {
        "/me/security?msg=no_other_sessions"
    } else {
        "/me/security?msg=others_revoked"
    };
    let mut resp = Response::default();
    *resp.status_mut() = axum::http::StatusCode::SEE_OTHER;
    resp.headers_mut().insert(
        LOCATION,
        target.parse().expect("static header value"),
    );
    Ok(resp)
}

// ---------- helpers ----------

fn describe_auth_methods(methods: &[sui_id_shared::AuthMethod]) -> String {
    use sui_id_shared::AuthMethod;
    if methods.is_empty() {
        return "—".into();
    }
    let parts: Vec<&str> = methods
        .iter()
        .map(|m| match m {
            AuthMethod::Pwd => "password",
            AuthMethod::Totp => "TOTP",
            AuthMethod::RecoveryCode => "recovery code",
            AuthMethod::Webauthn => "passkey",
        })
        .collect();
    parts.join(" + ")
}

/// Translate a `?msg=...` query value into a flash. We deliberately
/// keep the set of recognised messages closed — a free-form
/// query-string value would be reflected XSS.
fn flash_from_query(jar: &CookieJar) -> Option<sui_id_web::Flash> {
    // We don't actually have direct access to the query string from
    // the cookie jar — the caller would have to pass it in. For
    // simplicity in this first cut, we don't surface flashes via
    // query strings here; the redirects above are mostly
    // navigational. A subsequent revision can add a `Query<MsgQ>`
    // extractor and translate codes to localised messages.
    let _ = jar;
    None
}

// ---------- password change (v0.19.0) ----------

#[derive(Debug, Deserialize)]
pub struct PasswordChangeForm {
    #[serde(rename = "_csrf")]
    pub csrf: String,
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
    /// Checkbox value. Browsers send the field only when checked,
    /// so the option is presence-detected. Any non-empty string
    /// means "yes, sweep my other sessions and refresh tokens".
    #[serde(default)]
    pub revoke_others: Option<String>,
}

pub async fn password_change_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let user = users::get(&app.db, user_id).await.map_err(|e| HttpError::html(CoreError::from(e)))?;
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_password_change(
        sui_id_web::PasswordChangeData {
            username: user.username,
            revoke_others_default: true,
        },
        None,
        token.clone(),
        lang,
    );
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn password_change_post(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    crate::handlers::RequestLocale(_lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<PasswordChangeForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    // Order of checks:
    //
    // 1. CSRF first — cheap and protects every other check from
    //    being driven from a hostile origin.
    // 2. Rate limit second, sharing the Login bucket. Even though
    //    the caller already has a valid session, we don't want
    //    someone with a stolen cookie to grind the
    //    `current_password` field at unbounded rate.
    // 3. UI-level mismatch check (new vs confirm) before we go to
    //    the database — pure form ergonomics.
    // 4. The actual password change in `core::me_security`, which
    //    verifies `current_password` against the stored hash and
    //    enforces the policy on the new one.
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Login,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;

    if form.new_password != form.confirm_password {
        return Err(HttpError::html(CoreError::BadRequest(
            "new password and confirmation do not match".into(),
        )));
    }

    let revoke_others = form.revoke_others.is_some();

    // Pull the current session id from the cookie so we can keep
    // it alive across the sweep. If the cookie isn't there we'd
    // have failed `CurrentUser` already, but be explicit.
    let raw_session = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
    let keep = SessionId::from_str(&raw_session)
        .map_err(|_| HttpError::html(CoreError::Unauthenticated))?;

    // RFC 003: load HIBP settings from the DB for this request.
    let hibp_mode = sui_id_store::repos::server_settings::get(&app.db).await
        .map(|s| s.hibp_mode)
        .unwrap_or_default();

    let report = sui_id_core::me_security::change_password_self(
        &app.db,
        &app.clock,
        Some(app.hibp_client.as_ref()),
        hibp_mode,
        user_id,
        &form.current_password,
        &form.new_password,
        Some(keep),
        revoke_others,
    ).await
    .map_err(HttpError::html)?;

    let _ = report; // counts are in the audit event already; nothing to surface

    // Post-change notification mail (best-effort). The user just
    // changed their password from a known-authenticated session;
    // sending a confirmation to their email is the standard
    // self-defence against an attacker who silently cycled
    // someone else's password. Failures here do not roll the
    // change back — the audit log records both the change and
    // the notification outcome separately.
    //
    // Sent inline (await the result rather than tokio::spawn).
    // Inline keeps the test path deterministic and also means a
    // notification timeout is bounded by our SMTP timeout
    // (single-digit seconds in production), which is
    // operationally fine for a self-service action that already
    // holds a database write.
    if let Ok(Some(user_row)) =
        sui_id_store::repos::users::find_by_id_opt(&app.db, user_id).await
    {
        if let Some(email) = user_row.email.as_deref() {
            // Recipient's preferred locale, falling through to
            // the server default if unset. Resolved here rather
            // than inside `notify_password_changed` so that
            // function stays a pure builder.
            let recipient_locale = user_row
                .preferred_lang
                .as_deref()
                .and_then(sui_id_i18n::Locale::parse)
                .unwrap_or_default();
            if let Err(e) = sui_id_core::forgot_password::notify_password_changed(
                app.mailer.as_ref(),
                email,
                &user_row.display_name,
                recipient_locale,
            ).await
            {
                tracing::warn!(
                    error = %e,
                    "failed to send password-change notification"
                );
            }
        }
    }
    Ok(Redirect::to("/me/security?msg=password_changed").into_response())
}

// =====================================================================
// RFC 040 — /me/security tabbed pages
// =====================================================================

use sui_id_web::{MeOverviewData, MePasskeyData, MeLanguageData, MeShellData, MeTab};

/// GET /me/security → redirect to /me/security/overview
pub async fn security_redirect() -> Redirect {
    Redirect::to("/me/security/overview")
}

/// GET /admin/profile — 301 Permanent Redirect to /me/security/overview.
///
/// The legacy `/admin/profile` single-page UI was consolidated onto
/// `/me/security/*` in v0.44.0 (RFC 055). This stub stays in the
/// router to honour existing bookmarks. `permanent()` emits HTTP
/// 308 (the modern method-preserving permanent), but since this
/// route only accepts GET that's equivalent to 301 here.
pub async fn admin_profile_redirect() -> Redirect {
    Redirect::permanent("/me/security/overview")
}

/// GET /me/security/overview
pub async fn overview_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let lang = req_locale;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let shell = MeShellData {
        username: user.username,
        is_admin: user.is_admin,
        active_tab: MeTab::Overview,
    };
    let totp_enabled = user_totp::get(&app.db, user_id)
        .await.ok().flatten()
        .map(|r| r.enabled).unwrap_or(false);
    let passkey_count = sui_id_store::repos::user_webauthn_credentials::count_for_user(
        &app.db, user_id
    ).await.unwrap_or(0);
    let active_session_count = sessions::list_active_for_user(&app.db, user_id)
        .await.map(|v| v.len()).unwrap_or(0);
    let recent_events: Vec<sui_id_web::MeAuditEntry> =
        audit::recent_for_user(&app.db, user_id, 10)
        .await.unwrap_or_default()
        .into_iter()
        .map(|r| sui_id_web::MeAuditEntry {
            at: r.at,
            action: r.action,
            result: r.result,
            note: r.note,
        })
        .collect();
    let csrf_tok = csrf::ensure_token(&jar);
    let resp = axum::response::Html(sui_id_web::render_me_overview(
        MeOverviewData { shell, totp_enabled, passkey_count, active_session_count, recent_events, csrf_token: csrf_tok.clone() },
        app.is_dev_mode, lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}

/// GET /me/security/passkeys
pub async fn passkeys_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let lang = req_locale;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let shell = MeShellData {
        username: user.username,
        is_admin: user.is_admin,
        active_tab: MeTab::Passkey,
    };
    let passkeys = sui_id_store::repos::user_webauthn_credentials::list_for_user(
        &app.db, user_id
    ).await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let descriptors = passkeys.into_iter().map(|p| sui_id_web::PasskeyDescriptor {
        id: p.id.to_string(),
        nickname: p.nickname,
        created_at: p.created_at,
        last_used_at: None,
    }).collect();
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
        MePasskeyData { shell, passkeys: descriptors, origin_eligible, csrf_token: csrf_tok.clone() },
        flash, app.is_dev_mode, lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}

#[derive(serde::Deserialize)]
pub struct PasskeyRenameForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub nickname: String,
}

/// POST /me/security/passkeys/{id}/rename
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
            "nickname must be 1–64 characters".into()
        )));
    }
    sui_id_store::repos::user_webauthn_credentials::update_nickname(
        &app.db, &cred_id, user_id, new_name,
    ).await.map_err(|e| HttpError::html(CoreError::from(e)))?;
    Ok(Redirect::to("/me/security/passkeys").into_response())
}

/// Query parameters for `GET /me/security/language` (RFC 057).
///
/// `saved=1` means the user just successfully saved their preference
/// and should see a confirmation banner. Other values (typo, stale
/// link) are ignored — we never falsely tell the user their save
/// succeeded.
#[derive(serde::Deserialize)]
pub struct LanguageGetQuery {
    pub saved: Option<u8>,
}

/// GET /me/security/language
pub async fn language_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
    axum::extract::Query(q): axum::extract::Query<LanguageGetQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let lang = req_locale;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let shell = MeShellData {
        username: user.username.clone(),
        is_admin: user.is_admin,
        active_tab: MeTab::Language,
    };
    let just_saved = q.saved == Some(1);
    let flash: Option<sui_id_web::Flash> = None;
    let csrf_tok = csrf::ensure_token(&jar);
    let resp = axum::response::Html(sui_id_web::render_me_language(
        MeLanguageData {
            shell,
            current_preferred_lang: user.preferred_lang.clone(),
            csrf_token: csrf_tok.clone(),
            just_saved,
        },
        flash, app.is_dev_mode, lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}

#[derive(serde::Deserialize)]
pub struct LanguageForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    /// "ja" / "en" / "zh" / "" (= clear preference)
    pub locale: String,
}

/// POST /me/security/language
pub async fn language_post(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
    Form(form): Form<LanguageForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let lang = req_locale;
    let new_lang = if form.locale.trim().is_empty() {
        None
    } else {
        match sui_id_i18n::Locale::parse(form.locale.trim()) {
            Some(loc) => Some(loc.tag().to_string()),
            None => return Err(HttpError::html(CoreError::BadRequest(
                "unsupported locale".into()
            )).with_lang(lang)),
        }
    };
    sui_id_store::repos::users::set_preferred_lang(
        &app.db,
        user_id,
        new_lang.as_deref(),
        app.clock.now(),
    ).await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    Ok(Redirect::to("/me/security/language?saved=1").into_response())
}

/// GET /me/security/mfa — MFA status tab
pub async fn mfa_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let lang = req_locale;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let shell = MeShellData {
        username: user.username,
        is_admin: user.is_admin,
        active_tab: MeTab::Mfa,
    };
    let totp_enabled = user_totp::get(&app.db, user_id)
        .await.ok().flatten()
        .map(|r| r.enabled).unwrap_or(false);
    let passkey_count = sui_id_store::repos::user_webauthn_credentials::count_for_user(
        &app.db, user_id
    ).await.unwrap_or(0);
    // RFC 056 (v0.44.0): real count via decryption.
    // unwrap_or(0) — a decryption error shouldn't fail the entire tab
    // render; the count is a display detail, not a correctness invariant.
    let recovery_codes_remaining = sui_id_core::mfa::count_recovery_codes_remaining(
        &app.db, user_id,
    ).await.unwrap_or(0);
    let csrf_tok = csrf::ensure_token(&jar);
    let resp = axum::response::Html(sui_id_web::render_me_mfa(
        sui_id_web::MeMfaData {
            shell,
            totp_enabled,
            passkey_count,
            recovery_codes_remaining,
            fresh_recovery_codes: None,
            csrf_token: csrf_tok.clone(),
        },
        None, app.is_dev_mode, lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}

/// GET /me/security/sessions — Sessions tab
pub async fn sessions_tab_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let lang = req_locale;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await.map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let shell = MeShellData {
        username: user.username,
        is_admin: user.is_admin,
        active_tab: MeTab::Sessions,
    };

    let raw_session = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_owned())
        .unwrap_or_default();
    let current_session_id = raw_session;

    let session_rows = sessions::list_active_for_user(&app.db, user_id)
        .await.map_err(|e| HttpError::html(e.into()).with_lang(lang))?;

    let sessions_view: Vec<sui_id_web::MeSessionDescriptor> = session_rows.into_iter().map(|s| {
        let auth_methods = describe_auth_methods(&s.auth_methods);
        let is_current = s.id.to_string() == current_session_id;
        sui_id_web::MeSessionDescriptor {
            id: s.id.to_string(),
            created_at: s.created_at,
            expires_at: s.expires_at,
            auth_methods,
            is_current,
        }
    }).collect();

    let csrf_tok = csrf::ensure_token(&jar);
    let resp = axum::response::Html(sui_id_web::render_me_sessions(
        sui_id_web::MeSessionsData {
            shell,
            current_session_id,
            sessions: sessions_view,
            csrf_token: csrf_tok.clone(),
        },
        None, app.is_dev_mode, lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}

// ---------------------------------------------------------------
// MFA mutative routes (RFC 055, v0.44.0)
//
// Moved from `handlers/admin.rs::profile_mfa_*`. Identical business
// logic; new path prefix (`/me/security/mfa/*` instead of
// `/admin/profile/mfa/*`); post-action redirects target the new
// tab URL (`/me/security/mfa`) instead of `/admin/profile`; and the
// confirm/regenerate handlers now render `render_me_mfa` with the
// fresh recovery codes inline, instead of `render_profile`.
// ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MfaConfirmForm {
    pub code: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// POST /me/security/mfa/enroll/start — begin TOTP enrollment
pub async fn mfa_enroll_start(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<crate::handlers::admin::CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await.map_err(|e| HttpError::html(CoreError::from(e)))?;
    let ticket = sui_id_core::mfa::start_enrollment(
        &app.db, app.issuer(), user_id, &user.username,
    ).await.map_err(HttpError::html)?;
    let qr_svg = crate::handlers::admin::render_qr_svg_pub(&ticket.otpauth_uri);
    let secret_b32 = sui_id_core::totp::base32_encode(&ticket.secret).await;
    let otpauth_uri = ticket.otpauth_uri;
    drop(ticket.secret);
    let token = csrf::ensure_token(&jar);
    let resp = Html(sui_id_web::render_mfa_setup(
        sui_id_web::MfaSetupData { otpauth_uri, qr_svg, secret_b32 },
        None, token.clone(), lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// POST /me/security/mfa/enroll/confirm — confirm 6-digit TOTP code,
/// enable MFA, and surface the fresh recovery codes inline on the
/// MFA tab.
pub async fn mfa_enroll_confirm(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<MfaConfirmForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let code: u32 = form.code.trim().parse()
        .map_err(|_| HttpError::html(CoreError::BadRequest("verification code must be 6 digits".into())))?;
    let codes = sui_id_core::mfa::confirm_enrollment(&app.db, &app.clock, user_id, code).await
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
    ).await;
    render_mfa_tab_with_fresh_codes(&app, &jar, user_id, lang, Some(codes),
        sui_id_web::Flash {
            kind: sui_id_web::FlashKind::Info,
            text: lang.strings().profile_mfa_enrolled_flash.into(),
        }).await
}

/// POST /me/security/mfa/disable
pub async fn mfa_disable(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Form(form): Form<crate::handlers::admin::CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    sui_id_core::mfa::disable(&app.db, user_id).await.map_err(HttpError::html)?;
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
    ).await;
    Ok(Redirect::to("/me/security/mfa").into_response())
}

/// POST /me/security/mfa/recovery-codes/regenerate
pub async fn mfa_regenerate_recovery(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<crate::handlers::admin::CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let codes = sui_id_core::mfa::regenerate_recovery_codes(&app.db, user_id).await
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
    ).await;
    render_mfa_tab_with_fresh_codes(&app, &jar, user_id, lang, Some(codes),
        sui_id_web::Flash {
            kind: sui_id_web::FlashKind::Info,
            text: lang.strings().profile_recovery_regenerated_flash.into(),
        }).await
}

/// Helper: render the MFA tab page with fresh recovery codes
/// embedded inline (one-time display) and a flash banner.
async fn render_mfa_tab_with_fresh_codes(
    app: &crate::state::AppState,
    jar: &CookieJar,
    user_id: sui_id_shared::ids::UserId,
    lang: sui_id_i18n::Locale,
    fresh_codes: Option<Vec<String>>,
    flash: sui_id_web::Flash,
) -> Result<Response, HttpError> {
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await.map_err(|e| HttpError::html(CoreError::from(e)))?;
    let totp_enabled = sui_id_store::repos::user_totp::get(&app.db, user_id)
        .await.ok().flatten()
        .map(|r| r.enabled).unwrap_or(false);
    let passkey_count = sui_id_store::repos::user_webauthn_credentials::count_for_user(
        &app.db, user_id,
    ).await.unwrap_or(0);
    let recovery_codes_remaining = sui_id_core::mfa::count_recovery_codes_remaining(
        &app.db, user_id,
    ).await.unwrap_or(0);
    let shell = sui_id_web::MeShellData {
        username: user.username,
        is_admin: user.is_admin,
        active_tab: sui_id_web::MeTab::Mfa,
    };
    let token = csrf::ensure_token(jar);
    let resp = Html(sui_id_web::render_me_mfa(
        sui_id_web::MeMfaData {
            shell,
            totp_enabled,
            passkey_count,
            recovery_codes_remaining,
            fresh_recovery_codes: fresh_codes,
            csrf_token: token.clone(),
        },
        Some(flash),
        app.is_dev_mode,
        lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, app, &token))
}

// ---------------------------------------------------------------
// Passkey mutative routes (RFC 055, v0.44.0)
// Moved from `handlers/admin.rs::webauthn_*`.
// ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PasskeyRegisterStartForm {
    pub nickname: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// POST /me/security/passkeys/register/start
pub async fn passkey_register_start(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Form(form): Form<PasskeyRegisterStartForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let started = sui_id_core::webauthn::start_registration(
        &app.db, &app.clock, app.issuer(), user_id,
    ).await.map_err(HttpError::html)?;
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
    let challenge_value: serde_json::Value =
        serde_json::from_str(&started.challenge_json)
            .map_err(|_| HttpError::html(CoreError::Internal))?;
    Ok((jar, axum::Json(challenge_value)).into_response())
}

#[derive(Debug, Deserialize)]
pub struct PasskeyRegisterCompleteForm {
    pub credential: String,
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// POST /me/security/passkeys/register/complete
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
        &app.db, &app.clock, app.issuer(),
        pending_id, user_id, &nickname, &credential,
    ).await.map_err(HttpError::html)?;
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
    ).await;
    let jar = jar.add(crate::handlers::clear_webauthn_pending_cookie(
        app.config.server.cookie_secure,
    ));
    Ok((jar, Redirect::to("/me/security/passkeys")).into_response())
}

#[derive(Debug, Deserialize)]
pub struct PasskeyDeleteForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

/// POST /me/security/passkeys/{id}/delete
pub async fn passkey_delete(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Path(cred_id): Path<String>,
    Form(form): Form<PasskeyDeleteForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let id = cred_id.parse::<sui_id_shared::ids::WebauthnCredentialId>().map_err(|_| {
        HttpError::html(CoreError::BadRequest("invalid credential id".into()))
    })?;
    sui_id_core::webauthn::delete(&app.db, user_id, id).await.map_err(HttpError::html)?;
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
    ).await;
    Ok(Redirect::to("/me/security/passkeys").into_response())
}
