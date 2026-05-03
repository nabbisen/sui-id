//! Setup wizard endpoints — 3-step flow (welcome → admin → done).
//!
//! ## Step structure
//!
//! - `GET /setup` — 画面 1 (welcome). A landing screen with a brief
//!   description of what's about to happen and a single "begin"
//!   button that takes the operator to step 2.
//! - `GET /setup/admin` — 画面 2 (admin form). The form that
//!   actually creates the first administrator: setup token,
//!   username, optional email, optional display name, password,
//!   password confirmation.
//! - `POST /setup/admin` — consumes the form, creates the admin and
//!   the first signing key, marks the system initialized,
//!   auto-logs the operator in, and redirects to `/setup/done`.
//! - `GET /setup/done` — 画面 4 (completion). Success message and
//!   a button to enter the admin dashboard.
//!
//! ## What's NOT a step
//!
//! The design memo lists a fourth screen for encryption settings.
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
use sui_id_store::repos::state;
use sui_id_web::{
    render_setup_admin, render_setup_done, render_setup_welcome, Flash, FlashKind,
};

// ---------- 画面 1 — welcome ----------

pub async fn welcome_get(
    state_ext: AppStateExt,
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
    Ok(Html(render_setup_welcome(None)).into_response())
}

// ---------- 画面 2 — admin form ----------

pub async fn admin_get(
    state_ext: AppStateExt,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    if initialized {
        return Ok(Redirect::to("/admin/login").into_response());
    }
    Ok(Html(render_setup_admin(None)).into_response())
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

    // Form-level checks first so we can surface them as friendly
    // flash banners without consuming the setup token (which would
    // otherwise count against the rate limit and require re-entry).
    if form.password != form.confirm_password {
        let flash = Flash {
            kind: FlashKind::Warn,
            text: "パスワードと確認用パスワードが一致しません。".into(),
        };
        return Ok((
            axum::http::StatusCode::BAD_REQUEST,
            Html(render_setup_admin(Some(flash))),
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
    let hibp_settings = sui_id_store::repos::server_settings::get(&app.db)
        .map_err(|e| HttpError::html(sui_id_core::errors::CoreError::from(e)))?;
    let hibp_mode = hibp_settings.hibp_mode;
    if hibp_mode != sui_id_store::models::HibpMode::Off {
        let client = app.hibp_client.clone();
        let pw_for_check = form.password.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            sui_id_core::hibp::enforce_hibp(hibp_mode, Some(client.as_ref()), &pw_for_check)
        })
        .await
        .map_err(|_| {
            HttpError::html(sui_id_core::errors::CoreError::Internal)
        })?;
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
                    text: "このパスワードは過去のデータ漏洩で確認されています。\
                           別のものを選んでください。"
                        .into(),
                };
                return Ok((
                    axum::http::StatusCode::BAD_REQUEST,
                    Html(render_setup_admin(Some(flash))),
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
    );

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
            )
            .map_err(HttpError::html)?;
            let cookie =
                session_cookie(session_row.id.to_string(), app.config.server.cookie_secure);
            let jar = jar.add(cookie);
            Ok((jar, Redirect::to("/setup/done")).into_response())
        }
        Err(e) => {
            let flash = Flash {
                kind: match &e {
                    CoreError::AlreadyInitialized | CoreError::Forbidden => FlashKind::Error,
                    _ => FlashKind::Warn,
                },
                text: friendly_error_text(&e),
            };
            tracing::warn!(error = %e, "setup form rejected");
            Ok((
                axum::http::StatusCode::BAD_REQUEST,
                Html(render_setup_admin(Some(flash))),
            )
                .into_response())
        }
    }
}

// ---------- 画面 4 — completion ----------

pub async fn done_get(
    state_ext: AppStateExt,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    Ok(Html(render_setup_done(initialized)).into_response())
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

fn friendly_error_text(e: &CoreError) -> String {
    match e {
        CoreError::AlreadyInitialized => "サーバーは既に初期化されています。".into(),
        CoreError::Forbidden => "セットアップトークンが正しくありません。".into(),
        CoreError::Conflict(msg) => msg.clone(),
        CoreError::BadRequest(msg) => msg.clone(),
        _ => "セットアップに失敗しました。フォームを確認して再度お試しください。".into(),
    }
}
