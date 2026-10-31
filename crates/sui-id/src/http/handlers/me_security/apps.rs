//! `/me/apps` handlers — self-service consent grant review (RFC 072, v0.60.0).

use axum::Form;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use sui_id_shared::ids::ClientId;
use sui_id_web::pages::me_security::apps::{AppGrantData, MeAppsData};
use sui_id_web::render_me_apps;

use crate::errors::HttpError;
use crate::handlers::admin::with_csrf_cookie;
use crate::handlers::{AppStateExt, CurrentUser, RequestLocale, enforce_csrf};
use sui_id_core::CoreError;

/// GET /me/apps — list consent grants the signed-in user has issued.
pub async fn me_apps_get(
    State(app): AppStateExt,
    CurrentUser(user_id): CurrentUser,
    RequestLocale(lang): RequestLocale,
    jar: CookieJar,
) -> Result<Response, HttpError> {
    let grants_raw = sui_id_store::repos::user_consent::list_for_user(&app.db, user_id)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    let grants = grants_raw
        .into_iter()
        .map(|g| AppGrantData {
            client_id: g.client_id.to_string(),
            client_name: g.client_name,
            granted_scopes: g
                .granted_scopes
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            granted_at: g.granted_at,
            last_used_at: g.last_used_at,
        })
        .collect();

    let token = crate::csrf::ensure_token(&jar);
    let data = MeAppsData {
        grants,
        csrf_token: token.clone(),
        dev_mode: app.is_dev_mode,
    };

    let resp = Html(render_me_apps(data, None, lang)).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(serde::Deserialize)]
pub struct RevokeForm {
    #[serde(rename = "_csrf")]
    pub csrf: String,
}

/// POST /me/apps/{client_id}/revoke — remove a consent grant and
/// all associated refresh tokens atomically.
pub async fn me_apps_revoke(
    State(app): AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Path(client_id_str): Path<String>,
    Form(form): Form<RevokeForm>,
) -> Result<Response, HttpError> {
    enforce_csrf(&jar, Some(form.csrf.as_str()))?;

    let client_id: ClientId = client_id_str
        .parse()
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid client id".into())))?;

    // Cycle I stage 3, U20: the sealed command returns `NotFound` when
    // there was no grant to revoke, and writes no audit row for a
    // revocation that did not happen. That is the store's job. It is not
    // an error to show the user: the only ways to reach it are a
    // double-submit or a stale page, and in both the grant they wanted
    // gone is gone. `me_security/sessions.rs`'s `revoke_one` already
    // treats an unknown session id the same way, in the same `/me` area —
    // redirect back to the page rather than an error.
    match sui_id_store::commands::revoke_consent(&app.db, user_id, client_id).await {
        Ok(_) => {}
        Err(sui_id_store::StoreError::NotFound) => {}
        Err(e) => return Err(HttpError::html(CoreError::from(e))),
    }

    Ok(Redirect::to("/me/apps").into_response())
}
