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
    CurrentAdminOrAuditor(admin_id, _role, _): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let server_settings = sui_id_store::repos::server_settings::get(&app.db).await
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
    };
        let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
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
    CurrentAdmin(admin_id, _): CurrentAdmin,
    jar: CookieJar,
    axum::Form(form): axum::Form<BasicLangForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let parsed = sui_id_i18n::Locale::parse(&form.default_lang)
        .ok_or_else(|| HttpError::html(CoreError::BadRequest("unknown language tag".into())))?;
    let now = app.clock.now();
    sui_id_store::repos::server_settings::update_default_lang(&app.db, &form.default_lang, now).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    // Re-render with a success flash so the admin sees the change took effect.
    let t = parsed.strings();
    let flash = Some(sui_id_web::Flash {
        kind: sui_id_web::FlashKind::Info,
        text: t.me_language_saved_flash.into(),
    });
    let cfg = app.config.as_ref();
    let server_settings = sui_id_store::repos::server_settings::get(&app.db).await
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
    };
    let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let html = sui_id_web::render_settings_basic(data, flash, lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- セキュリティ ----------

pub async fn security_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, _): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let server_settings = sui_id_store::repos::server_settings::get(&app.db).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let token = csrf::ensure_token(&jar);
    let data = sui_id_web::SettingsSecurityData {
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
    };
        let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
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
    CurrentAdmin(_admin_id, _): CurrentAdmin,
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
    sui_id_store::repos::server_settings::update_idle_session_timeout(&app.db, form.secs, now).await
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
    CurrentAdmin(_admin_id, _): CurrentAdmin,
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
    sui_id_store::repos::server_settings::update_max_concurrent_sessions(&app.db, form.cap, now).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    Ok(axum::response::Redirect::to("/admin/settings/security").into_response())
}

// ---------- 認証 ----------

pub async fn authentication_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, _): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let data = sui_id_web::SettingsAuthenticationData {
        password_min_length: app.security_level().password_min_len(),
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
    };
    let token = csrf::ensure_token(&jar);
        let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let html = sui_id_web::render_settings_authentication(data, None, token.clone(), lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- ログ ----------

pub async fn logs_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, _): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    let now = app.clock.now();
    let since_24h = now - Duration::hours(24);

    // Counts over the last 24 hours for the most operationally
    // interesting actions. We do four queries rather than one
    // because count_by_action_in_window groups per bucket; for the
    // logs tab we just want totals.
    async fn count_action(db: &sui_id_store::Database, action: &str, since: chrono::DateTime<chrono::Utc>, until: chrono::DateTime<chrono::Utc>) -> Result<i64, HttpError> {
        let buckets = audit::count_by_action_in_window(db, &[action], since, until, 60 * 24)
            .await
            .map_err(|e| HttpError::html(CoreError::from(e)))?;
        Ok(buckets.iter().map(|b| b.count).sum())
    }
    let login_success = count_action(&app.db, "auth.login.success", since_24h, now).await?;
    let login_failure = count_action(&app.db, "auth.login.failure", since_24h, now).await?;
    let login_locked = count_action(&app.db, "auth.login.locked", since_24h, now).await?;
    let password_changed = count_action(&app.db, "auth.password.changed_self", since_24h, now).await?;
    let data = sui_id_web::SettingsLogsData {
        log_format: app.config.log.format.clone(),
        log_filter: app.config.log.filter.clone(),
        login_success_24h: login_success,
        login_failure_24h: login_failure,
        login_locked_24h: login_locked,
        password_changed_self_24h: password_changed,
        // Audit chain status — small tail check, same shape the
        // boot-time verifier uses.
        chain_report: audit::verify_chain_tail(&app.db, 100).await
            .map(|r| sui_id_web::SettingsChainStatus {
                checked: r.checked,
                broken_at_seq: r.broken_at_seq,
                legacy_unhashed: r.legacy_unhashed,
            })
            .map_err(|e| HttpError::html(CoreError::from(e)))?,
    };
    let token = csrf::ensure_token(&jar);
        let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let html = sui_id_web::render_settings_logs(data, None, token.clone(), lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- その他 ----------

pub async fn other_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, _): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let user_count = users::list(&app.db).await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let client_count = clients::list(&app.db).await
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let data = sui_id_web::SettingsOtherData {
        binary_version: env!("CARGO_PKG_VERSION").to_owned(),
        schema_version: sui_id_store::migrations::MAX_SCHEMA_VERSION,
        db_path: cfg.storage.db_path.display().to_string(),
        master_key_file: cfg.storage.key_file.display().to_string(),
        user_count,
        client_count,
        clock_now: Utc::now(),
    };
    let token = csrf::ensure_token(&jar);
        let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
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
    pub password: String,
    pub from_address: String,
    #[serde(default)]
    pub from_name: String,
    pub base_url: String,
}

pub async fn email_get(
    state_ext: AppStateExt,
    CurrentAdminOrAuditor(admin_id, _role, _): CurrentAdminOrAuditor,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg_row = sui_id_store::repos::smtp_config::get(&app.db).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let data = build_email_data(cfg_row.as_ref());
    let token = csrf::ensure_token(&jar);
        let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let html = sui_id_web::render_settings_email(data, token.clone(), None, lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

pub async fn email_post(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, _): CurrentAdmin,
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
    let existing = sui_id_store::repos::smtp_config::get(&app.db).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let password_enc = if form.password.is_empty() {
        existing.as_ref().and_then(|r| r.password_enc.clone())
    } else {
        Some(
            sui_id_store::repos::smtp_config::seal_password(&form.password, app.db.key()).await
                .map_err(|e| HttpError::html(CoreError::from(e)))?,
        )
    };
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
    sui_id_store::repos::smtp_config::upsert(&app.db, &row).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: now,
            actor: Some(admin_id),
            action: "auth.smtp_config.changed".into(),
            target: None,
            result: "ok".into(),
            note: Some(format!(
                "enabled={enabled} host={} port={} tls={}",
                row.host,
                row.port,
                row.tls_mode.as_str()
            )),
        },
    ).await;

    Ok(Redirect::to("/admin/settings/email").into_response())
}

#[derive(Debug, serde::Deserialize)]
pub struct EmailTestForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
}

pub async fn email_test(
    state_ext: AppStateExt,
    CurrentAdmin(admin_id, _): CurrentAdmin,
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

    let cfg_row = sui_id_store::repos::smtp_config::get(&app.db).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let data = build_email_data(cfg_row.as_ref());
    let token = csrf::ensure_token(&jar);
    let flash = match &result {
        Ok(()) => Some(sui_id_web::Flash {
            kind: sui_id_web::FlashKind::Info,
            text: "SMTP 接続テストが成功しました。".into(),
        }),
        Err(sui_id_core::CoreError::BadRequest(msg)) => Some(sui_id_web::Flash {
            kind: sui_id_web::FlashKind::Error,
            text: format!("SMTP 接続テストに失敗しました: {msg}"),
        }),
        Err(e) => Some(sui_id_web::Flash {
            kind: sui_id_web::FlashKind::Error,
            text: format!("SMTP 接続テストに失敗しました: {e}"),
        }),
    };
        let lang = crate::handlers::resolve_admin_locale(&app, admin_id).await;
    let html = sui_id_web::render_settings_email(data, token.clone(), flash, lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

fn build_email_data(cfg: Option<&sui_id_store::models::SmtpConfigRow>) -> sui_id_web::SettingsEmailData {
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
        },
    }
}

fn ehlo_hostname_from_issuer(issuer: &str) -> String {
    url::Url::parse(issuer)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| "sui-id.local".to_owned())
}
