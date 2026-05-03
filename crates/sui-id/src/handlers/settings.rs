//! `/admin/settings/*` — read-only settings overview.
//!
//! These pages surface the current effective configuration: what's
//! in `Config` (loaded from `sui-id.toml` at boot), what's in the
//! database, and a few derived facts (the audit-chain tail check
//! status, the schema version this binary was built for, etc).
//!
//! Nothing here is editable. Values change either by editing
//! `sui-id.toml` and restarting, or via the existing dedicated admin
//! pages (users, clients, signing keys). Where a setting belongs to
//! a more specific page we deep-link to it rather than re-implement
//! the controls.
//!
//! Layout: five top-level tabs implemented as five separate routes.
//! Each tab is its own page, so a refresh / bookmark / back-button
//! preserves the active tab without JavaScript and the tab state
//! survives in-flight server-side flash messages cleanly.

use crate::handlers::admin::with_csrf_cookie;
use crate::handlers::{AppStateExt, CurrentAdmin};
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
    CurrentAdmin(_admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let data = sui_id_web::SettingsBasicData {
        issuer: cfg.server.issuer.clone(),
        listen_addr: cfg.server.listen_addr.clone(),
        cookie_secure: cfg.server.cookie_secure,
        trusted_proxies: cfg.server.trusted_proxies.clone(),
        discovery_url: format!("{}/.well-known/openid-configuration", cfg.server.issuer),
        jwks_url: format!("{}/.well-known/jwks.json", cfg.server.issuer),
    };
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_settings_basic(data, None);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- セキュリティ ----------

pub async fn security_get(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let data = sui_id_web::SettingsSecurityData {
        max_lockout_label: cfg.security.max_lockout.label().to_owned(),
        // Security headers are unconditionally on for admin
        // responses; `/oauth2/*` deliberately omits some so SDKs
        // can read JSON cross-origin. Surface the policy plainly.
        hsts_enabled: cfg.server.cookie_secure,
        csp_enabled: true,
        x_frame_deny: true,
        permissions_policy_minimal: true,
        // CORS allowlist for the token endpoint is built dynamically
        // from registered redirect_uris, so there's nothing to show
        // here except the fact.
        cors_token_dynamic_from_clients: true,
        cors_public_endpoints_open: true,
    };
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_settings_security(data, None);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- 認証 ----------

pub async fn authentication_get(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let data = sui_id_web::SettingsAuthenticationData {
        password_min_length: 12,
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
    let html = sui_id_web::render_settings_authentication(data, None);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- ログ ----------

pub async fn logs_get(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    let now = app.clock.now();
    let since_24h = now - Duration::hours(24);

    // Counts over the last 24 hours for the most operationally
    // interesting actions. We do four queries rather than one
    // because count_by_action_in_window groups per bucket; for the
    // logs tab we just want totals.
    let count_in = |action: &'static str| -> Result<i64, HttpError> {
        let buckets = audit::count_by_action_in_window(
            &app.db,
            &[action],
            since_24h,
            now,
            // Single 24-hour bucket — the function works either
            // way, but a wide bucket means at most one row.
            60 * 24,
        )
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
        Ok(buckets.iter().map(|b| b.count).sum())
    };

    let data = sui_id_web::SettingsLogsData {
        log_format: app.config.log.format.clone(),
        log_filter: app.config.log.filter.clone(),
        login_success_24h: count_in("auth.login.success")?,
        login_failure_24h: count_in("auth.login.failure")?,
        login_locked_24h: count_in("auth.login.locked")?,
        password_changed_self_24h: count_in("auth.password.changed_self")?,
        // Audit chain status — small tail check, same shape the
        // boot-time verifier uses.
        chain_report: audit::verify_chain_tail(&app.db, 100)
            .map(|r| sui_id_web::SettingsChainStatus {
                checked: r.checked,
                broken_at_seq: r.broken_at_seq,
                legacy_unhashed: r.legacy_unhashed,
            })
            .map_err(|e| HttpError::html(CoreError::from(e)))?,
    };
    let token = csrf::ensure_token(&jar);
    let html = sui_id_web::render_settings_logs(data, None);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

// ---------- その他 ----------

pub async fn other_get(
    state_ext: AppStateExt,
    CurrentAdmin(_admin_id): CurrentAdmin,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let cfg = app.config.as_ref();
    let user_count = users::list(&app.db)
        .map(|v| v.len())
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let client_count = clients::list(&app.db)
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
    let html = sui_id_web::render_settings_other(data, None);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}
