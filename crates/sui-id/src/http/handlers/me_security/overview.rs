//! /me/security overview tab handlers (RFC 068).

use crate::{csrf, errors::HttpError};
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use sui_id_core::errors::CoreError;
use sui_id_store::repos::{audit, sessions, user_totp};

use crate::handlers::admin::with_csrf_cookie;
use crate::handlers::{AppStateExt, CurrentUser};
use sui_id_web::pages::{MeOverviewData, MeRecoveryEvent, MeShellData, MeTab};

/// Revoke a single session belonging to the current user.
///
/// The `id` path parameter is parsed and then cross-checked against
/// the database: the row must exist *and* belong to the current
/// user. We don't tell the caller which check failed — both refuse
/// with the same 404-shaped redirect, so there's no oracle for
/// guessing other people's session ids.
pub async fn overview_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let lang = req_locale;
    let user = sui_id_store::repos::users::get(&app.db, user_id)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)).with_lang(lang))?;
    let shell = MeShellData {
        username: user.username,
        role: user.role,
        active_tab: MeTab::Overview,
    };
    let totp_enabled = user_totp::get(&app.db, user_id)
        .await
        .ok()
        .flatten()
        .map(|r| r.enabled)
        .unwrap_or(false);
    let passkey_count =
        sui_id_store::repos::user_webauthn_credentials::count_for_user(&app.db, user_id)
            .await
            .unwrap_or(0);
    let active_session_count = sessions::list_active_for_user(&app.db, user_id)
        .await
        .map(|v| v.len())
        .unwrap_or(0);
    let recent_events: Vec<sui_id_web::MeAuditEntry> = audit::recent_for_user(&app.db, user_id, 10)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| sui_id_web::MeAuditEntry {
            at: r.at,
            action: r.action,
            result: r.result,
            note: r.note,
        })
        .collect();
    // RFC 103 5b: the user's own most recent recovery-link event, if any.
    let recovery_event = audit::most_recent_recovery_event_for_user(&app.db, user_id)
        .await
        .ok()
        .flatten()
        .and_then(|row| sui_id_core::recovery_link::summarize_recent_event(&row))
        .map(|summary| match summary {
            sui_id_core::recovery_link::RecoveryEventSummary::Issued { by_admin: true, at } => {
                MeRecoveryEvent::IssuedByAdmin(at)
            }
            sui_id_core::recovery_link::RecoveryEventSummary::Issued {
                by_admin: false,
                at,
            } => MeRecoveryEvent::IssuedByOperator(at),
            sui_id_core::recovery_link::RecoveryEventSummary::Completed {
                origin: sui_id_store::models::ResetTokenOrigin::Email,
                at,
            } => MeRecoveryEvent::CompletedSelfService(at),
            sui_id_core::recovery_link::RecoveryEventSummary::Completed {
                origin: sui_id_store::models::ResetTokenOrigin::Web,
                at,
            } => MeRecoveryEvent::CompletedByAdmin(at),
            sui_id_core::recovery_link::RecoveryEventSummary::Completed {
                origin: sui_id_store::models::ResetTokenOrigin::Cli,
                at,
            } => MeRecoveryEvent::CompletedByOperator(at),
        });
    let csrf_tok = csrf::ensure_token(&jar);
    let resp = axum::response::Html(sui_id_web::render_me_overview(
        MeOverviewData {
            shell,
            totp_enabled,
            passkey_count,
            active_session_count,
            recent_events,
            csrf_token: csrf_tok.clone(),
            last_login_at: user.last_login_at,
            recovery_event,
        },
        app.is_dev_mode,
        lang,
    ))
    .into_response();
    Ok(with_csrf_cookie(resp, &app, &csrf_tok))
}
