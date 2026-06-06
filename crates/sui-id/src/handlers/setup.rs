//! Setup wizard endpoints — 5-step flow
//! (welcome → admin → language → hibp-mode → done).
//!
//! ## Step structure
//!
//! - `GET /setup` — Step 1: welcome. A landing screen with a brief
//!   description of what's about to happen and a single "begin"
//!   button that takes the operator to step 2.
//! - `GET /setup/admin` — Step 2: admin form. Creates the first
//!   administrator: setup token, username, optional email, optional
//!   display name, password, password confirmation.
//! - `POST /setup/admin` — consumes the form, creates the admin and
//!   the first signing key, marks the system initialized, auto-logs
//!   the operator in, and redirects to `/setup/lang`.
//! - `GET /setup/lang` — Step 3: display-language picker (RFC 012).
//! - `POST /setup/lang` — writes `server_settings.default_lang` and
//!   redirects to `/setup/hibp`.
//! - `GET /setup/hibp` — Step 4: HIBP policy picker (RFC 012).
//! - `POST /setup/hibp` — writes `server_settings.hibp_mode` and
//!   redirects to `/setup/done`.
//! - `GET /setup/done` — Step 5: completion. Success message and
//!   a button to enter the admin dashboard.
//!
//! ## Encryption key
//!
//! The design memo lists an "encryption settings" screen. This is
//! intentionally absent: sui-id resolves the master key before HTTP
//! starts (env var, key file, or generate-on-first-run), so there
//! is no surface to plumb at wizard time. Key rotation is a CLI
//! command (`sui-id admin rotate-key`). This is documented as a
//! deliberate design choice, not a gap.
//! sui-id deliberately omits it: the master key is resolved from
//! `SUI_ID_MASTER_KEY` or `storage.key_file` *before* the HTTP
//! server starts, so the admin process never has the option of
//! "configuring encryption from the UI" — by the time the operator
//! reaches /setup the database is already encrypted and open.
//! Surfacing a UI for it would be confusing at best and dangerous
//! at worst (a process that owns its own master key shouldn't
//! advertise an interface to manipulate it). See
//! `docs/operators.md` and the v0.20.4 CHANGELOG entry for details.
//!
//! ## Step guards
//!
//! - `/setup` and `/setup/admin` redirect to `/admin/login` if the
//!   system is already initialized — there's no first admin to
//!   create twice.
//! - `/setup/done` is informational only and renders any time;
//!   if a curious operator types it in by hand before completing
//!   step 2, they see a generic "setup not yet complete" notice
//!   and a link to step 1.

use crate::errors::HttpError;
use crate::handlers::{session_cookie, AppStateExt};
use axum::response::{Html, IntoResponse, Redirect};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use sui_id_core::errors::CoreError;
use sui_id_core::{session, setup};
use sui_id_store::repos::{server_settings, state};
use sui_id_web::{
    render_setup_admin, render_setup_done, render_setup_hibp, render_setup_lang,
    render_setup_welcome, Flash, FlashKind,
};

// ---------- 画面 1 — welcome ----------

/// Optional `?lang=xx` query: an explicit language choice from the
/// setup wizard's welcome screen language picker (v0.48.2). When
/// present we set `LANG_COOKIE` and redirect to a clean `/setup`
/// URL so the chosen language persists through subsequent steps
/// (admin form, lang confirmation, HIBP, done) and survives a
/// browser refresh.
#[derive(Debug, serde::Deserialize, Default)]
pub struct WelcomeQuery {
    #[serde(default)]
    pub lang: Option<String>,
    /// v0.48.4: the setup URL printed at startup embeds the token as
    /// `?token=xxx` so operators never need to copy-paste it manually.
    #[serde(default)]
    pub token: Option<String>,
}

pub async fn welcome_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: axum_extra::extract::cookie::CookieJar,
    axum::extract::Query(query): axum::extract::Query<WelcomeQuery>,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    if initialized {
        // No second pass through the wizard. Send the operator to
        // login instead — they presumably arrived at /setup by
        // mistake or via an old link.
        return Ok(Redirect::to("/admin/login").into_response());
    }

    // v0.48.2: explicit language picker support. If the welcome
    // page was hit with `?lang=ja|en|zh`, persist that choice in
    // LANG_COOKIE and redirect to the bare /setup URL (PRG
    // pattern). The redirect carries the cookie via Set-Cookie
    // and from there RequestLocale picks it up for every
    // subsequent setup step.
    if let Some(raw) = query.lang.as_deref().map(str::trim) {
        if let Some(loc) = sui_id_i18n::Locale::parse(raw) {
            let secure = app.config.server.cookie_secure;
            let mut c = axum_extra::extract::cookie::Cookie::new(
                crate::handlers::LANG_COOKIE,
                loc.tag().to_string(),
            );
            c.set_path("/");
            c.set_http_only(false);
            c.set_same_site(axum_extra::extract::cookie::SameSite::Lax);
            c.set_secure(secure);
            c.set_max_age(time::Duration::days(365));
            let jar = jar.add(c);
            // v0.48.4: preserve ?token= through the lang PRG redirect.
            let redirect = match query.token.as_deref().filter(|t| !t.is_empty()) {
                Some(tok) => format!("/setup?token={tok}"),
                None => "/setup".to_owned(),
            };
            return Ok((jar, Redirect::to(&redirect)).into_response());
        }
    }

    let token = query.token.clone().unwrap_or_default();
    Ok(Html(render_setup_welcome(None, lang, &token)).into_response())
}

// ---------- 画面 2 — admin form ----------

/// v0.48.4: `?token=xxx` carries the setup token from the welcome screen.
/// The admin form renders it as a hidden input so the operator never
/// needs to type it; the POST handler validates it from the form body.
#[derive(Debug, serde::Deserialize, Default)]
pub struct SetupAdminQuery {
    #[serde(default)]
    pub token: Option<String>,
}

pub async fn admin_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    axum::extract::Query(query): axum::extract::Query<SetupAdminQuery>,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    if initialized {
        return Ok(Redirect::to("/admin/login").into_response());
    }
    let token = query.token.unwrap_or_default();
    Ok(Html(render_setup_admin(None, lang, &token)).into_response())
}

#[derive(Debug, Deserialize)]
pub struct SetupAdminForm {
    pub setup_token: String,
    pub username: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub display_name: String,
    pub password: String,
    pub confirm_password: String,
}

pub async fn admin_post(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Form(form): Form<SetupAdminForm>,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Setup,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;
    let t = lang.strings();

    // Form-level checks first so we can surface them as friendly
    // flash banners without consuming the setup token (which would
    // otherwise count against the rate limit and require re-entry).
    if form.password != form.confirm_password {
        let flash = Flash {
            kind: FlashKind::Warn,
            text: t.setup_password_mismatch.into(),
        };
        return Ok((
            axum::http::StatusCode::BAD_REQUEST,
            Html(render_setup_admin(Some(flash), lang, &form.setup_token)),
        )
            .into_response());
    }

    // Pwned Passwords (HIBP) check — see migration 0017.
    //
    // The setup wizard runs once at install time, so this is the
    // single entry point at v0.24.0 (other password-set entry
    // points are scheduled in the ROADMAP scope-expansion entry).
    // The check is short-circuited when mode is `off`. The HTTP
    // request is synchronous via `ureq`; we wrap it in
    // `spawn_blocking` so the axum runtime is not stalled on
    // network I/O.
    let hibp_settings = sui_id_store::repos::server_settings::get(&app.db).await
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let hibp_mode = hibp_settings.hibp_mode;
    if hibp_mode != sui_id_store::models::HibpMode::Off {
        let client = app.hibp_client.clone();
        let pw_for_check = form.password.clone();
        let outcome = sui_id_core::hibp::enforce_hibp(hibp_mode, Some(client.as_ref()), &pw_for_check).await;
        match outcome {
            sui_id_core::hibp::HibpEnforcement::Allowed => {}
            sui_id_core::hibp::HibpEnforcement::AllowedWithWarning { count } => {
                tracing::warn!(
                    count,
                    "setup-wizard admin password was found in HIBP breaches; \
                     accepted because hibp_mode = warn"
                );
                // No audit row at the setup-wizard stage: there
                // is no actor to attribute to and the audit
                // chain has no genesis row yet (it gets seeded
                // by the very admin we're about to create).
            }
            sui_id_core::hibp::HibpEnforcement::Blocked { count: _ } => {
                let flash = Flash {
                    kind: FlashKind::Warn,
                    text: t.setup_hibp_blocked.into(),
                };
                return Ok((
                    axum::http::StatusCode::BAD_REQUEST,
                    Html(render_setup_admin(Some(flash), lang, &form.setup_token)),
                )
                    .into_response());
            }
        }
    }

    let display = trimmed_or_none(&form.display_name);
    let email = trimmed_or_none(&form.email);

    let outcome = setup::create_initial_admin(
        &app.db,
        &app.clock,
        &app.setup_token,
        form.setup_token.trim(),
        form.username.trim(),
        &form.password,
        display,
        email,
    ).await;

    match outcome {
        Ok(_) => {
            // Auto-login the new admin so step 4 is reachable as
            // an authenticated session, and the post-setup
            // "enter admin" link in step 4 lands on the dashboard
            // immediately rather than the login page.
            let session_row = session::login(
                &app.db,
                &app.clock,
                form.username.trim(),
                &form.password,
                app.config.security.max_lockout.as_secs(),
            ).await
            .map_err(HttpError::html)?;
            let cookie =
                session_cookie(session_row.id.to_string(), app.config.server.cookie_secure);
            let jar = jar.add(cookie);
            Ok((jar, Redirect::to("/setup/lang")).into_response())
        }
        Err(e) => {
            let flash = Flash {
                kind: match &e {
                    CoreError::AlreadyInitialized | CoreError::Forbidden => FlashKind::Error,
                    _ => FlashKind::Warn,
                },
                text: friendly_error_text(&e, lang),
            };
            tracing::warn!(error = %e, "setup form rejected");
            Ok((
                axum::http::StatusCode::BAD_REQUEST,
                Html(render_setup_admin(Some(flash), lang, &form.setup_token)),
            )
                .into_response())
        }
    }
}

// ---------- 画面 3 — language selection (RFC 012) ----------

#[derive(Deserialize)]
pub struct SetupLangForm {
    #[serde(default)]
    pub lang: String,
}

pub async fn lang_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    if !initialized {
        return Ok(Redirect::to("/setup").into_response());
    }
    // Pre-fill with current server default (falls back to "ja" if not yet set).
    let current = server_settings::get(&app.db).await
        .map(|s| s.default_lang)
        .unwrap_or_else(|_| "ja".into());
    Ok(Html(render_setup_lang(None, &current, lang)).into_response())
}

pub async fn lang_post(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(_lang): crate::handlers::RequestLocale,
    Form(form): Form<SetupLangForm>,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    if !initialized {
        return Ok(Redirect::to("/setup").into_response());
    }
    // Validate and normalise the choice; fall back to "ja".
    let chosen = match form.lang.as_str() {
        "en" => "en",
        _ => "ja",
    };
    // Parse as Locale to confirm it's a valid tag before writing.
    let locale = sui_id_i18n::Locale::parse(chosen).unwrap_or_default();
    server_settings::update_default_lang(&app.db, locale.tag(), chrono::Utc::now()).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    Ok(Redirect::to("/setup/hibp").into_response())
}

// ---------- 画面 4 — HIBP mode (RFC 012) ----------

#[derive(Deserialize)]
pub struct SetupHibpForm {
    #[serde(default)]
    pub hibp_mode: String,
}

pub async fn hibp_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    if !initialized {
        return Ok(Redirect::to("/setup").into_response());
    }
    let current = server_settings::get(&app.db).await
        .map(|s| s.hibp_mode.as_str().to_owned())
        .unwrap_or_else(|_| "warn".into());
    Ok(Html(render_setup_hibp(None, &current, lang)).into_response())
}

pub async fn hibp_post(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    Form(form): Form<SetupHibpForm>,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    if !initialized {
        return Ok(Redirect::to("/setup").into_response());
    }
    let mode: sui_id_store::models::HibpMode = match form.hibp_mode.as_str() {
        "off" => sui_id_store::models::HibpMode::Off,
        "block" => sui_id_store::models::HibpMode::Block,
        _ => sui_id_store::models::HibpMode::Warn,
    };
    server_settings::update_hibp_mode(&app.db, mode, chrono::Utc::now()).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let _ = lang; // used by future flash messages if needed
    Ok(Redirect::to("/setup/done").into_response())
}

// ---------- 画面 5 — completion ----------

pub async fn done_get(
    state_ext: AppStateExt,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    Ok(Html(render_setup_done(initialized, lang)).into_response())
}

// ---------- helpers ----------

fn trimmed_or_none(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

fn friendly_error_text(e: &CoreError, lang: sui_id_i18n::Locale) -> String {
    let t = lang.strings();
    match e {
        CoreError::AlreadyInitialized => t.setup_already_initialized.into(),
        CoreError::Forbidden => t.setup_invalid_token.into(),
        CoreError::Conflict(msg) => msg.clone(),
        CoreError::BadRequest(msg) => msg.clone(),
        _ => t.setup_generic_failure.into(),
    }
}
