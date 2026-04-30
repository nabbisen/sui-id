//! Setup wizard endpoints.
//!
//! `GET /setup` renders the wizard if and only if the system is in the
//! uninitialized state. `POST /setup` consumes the form, creates the first
//! administrator and a signing key, marks the system initialized, and
//! redirects to the login page.

use crate::errors::HttpError;
use crate::handlers::{session_cookie, AppStateExt, SESSION_COOKIE};
use axum::response::{Html, IntoResponse, Redirect};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use sui_id_core::errors::CoreError;
use sui_id_core::{session, setup};
use sui_id_store::repos::state;
use sui_id_web::{render_setup, Flash, FlashKind};

pub async fn get(state_ext: AppStateExt) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let initialized =
        state::is_initialized(&app.db).map_err(|e| HttpError::html(CoreError::from(e)))?;
    if initialized {
        return Ok(Redirect::to("/admin").into_response());
    }
    Ok(Html(render_setup(None)).into_response())
}

#[derive(Debug, Deserialize)]
pub struct SetupForm {
    pub setup_token: String,
    pub username: String,
    #[serde(default)]
    pub display_name: String,
    pub password: String,
}

pub async fn post(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    jar: CookieJar,
    Form(form): Form<SetupForm>,
) -> Result<axum::response::Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Setup,
        ip,
        crate::handlers::ErrorAs::Html,
    )?;

    let display = if form.display_name.trim().is_empty() {
        None
    } else {
        Some(form.display_name.as_str())
    };

    let outcome = setup::create_initial_admin(
        &app.db,
        &app.clock,
        &app.setup_token,
        form.setup_token.trim(),
        form.username.trim(),
        &form.password,
        display,
    );

    match outcome {
        Ok(_) => {
            // Auto-login the new admin after setup.
            let session_row = session::login(
                &app.db,
                &app.clock,
                form.username.trim(),
                &form.password,
                app.config.security.max_lockout.as_secs(),
            )
                .map_err(HttpError::html)?;
            let cookie = session_cookie(session_row.id.to_string(), app.config.server.cookie_secure);
            let jar = jar.add(cookie);
            Ok((jar, Redirect::to("/admin")).into_response())
        }
        Err(e) => {
            let flash = Flash {
                kind: match &e {
                    CoreError::AlreadyInitialized | CoreError::Forbidden => FlashKind::Error,
                    _ => FlashKind::Warn,
                },
                text: friendly_error_text(&e),
            };
            // Re-render with the form-level flash; do not expose internal causes.
            tracing::warn!(error = %e, "setup form rejected");
            let _ = SESSION_COOKIE;
            Ok((axum::http::StatusCode::BAD_REQUEST, Html(render_setup(Some(flash)))).into_response())
        }
    }
}

fn friendly_error_text(e: &CoreError) -> String {
    match e {
        CoreError::AlreadyInitialized => "This server is already initialized.".into(),
        CoreError::Forbidden => "The setup token is incorrect.".into(),
        CoreError::Conflict(msg) => msg.clone(),
        CoreError::BadRequest(msg) => msg.clone(),
        _ => "Setup failed. Please check the form and try again.".into(),
    }
}
