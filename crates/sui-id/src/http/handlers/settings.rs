//! `/admin/settings/*` — settings overview and inline editing.
//!
//! These pages surface the current effective configuration: what's
//! in `Config` (loaded from `sui-id.toml` at boot), what's in the
//! database, and a few derived facts (the audit-chain tail check
//! status, the schema version this binary was built for, etc).
//!
//! Some fields are editable inline (default language, HIBP mode,
//! idle session timeout, max concurrent sessions, SMTP config).
//! Static config values that require a restart (listen address, DB
//! path, key file) are shown read-only. Where a setting belongs to
//! a more specific page we deep-link to it rather than re-implement
//! the controls.
//!
//! Layout: five top-level tabs implemented as five separate routes.
//! Each tab is its own page, so a refresh / bookmark / back-button
//! preserves the active tab without JavaScript and the tab state
//! survives in-flight server-side flash messages cleanly.

use crate::handlers::admin::with_csrf_cookie;
use crate::handlers::{AppStateExt, CurrentAdmin, CurrentAdminOrAuditor};
use crate::{csrf, errors::HttpError};
use axum::extract::State;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use secrecy::{ExposeSecret, SecretString};
use std::str::FromStr;

use chrono::{Duration, Utc};
use sui_id_core::errors::CoreError;
use sui_id_store::repos::{audit, clients, users};

/// `/admin/settings` → bounce to the basic tab. Pages are
/// independent routes so we don't have to pick a "default tab"
/// rendered inline; redirecting keeps the URL canonical.
pub async fn index_redirect() -> Redirect {
    Redirect::to("/admin/settings/basic")
}

// ---------- 基本 ----------

pub async fn basic_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let server_settings = sui_id_store::repos::server_settings::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let token = csrf::ensure_token(&jar);
    let data = sui_id_web::SettingsBasicData {
        issuer: cfg.server.issuer.clone(),
        listen_addr: cfg.server.listen_addr.clone(),
        cookie_secure: cfg.server.cookie_secure,
        trusted_proxies: cfg.server.trusted_proxies.clone(),
        discovery_url: format!("{}/.well-known/openid-configuration", cfg.server.issuer),
        jwks_url: format!("{}/.well-known/jwks.json", cfg.server.issuer),
        default_lang: server_settings.default_lang,
        csrf_token: token.clone(),
        can_write: actor.can_write(),
        username,
    };
    let html = sui_id_web::render_settings_basic(data, None, lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// Update the server-wide default UI language.
///
/// Form fields:
///   - `_csrf`: standard CSRF token
///   - `default_lang`: BCP-47 tag (must be one of `Locale::ALL`)
#[derive(Debug, serde::Deserialize)]
pub struct BasicLangForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub default_lang: String,
}

pub async fn basic_lang_post(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id, ref admin_actor, username): CurrentAdmin,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::Form(form): axum::Form<BasicLangForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let parsed = sui_id_i18n::Locale::parse(&form.default_lang)
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("unknown language tag".into())))?;
    let now = app.clock.now();
    sui_id_store::commands::change_default_lang(
        &app.db,
        admin_actor.user_id(),
        form.default_lang.clone(),
        now,
    )
    .await
    .map_err(|e| HttpError::html(CoreError::from(e)))?;

    // Re-render with a success flash so the admin sees the change took effect.
    let t = parsed.strings();
    let flash = Some(sui_id_web::Flash {
        kind: sui_id_web::FlashKind::Info,
        text: t.me_language_saved_flash.into(),
    });
    let cfg = app.config.as_ref();
    let server_settings = sui_id_store::repos::server_settings::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let token = csrf::ensure_token(&jar);
    let data = sui_id_web::SettingsBasicData {
        can_write: admin_actor.as_read_only().can_write(),
        issuer: cfg.server.issuer.clone(),
        listen_addr: cfg.server.listen_addr.clone(),
        cookie_secure: cfg.server.cookie_secure,
        trusted_proxies: cfg.server.trusted_proxies.clone(),
        discovery_url: format!("{}/.well-known/openid-configuration", cfg.server.issuer),
        jwks_url: format!("{}/.well-known/jwks.json", cfg.server.issuer),
        default_lang: server_settings.default_lang,
        csrf_token: token.clone(),
        username,
    };
    let html = sui_id_web::render_settings_basic(data, flash, lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- セキュリティ ----------

pub async fn security_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let server_settings = sui_id_store::repos::server_settings::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let token = csrf::ensure_token(&jar);
    let data = sui_id_web::SettingsSecurityData {
        can_write: actor.can_write(),
        max_lockout_label: cfg.security.max_lockout.label().to_owned(),
        hsts_enabled: cfg.server.cookie_secure,
        csp_enabled: true,
        x_frame_deny: true,
        permissions_policy_minimal: true,
        cors_token_dynamic_from_clients: true,
        cors_public_endpoints_open: true,
        idle_session_timeout_secs: server_settings.idle_session_timeout_secs,
        max_concurrent_sessions: server_settings.max_concurrent_sessions,
        csrf_token: token.clone(),
        username,
    };
    let html = sui_id_web::render_settings_security(data, None, lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// POST /admin/settings/security/idle-timeout
///
/// Update `server_settings.idle_session_timeout_secs`.
/// Application bounds: `[0, 30 * 86400]`.
#[derive(Debug, serde::Deserialize)]
pub struct IdleTimeoutForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub secs: i64,
}

pub async fn idle_timeout_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, _, _): CurrentAdmin,
    jar: CookieJar,
    axum::Form(form): axum::Form<IdleTimeoutForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    const MAX: i64 = 30 * 86400;
    if !(0..=MAX).contains(&form.secs) {
        return Err(HttpError::html(CoreError::BadRequest(
            "idle_session_timeout_secs out of range".into(),
        )));
    }
    let now = app.clock.now();
    sui_id_store::commands::change_idle_session_timeout(&app.db, admin_id, form.secs, now)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    Ok(axum::response::Redirect::to("/admin/settings/security").into_response())
}

/// POST /admin/settings/security/max-sessions
///
/// Update `server_settings.max_concurrent_sessions`.
/// Application bounds: `[0, 1000]`.
#[derive(Debug, serde::Deserialize)]
pub struct MaxSessionsForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub cap: i64,
}

pub async fn max_sessions_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, _, _): CurrentAdmin,
    jar: CookieJar,
    axum::Form(form): axum::Form<MaxSessionsForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    if !(0..=1000).contains(&form.cap) {
        return Err(HttpError::html(CoreError::BadRequest(
            "max_concurrent_sessions out of range".into(),
        )));
    }
    let now = app.clock.now();
    sui_id_store::commands::change_max_concurrent_sessions(&app.db, admin_id, form.cap, now)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    Ok(axum::response::Redirect::to("/admin/settings/security").into_response())
}

// ---------- 認証 ----------

pub async fn authentication_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let data = authentication_data(&app, actor.can_write(), username).await?;
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_settings_authentication(data, None, token.clone(), lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// The *Authentication* tab's data, for its `GET` and for a refused save
/// (P5 review: one builder, so the two renders cannot drift).
async fn authentication_data(
    app: &crate::state::AppState,
    can_write: bool,
    username: String,
) -> Result<sui_id_web::SettingsAuthenticationData, HttpError> {
    let cfg = app.config.as_ref();
    let server_settings = sui_id_store::repos::server_settings::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    // RFC 151 §4's scan allows `SecurityLevel::password_min_len()` only
    // at setup, dev mode's seeding, and `handlers::password_min_len`'s
    // own dev branch, so dev mode's value is read through that function.
    let effective = crate::handlers::password_min_len(app)
        .await
        .map_err(HttpError::html)?;
    Ok(sui_id_web::SettingsAuthenticationData {
        password_min_length: server_settings.password_min_len as usize,
        password_min_length_floor: sui_id_store::commands::PASSWORD_MIN_LEN_FLOOR as usize,
        password_min_length_ceiling: sui_id_store::commands::PASSWORD_MIN_LEN_CEILING as usize,
        is_dev_mode: app.is_dev_mode,
        password_min_length_dev_floor: effective,
        hibp_mode: server_settings.hibp_mode,
        password_argon2id: "Argon2id".into(),
        totp_enabled_per_user: true,
        webauthn_enabled_per_user: true,
        recovery_codes_per_enrollment: 8,
        pkce_required: true,
        access_token_lifetime_secs: cfg.tokens.access_lifetime_secs,
        id_token_lifetime_secs: cfg.tokens.id_token_lifetime_secs,
        refresh_token_lifetime_secs: cfg.tokens.refresh_lifetime_secs,
        refresh_rotation: true,
        refresh_theft_detection: true,
        can_write,
        username,
    })
}

/// POST /admin/settings/authentication/password-min-len
///
/// Update `server_settings.password_min_len` (RFC 151). Administrator
/// only (`CurrentAdmin`, not `CurrentAdminOrAuditor`): an auditor's own
/// POST is refused by the extractor before this body runs, the same
/// gate `idle_timeout_post`/`max_sessions_post` already use.
#[derive(Debug, serde::Deserialize)]
pub struct PasswordMinLenForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    /// Taken as text, not a typed integer: a non-numeric value must
    /// re-render the tab with a plain flash, never axum's own 422 for
    /// a form field it could not parse.
    pub password_min_len: String,
}

pub async fn password_min_len_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, _, username): CurrentAdmin,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::Form(form): axum::Form<PasswordMinLenForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;

    let parsed = form.password_min_len.trim().parse::<i64>().ok();
    let outcome = match parsed {
        Some(n) => {
            let now = app.clock.now();
            sui_id_store::commands::change_password_min_len(&app.db, admin_id, n, now)
                .await
                .map(|_| ())
        }
        None => Err(sui_id_store::StoreError::PasswordMinLenOutOfRange {
            floor: sui_id_store::commands::PASSWORD_MIN_LEN_FLOOR,
            ceiling: sui_id_store::commands::PASSWORD_MIN_LEN_CEILING,
        }),
    };

    match outcome {
        Ok(()) => {
            Ok(axum::response::Redirect::to("/admin/settings/authentication").into_response())
        }
        Err(sui_id_store::StoreError::PasswordMinLenOutOfRange { floor, ceiling }) => {
            let t = lang.strings();
            let data = authentication_data(&app, true, username).await?;
            let flash = Some(sui_id_web::Flash {
                kind: sui_id_web::FlashKind::Error,
                text: (t.settings_auth_min_length_flash_invalid)(floor as usize, ceiling as usize),
            });
            let token = csrf::ensure_token(&jar);
            let html = sui_id_web::render_settings_authentication(data, flash, token.clone(), lang);
            Ok(with_csrf_cookie(
                (axum::http::StatusCode::BAD_REQUEST, Html(html)).into_response(),
                &app,
                &token,
            ))
        }
        Err(e) => Err(HttpError::html(CoreError::from(e))),
    }
}

// ---------- パスワード漏洩チェック (RFC 149 Q3b) ----------

/// `POST /admin/settings/authentication/hibp-mode` — the direct save
/// route. Strengthening (or no change) saves immediately, like
/// `password_min_len_post`. A weakening is never saved here: the
/// direction is decided from the *stored* mode and the submitted one,
/// inside this request, never from anything the form claims about
/// itself (RFC 149 Q3b §2) — so it redirects to the confirm screen
/// instead, carrying only the mode the person picked.
#[derive(Debug, serde::Deserialize)]
pub struct HibpModeForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub hibp_mode: String,
}

pub async fn hibp_mode_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, _, username): CurrentAdmin,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::Form(form): axum::Form<HibpModeForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;

    let Some(new_mode) = sui_id_store::models::HibpMode::parse(form.hibp_mode.trim()) else {
        let t = lang.strings();
        let data = authentication_data(&app, true, username).await?;
        let flash = Some(sui_id_web::Flash {
            kind: sui_id_web::FlashKind::Error,
            text: t.settings_auth_hibp_invalid.to_owned(),
        });
        let token = csrf::ensure_token(&jar);
        let html = sui_id_web::render_settings_authentication(data, flash, token.clone(), lang);
        return Ok(with_csrf_cookie(
            (axum::http::StatusCode::BAD_REQUEST, Html(html)).into_response(),
            &app,
            &token,
        ));
    };

    let current = sui_id_store::repos::server_settings::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?
        .hibp_mode;

    match sui_id_store::models::classify_hibp_mode_change(current, new_mode) {
        sui_id_store::models::HibpModeChange::Unchanged => {
            Ok(Redirect::to("/admin/settings/authentication").into_response())
        }
        sui_id_store::models::HibpModeChange::Strengthened => {
            let now = app.clock.now();
            // The command re-decides the direction inside its own
            // transaction. If another administrator raised the mode since
            // `current` was read, this save is now a weakening: it is
            // refused there, and the person is sent to the confirmation.
            match sui_id_store::commands::change_hibp_mode(
                &app.db,
                admin_id,
                new_mode,
                sui_id_store::commands::HibpModeAuthority::StrengthenOnly,
                now,
            )
            .await
            {
                Ok(_) => Ok(Redirect::to("/admin/settings/authentication").into_response()),
                Err(sui_id_store::StoreError::Conflict) => Ok(Redirect::to(&format!(
                    "/admin/settings/authentication/hibp-mode-confirm?mode={}",
                    new_mode.as_str()
                ))
                .into_response()),
                Err(e) => Err(HttpError::html(CoreError::from(e))),
            }
        }
        sui_id_store::models::HibpModeChange::Weakened => Ok(Redirect::to(&format!(
            "/admin/settings/authentication/hibp-mode-confirm?mode={}",
            new_mode.as_str()
        ))
        .into_response()),
    }
}

/// `GET /admin/settings/authentication/hibp-mode-confirm?mode=<new>` —
/// RFC 058's confirmation screen for a weakening. No step-up is asked
/// to view it; the confirm `POST` asks for one, immediately before it
/// saves (same split `users_disable_confirm_get`/`users_set_disabled`
/// use).
pub async fn hibp_mode_confirm_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref actor, admin_username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Response, HttpError> {
    if !actor.can_write() {
        return Err(crate::errors::HttpError::html_403_auditor());
    }
    let State(app) = state_ext;
    let t = lang.strings();
    let mode_str = q.get("mode").cloned().unwrap_or_default();
    let new_mode = sui_id_store::models::HibpMode::parse(mode_str.trim())
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("invalid hibp mode".into())))?;
    let current = sui_id_store::repos::server_settings::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?
        .hibp_mode;
    let impact = match new_mode {
        sui_id_store::models::HibpMode::Warn => t.settings_auth_hibp_confirm_impact_to_warn,
        // `Off` and the unreachable `Block` (never a weakening target,
        // §2) share the "no longer checked" sentence rather than a
        // third arm for a case that cannot occur through the save
        // route above.
        _ => t.settings_auth_hibp_confirm_impact_to_off,
    };
    let token = crate::csrf::ensure_token(&jar);
    let data = sui_id_web::SettingsHibpModeConfirmData {
        old_label: sui_id_web::hibp_mode_label(current, t).to_owned(),
        new_label: sui_id_web::hibp_mode_label(new_mode, t).to_owned(),
        new_mode: new_mode.as_str().to_owned(),
        impact: impact.to_owned(),
        csrf_token: token.clone(),
        admin_username,
    };
    let resp = Html(sui_id_web::render_settings_hibp_mode_confirm(
        data,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// `POST /admin/settings/authentication/hibp-mode-confirm` — CSRF,
/// `_confirmed=1`, a fresh step-up (returning to the confirm screen,
/// not auto-saving, if it isn't fresh), then the same direction check
/// the save route uses: re-derived from the *current* stored mode, so
/// a second change racing this one cannot be saved as a weakening it
/// no longer is, and saving the mode already stored writes nothing.
#[derive(Debug, serde::Deserialize)]
pub struct HibpModeConfirmForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    #[serde(default)]
    pub confirmed: String,
    pub mode: String,
}

pub async fn hibp_mode_confirm_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, _, _username): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    axum::Form(form): axum::Form<HibpModeConfirmForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    crate::handlers::require_confirmed(&form.confirmed)?;
    let return_to = format!(
        "/admin/settings/authentication/hibp-mode-confirm?mode={}",
        form.mode
    );
    if let Err(redirect) = crate::handlers::require_fresh_step_up(&app, &ctx, &return_to).await {
        return Ok(redirect);
    }
    let new_mode = sui_id_store::models::HibpMode::parse(form.mode.trim())
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("invalid hibp mode".into())))?;
    let current = sui_id_store::repos::server_settings::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?
        .hibp_mode;
    if !matches!(
        sui_id_store::models::classify_hibp_mode_change(current, new_mode),
        sui_id_store::models::HibpModeChange::Unchanged
    ) {
        let now = app.clock.now();
        sui_id_store::commands::change_hibp_mode(
            &app.db,
            admin_id,
            new_mode,
            // Confirmed, and stepped up just above (RFC 058).
            sui_id_store::commands::HibpModeAuthority::MayWeaken,
            now,
        )
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    }
    Ok(Redirect::to("/admin/settings/authentication").into_response())
}

// ---------- ログ ----------

pub async fn logs_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    let now = app.clock.now();
    let since_24h = now - Duration::hours(24);

    // Counts over the last 24 hours for the most operationally
    // interesting actions. We do four queries rather than one
    // because count_by_action_in_window groups per bucket; for the
    // logs tab we just want totals.
    async fn count_action(
        db: &sui_id_store::Database,
        action: &str,
        since: chrono::DateTime<chrono::Utc>,
        until: chrono::DateTime<chrono::Utc>,
    ) -> Result<i64, HttpError> {
        let buckets = audit::count_by_action_in_window(db, &[action], since, until, 60 * 24)
            .await
            .map_err(|e| HttpError::html(CoreError::from(e)))?;
        Ok(buckets.iter().map(|b| b.count).sum())
    }
    let login_success = count_action(&app.db, "auth.login.success", since_24h, now).await?;
    let login_failure = count_action(&app.db, "auth.login.failure", since_24h, now).await?;
    let login_locked = count_action(&app.db, "auth.lockout", since_24h, now).await?;
    let password_changed =
        count_action(&app.db, "auth.password.changed_self", since_24h, now).await?;
    // RFC 121 D6: the one shared check, which never fails the whole handler —
    // a chain-verify error is one of its three outcomes, not a `?` that
    // takes the 24-hour counters above down with it.
    let chain_report = crate::handlers::chain_status(&app, 100).await;
    let data = sui_id_web::SettingsLogsData {
        log_format: app.config.log.format.clone(),
        log_filter: app.config.log.filter.clone(),
        login_success_24h: login_success,
        login_failure_24h: login_failure,
        login_locked_24h: login_locked,
        password_changed_self_24h: password_changed,
        chain_report,
        can_write: actor.can_write(),
        username,
    };
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_settings_logs(data, None, token.clone(), lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- その他 ----------

pub async fn other_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let user_count = users::list(&app.db)
        .await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let client_count = clients::list(&app.db)
        .await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    // RFC 112 D4: the version the database *records*, read with the one reader
    // every caller uses. This page used to show `MAX_SCHEMA_VERSION` under the
    // label "schema version": the binary's ceiling, not the database's value.
    // RFC 094 cycle J stage 6b: reads it through a store method now, not a raw
    // `with_conn` connection handed to `migrations::` from outside the store.
    let stored_schema_version = app
        .db
        .stored_schema_version()
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let data = sui_id_web::SettingsOtherData {
        binary_version: env!("CARGO_PKG_VERSION").to_owned(),
        schema_version: stored_schema_version,
        db_path: cfg.storage.db_path.display().to_string(),
        master_key_file: cfg.storage.key_file.display().to_string(),
        user_count,
        client_count,
        clock_now: Utc::now(),
        can_write: actor.can_write(),
        username,
    };
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_settings_other(data, None, token.clone(), lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- メール (v0.22.0) ----------

#[derive(Debug, serde::Deserialize)]
pub struct EmailSettingsForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    #[serde(default)]
    pub enabled: Option<String>,
    pub host: String,
    pub port: u16,
    pub tls_mode: String,
    #[serde(default)]
    pub username: String,
    /// New password value. Empty string means "keep the existing
    /// stored password". `None` (form field absent) is treated the
    /// same as empty for resilience against missing fields.
    #[serde(default)]
    pub password: SecretString,
    pub from_address: String,
    #[serde(default)]
    pub from_name: String,
    pub base_url: String,
}

pub async fn email_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(_admin_id, _role, ref actor, username): CurrentAdminOrAuditor,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg_row = sui_id_store::repos::smtp_config::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let mut data = build_email_data(cfg_row.as_ref());
    data.can_write = actor.can_write();
    data.admin_username = username;
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_settings_email(data, token.clone(), None, lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn email_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, ref admin_actor, _): CurrentAdmin,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    axum::Form(form): axum::Form<EmailSettingsForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;

    let tls_mode = sui_id_store::models::SmtpTlsMode::parse(&form.tls_mode).ok_or_else(|| {
        HttpError::html(CoreError::BadRequest(format!(
            "unknown tls_mode: {}",
            form.tls_mode
        )))
    })?;
    if !form.base_url.starts_with("http://") && !form.base_url.starts_with("https://") {
        return Err(HttpError::html(CoreError::BadRequest(
            "base_url must start with http:// or https://".into(),
        )));
    }

    let now = app.clock.now();
    let username = if form.username.trim().is_empty() {
        None
    } else {
        Some(form.username.trim().to_owned())
    };
    let from_name = if form.from_name.trim().is_empty() {
        None
    } else {
        Some(form.from_name.trim().to_owned())
    };
    let enabled = form
        .enabled
        .as_deref()
        .map(|v| matches!(v, "true" | "on" | "1"))
        .unwrap_or(false);

    // Password handling: empty string means "keep existing".
    // RFC 090: if a new password is provided, route through pending-change
    // so the secret never appears in a form field after initial entry.
    let existing = sui_id_store::repos::smtp_config::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    if !form.password.expose_secret().is_empty() {
        // Seal the payload for pending-change storage. The type is
        // shared with `pending_change::apply`, which decrypts into it
        // directly (RFC 094 M2b stage 2) -- defined once in core rather
        // than twice, so the create and apply sides cannot drift apart.
        let payload = sui_id_core::pending_change::SmtpPasswordChangePayload {
            enabled,
            host: form.host.trim().to_owned(),
            port: form.port,
            tls_mode: form.tls_mode.clone(),
            username: username.clone(),
            password: form.password.expose_secret().to_owned(),
            from_address: form.from_address.trim().to_owned(),
            from_name: from_name.clone(),
            base_url: form.base_url.trim().trim_end_matches('/').to_owned(),
        };
        let csrf_token_for_confirm = crate::csrf::ensure_token(&jar);
        let summary = format!(
            "SMTP password: will be updated | host: {} | port: {}",
            form.host.trim(),
            form.port
        );
        let pending = sui_id_core::pending_change::create(
            &app.db,
            admin_actor,
            ctx.session_id,
            "smtp_password_update",
            &payload,
            &summary,
            &csrf_token_for_confirm,
            &app.clock,
        )
        .await
        .map_err(HttpError::html)?;
        let redirect_url = format!(
            "/admin/settings/email/confirm?pending_change_id={}",
            pending.id
        );
        return Ok(with_csrf_cookie(
            axum::response::Redirect::to(&redirect_url).into_response(),
            &app,
            &csrf_token_for_confirm,
        ));
    }

    let password_enc = existing.as_ref().and_then(|r| r.password_enc.clone());
    let created_at = existing.map(|r| r.created_at).unwrap_or(now);

    let row = sui_id_store::models::SmtpConfigRow {
        enabled,
        host: form.host.trim().to_owned(),
        port: form.port,
        tls_mode,
        username,
        password_enc,
        from_address: form.from_address.trim().to_owned(),
        from_name,
        base_url: form.base_url.trim().trim_end_matches('/').to_owned(),
        created_at,
        updated_at: now,
    };
    sui_id_store::commands::change_smtp_config(&app.db, admin_id, row)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    Ok(Redirect::to("/admin/settings/email").into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct EmailTestForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn email_test(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id, _, username): CurrentAdmin,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::Form(form): axum::Form<EmailTestForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;

    // Run an isolated SmtpMailSender pointed at the same DB so it
    // reads the persisted config. This intentionally does not use
    // `app.mailer` — that one is the trait object and doesn't
    // expose `test_connection`.
    let probe = sui_id_core::mail::SmtpMailSender::new(
        app.db.clone(),
        ehlo_hostname_from_issuer(&app.config.server.issuer),
    );
    let result = probe.test_connection().await;

    let cfg_row = sui_id_store::repos::smtp_config::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let mut data = build_email_data(cfg_row.as_ref());
    data.can_write = true; // email_test is admin-only
    data.admin_username = username;
    let token = csrf::ensure_token(&jar);
    let flash = match &result {
        Ok(()) => Some(sui_id_web::Flash {
            kind: sui_id_web::FlashKind::Info,
            text: lang.strings().settings_email_test_passed.into(),
        }),
        Err(sui_id_core::CoreError::BadRequest(msg)) => Some(sui_id_web::Flash {
            kind: sui_id_web::FlashKind::Error,
            text: (lang.strings().settings_email_test_failed)(msg),
        }),
        Err(e) => Some(sui_id_web::Flash {
            kind: sui_id_web::FlashKind::Error,
            text: (lang.strings().settings_email_test_failed)(&e.to_string()),
        }),
    };
    let html = sui_id_web::render_settings_email(data, token.clone(), flash, lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// `GET /admin/settings/email/confirm` — display non-secret summary of a
/// pending SMTP settings change (RFC 090).
pub async fn email_confirm_get(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id, _, username): CurrentAdmin,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let id_str = q.get("pending_change_id").cloned().unwrap_or_default();
    // Validate that the id looks like a UUID before rendering (no DB lookup needed;
    // the confirm POST does the authoritative check).
    if id_str.is_empty() {
        return Err(HttpError::html(CoreError::BadRequest(
            "Missing pending_change_id.".into(),
        )));
    }
    let token = crate::csrf::ensure_token(&jar);
    // Fetch the non-secret summary for display (does not consume the row).
    let pending_id = sui_id_shared::ids::PendingChangeId::from_str(&id_str)
        .map_err(|_| HttpError::html(CoreError::BadRequest("Invalid pending_change_id.".into())))?;
    let summary = sui_id_store::repos::pending_settings_change::get_summary(
        &app.db,
        pending_id,
        app.clock.now(),
    )
    .await
    .map_err(|_| {
        HttpError::html(CoreError::BadRequest(
            "This pending change has expired or is no longer valid.".into(),
        ))
    })?;
    let data = sui_id_web::SettingsEmailConfirmData {
        pending_change_id: id_str,
        summary,
        csrf_token: token.clone(),
        admin_username: username,
    };
    let resp = Html(sui_id_web::render_settings_email_confirm(
        data,
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

/// `POST /admin/settings/email/confirm` — apply a pending SMTP settings change
/// (RFC 090). Final security boundary: revalidates admin role, session binding,
/// CSRF, step-up freshness, and change expiry before applying.
#[derive(Debug, serde::Deserialize)]
pub struct EmailConfirmForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub pending_change_id: String,
}

pub async fn email_confirm_post(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id, ref admin_actor, _): CurrentAdmin,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    axum::Form(form): axum::Form<EmailConfirmForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    // RFC 090 P5: revalidate step-up on the final confirm POST.
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/admin/settings/email/confirm").await
    {
        return Ok(redirect);
    }
    let pending_id = sui_id_shared::ids::PendingChangeId::from_str(&form.pending_change_id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("Invalid pending_change_id.".into())))?;
    // Validates, decrypts, and applies the pending change in one call
    // (RFC 094 M2b stage 2, S08): peeks the row, validates `tls_mode` and
    // seals the password outside any transaction, then one transaction
    // re-reads the row, checks the binding, and -- only on a match --
    // deletes it and writes `smtp_config` together. See
    // `sui_id_core::pending_change::apply`'s own doc comment for why a
    // validation or binding failure both leave the row intact.
    sui_id_core::pending_change::apply(
        &app.db,
        pending_id,
        admin_actor,
        ctx.session_id,
        &form.csrf,
        &app.clock,
    )
    .await
    .map_err(HttpError::html)?;
    let flash = sui_id_web::Flash {
        kind: sui_id_web::FlashKind::Info,
        text: "Email settings saved.".to_owned(),
    };
    let cfg_row = sui_id_store::repos::smtp_config::get(&app.db)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let mut data = build_email_data(cfg_row.as_ref());
    data.can_write = true;
    let token = crate::csrf::ensure_token(&jar);
    let html = sui_id_web::render_settings_email(data, token.clone(), Some(flash), lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

fn build_email_data(
    cfg: Option<&sui_id_store::models::SmtpConfigRow>,
) -> sui_id_web::SettingsEmailData {
    match cfg {
        Some(row) => sui_id_web::SettingsEmailData {
            configured: true,
            enabled: row.enabled,
            host: row.host.clone(),
            port: row.port,
            tls_mode: row.tls_mode.as_str().to_owned(),
            username: row.username.clone().unwrap_or_default(),
            has_password: row.password_enc.is_some(),
            from_address: row.from_address.clone(),
            from_name: row.from_name.clone().unwrap_or_default(),
            base_url: row.base_url.clone(),
            can_write: false, // overridden by caller via actor.can_write()
            admin_username: String::new(), // overridden by caller
        },
        None => sui_id_web::SettingsEmailData {
            configured: false,
            enabled: false,
            host: String::new(),
            port: 587,
            tls_mode: "starttls".into(),
            username: String::new(),
            has_password: false,
            from_address: String::new(),
            from_name: String::new(),
            base_url: String::new(),
            can_write: false, // overridden by caller via actor.can_write()
            admin_username: String::new(), // overridden by caller
        },
    }
}

fn ehlo_hostname_from_issuer(issuer: &str) -> String {
    url::Url::parse(issuer)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| "sui-id.local".to_owned())
}
