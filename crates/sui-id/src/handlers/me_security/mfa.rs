//! /me/security mfa tab handlers (RFC 068).

use crate::{csrf, errors::HttpError};
use axum::extract::{Form, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use sui_id_core::errors::CoreError;
use sui_id_store::repos::user_totp;


use super::forms::*;
use crate::handlers::{AppStateExt, CurrentUser, enforce_csrf};
use crate::handlers::admin::with_csrf_cookie;
use sui_id_web::pages::{MeShellData, MeTab};

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
    ctx: crate::handlers::SessionContext,
    jar: CookieJar,
    Form(form): Form<crate::handlers::admin::CsrfOnlyForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    // RFC 058: dangerous self-service action — disabling MFA reduces
    // the user's own account security and is one of the canonical
    // post-compromise attacker moves. Step-up is required.
    if let Err(redirect) =
        crate::handlers::require_fresh_step_up(&app, &ctx, "/me/security/mfa").await
    {
        return Ok(redirect);
    }
    sui_id_core::mfa::disable(&app.db, user_id).await.map_err(HttpError::html)?;
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: Some(user_id),
            action: "mfa.disable".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: Some("self".into()),
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
