//! HTTP handlers for the RFC 7662 introspection and RFC 7009
//! revocation endpoints.
//!
//! Both accept the request body as `application/x-www-form-urlencoded`
//! and authenticate the calling client via either HTTP Basic
//! (preferred) or `client_id` + `client_secret` form fields.

use crate::errors::HttpError;
use crate::handlers::AppStateExt;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Form, Json};
use serde::{Deserialize, Serialize};
use sui_id_core::errors::CoreError;

#[derive(Debug, Deserialize)]
pub struct IntrospectForm {
    pub token: String,
    #[serde(default)]
    pub token_type_hint: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
}

/// Wire format for `/oauth2/introspect` per RFC 7662 §2.2.
///
/// All fields except `active` are skipped on serialise when None;
/// when `active` is false, all the others must be omitted.
#[derive(Debug, Serialize)]
pub struct IntrospectionWire {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iat: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
}

pub async fn introspect(
    state_ext: AppStateExt,
    headers: HeaderMap,
    Form(form): Form<IntrospectForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let (client_id, client_secret) =
        client_credentials(&headers, &form.client_id, &form.client_secret)
            .ok_or_else(|| HttpError::oauth(CoreError::Unauthenticated))?;
    let cid = sui_id_core::oauth_token::authenticate_client(&app.db, &client_id, &client_secret)
        .await
        .map_err(HttpError::oauth)?;
    let resp = sui_id_core::oauth_token::introspect(
        &app.db,
        &app.clock,
        cid,
        &form.token,
        form.token_type_hint.as_deref(),
    )
    .await
    .map_err(HttpError::oauth)?;
    let wire = IntrospectionWire {
        active: resp.active,
        scope: resp.scope,
        client_id: resp.client_id,
        username: resp.username,
        token_type: resp.token_type,
        exp: resp.exp,
        iat: resp.iat,
        sub: resp.sub,
        aud: resp.aud,
        iss: resp.iss,
    };
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: None,
            action: "token.introspect".into(),
            target: Some(cid.to_string()),
            result: if resp.active {
                "active".into()
            } else {
                "inactive".into()
            },
            note: resp.kind.map(|k| k.to_string()),
        },
    )
    .await;
    Ok(Json(wire).into_response())
}

#[derive(Debug, Deserialize)]
pub struct RevokeForm {
    pub token: String,
    #[serde(default)]
    pub token_type_hint: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
}

pub async fn revoke(
    state_ext: AppStateExt,
    headers: HeaderMap,
    Form(form): Form<RevokeForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let (client_id, client_secret) =
        client_credentials(&headers, &form.client_id, &form.client_secret)
            .ok_or_else(|| HttpError::oauth(CoreError::Unauthenticated))?;
    let cid = sui_id_core::oauth_token::authenticate_client(&app.db, &client_id, &client_secret)
        .await
        .map_err(HttpError::oauth)?;
    sui_id_core::oauth_token::revoke(
        &app.db,
        &app.clock,
        cid,
        &form.token,
        form.token_type_hint.as_deref(),
    )
    .await
    .map_err(HttpError::oauth)?;
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: app.clock.now(),
            actor: None,
            action: "token.revoke".into(),
            target: Some(cid.to_string()),
            result: "ok".into(),
            note: form.token_type_hint,
        },
    )
    .await;
    // RFC 7009 §2.2: 200 OK with empty body.
    Ok(StatusCode::OK.into_response())
}

/// Pull the client credentials from either an HTTP Basic header or
/// the form body (RFC 6749 §2.3.1 calls Basic preferred; we accept
/// both for ergonomic reasons).
fn client_credentials(
    headers: &HeaderMap,
    form_client_id: &Option<String>,
    form_client_secret: &Option<String>,
) -> Option<(String, String)> {
    if let Some((id, secret)) = crate::handlers::oidc::parse_basic_auth(headers) {
        return Some((id, secret));
    }
    match (form_client_id, form_client_secret) {
        (Some(i), Some(s)) => Some((i.clone(), s.clone())),
        _ => None,
    }
}
