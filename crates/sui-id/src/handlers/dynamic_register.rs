//! `POST /oauth2/register` — RFC 7591 dynamic client registration (RFC 008).
//!
//! # Flow
//!
//! 1. Validate the `Authorization: Bearer <token>` header against
//!    `client_registration_token` (P4/P5).
//! 2. Validate the JSON body: `redirect_uris` required, `client_name` required,
//!    application-identity URIs validated HTTPS-or-localhost (P6).
//! 3. Create the client row with `registered_via = 'dynamic'`,
//!    `is_disabled = true` (admin must explicitly enable), and
//!    `consent_policy = 'first_time_only'` (sensible default for third-party).
//! 4. Return the RFC 7591 `ClientInformation` response.
//!
//! # Security
//!
//! - No token → 401.  Expired, revoked, or exhausted token → 400
//!   `invalid_token`.
//! - Open registration (no token required) is a future knob defaulting off.
//! - Dynamically registered clients start disabled so an operator must
//!   consciously enable them before they can obtain tokens (P4).

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::HttpError;
use crate::handlers::AppStateExt;
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::ClientId;
use sui_id_store::models::{ClientRow, ConsentPolicy, RegistrationSource};

// ── RFC 7591 request body ─────────────────────────────────────────────────────

/// RFC 7591 §2 client metadata — the request body of `POST /oauth2/register`.
#[derive(Debug, Deserialize)]
pub struct RegistrationRequest {
    /// REQUIRED.
    pub redirect_uris: Vec<String>,
    /// Human-readable client name. Required by this deployment (not RFC 7591).
    pub client_name: Option<String>,
    /// Space-separated list of requested scopes. Empty or absent → any scope.
    pub scope: Option<String>,
    /// Grant types: "authorization_code" (default), "refresh_token".
    pub grant_types: Option<Vec<String>>,
    /// Token endpoint auth method: "client_secret_post" or "none".
    pub token_endpoint_auth_method: Option<String>,
    /// Application-identity URIs (P6).
    pub logo_uri: Option<String>,
    pub client_uri: Option<String>,
    pub policy_uri: Option<String>,
    pub tos_uri: Option<String>,
    pub post_logout_redirect_uris: Option<Vec<String>>,
}

// ── RFC 7591 response body ────────────────────────────────────────────────────

/// RFC 7591 §3.2.1 client information response.
#[derive(Debug, Serialize)]
pub struct RegistrationResponse {
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub token_endpoint_auth_method: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tos_uri: Option<String>,
}

// ── Error helper ──────────────────────────────────────────────────────────────

fn reg_error(status: StatusCode, error: &str, description: &str) -> Response {
    #[derive(Serialize)]
    struct RegError<'a> {
        error: &'a str,
        error_description: &'a str,
    }
    (
        status,
        Json(RegError {
            error,
            error_description: description,
        }),
    )
        .into_response()
}

// ── Handler ───────────────────────────────────────────────────────────────────

/// `POST /oauth2/register` — RFC 7591 dynamic client registration.
pub async fn dynamic_register(
    state_ext: AppStateExt,
    headers: HeaderMap,
    Json(body): Json<RegistrationRequest>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    // ── P4/P5: validate registration token ───────────────────────────────────

    let raw_token = extract_bearer_token(&headers);
    match raw_token {
        None => {
            return Ok(reg_error(
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                "Authorization: Bearer <token> is required for dynamic client registration",
            ));
        }
        Some(tok) => {
            // SHA-256 the supplied token for constant-time comparison.
            let hash = sha256_hex(tok);
            let valid = sui_id_store::repos::client_registration_token::consume(
                &app.db,
                &hash,
                app.clock.now(),
            )
            .await
            .map_err(|e| HttpError::api(CoreError::from(e)))?;

            if !valid {
                return Ok(reg_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_token",
                    "Registration token is invalid, expired, or exhausted.",
                ));
            }
        }
    }

    // ── Validate request body ─────────────────────────────────────────────────

    if body.redirect_uris.is_empty() {
        return Ok(reg_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "redirect_uris must contain at least one URI",
        ));
    }
    for uri in &body.redirect_uris {
        sui_id_core::admin::clients::validate_redirect_uri(uri).map_err(|e| HttpError::api(e))?;
    }

    let client_name = match body.client_name.as_deref().filter(|s| !s.is_empty()) {
        Some(n) => n.to_owned(),
        None => {
            return Ok(reg_error(
                StatusCode::BAD_REQUEST,
                "invalid_client_metadata",
                "client_name is required",
            ));
        }
    };

    // Validate application-identity URIs (P6).
    let logo_uri = validated_uri(body.logo_uri, "logo_uri")?;
    let homepage_uri = validated_uri(body.client_uri, "client_uri")?;
    let privacy_policy_uri = validated_uri(body.policy_uri, "policy_uri")?;
    let tos_uri = validated_uri(body.tos_uri, "tos_uri")?;

    // Determine confidentiality from token_endpoint_auth_method.
    let auth_method = body
        .token_endpoint_auth_method
        .as_deref()
        .unwrap_or("client_secret_post");
    let confidential = auth_method != "none";

    // ── Create client row ─────────────────────────────────────────────────────

    let secret_plain = if confidential {
        Some(sui_id_core::tokens::random_token(32))
    } else {
        None
    };
    let secret_hash = match secret_plain.as_deref() {
        Some(s) => Some(sui_id_core::password::hash_password(s).map_err(|e| HttpError::api(e))?),
        None => None,
    };

    let scope = body.scope.clone().unwrap_or_default();
    let post_logout = body.post_logout_redirect_uris.clone().unwrap_or_default();
    let now = app.clock.now();
    let client_id = ClientId::new();

    let row = ClientRow {
        id: client_id,
        name: client_name.clone(),
        confidential,
        secret_hash,
        redirect_uris: body.redirect_uris.clone(),
        allowed_scopes: scope.clone(),
        post_logout_redirect_uris: post_logout,
        // Dynamically registered clients start DISABLED — admin must enable.
        is_disabled: true,
        is_deleted: false,
        // Sensible default for third-party clients.
        consent_policy: ConsentPolicy::FirstTime,
        registered_via: RegistrationSource::Dynamic,
        logo_uri: logo_uri.clone(),
        homepage_uri: homepage_uri.clone(),
        privacy_policy_uri: privacy_policy_uri.clone(),
        tos_uri: tos_uri.clone(),
        created_at: now,
        updated_at: now,
    };

    sui_id_store::repos::clients::create(&app.db, &row)
        .await
        .map_err(|e| HttpError::api(CoreError::from(e)))?;

    // Audit the dynamic registration.
    let _ = sui_id_store::repos::audit::append(
        &app.db,
        &sui_id_store::models::AuditLogRow {
            at: now,
            actor: None,
            action: "client.dynamic_register".into(),
            target: Some(client_id.to_string()),
            result: "ok".into(),
            note: Some(format!("name={client_name}")),
        },
    )
    .await;

    // ── RFC 7591 §3.2.1 response ──────────────────────────────────────────────

    let grant_types = body
        .grant_types
        .clone()
        .unwrap_or_else(|| vec!["authorization_code".into(), "refresh_token".into()]);

    let resp = RegistrationResponse {
        client_id: client_id.to_string(),
        client_secret: secret_plain,
        client_name,
        redirect_uris: body.redirect_uris,
        grant_types,
        token_endpoint_auth_method: auth_method.to_owned(),
        scope,
        logo_uri,
        client_uri: homepage_uri,
        policy_uri: privacy_policy_uri,
        tos_uri,
    };

    Ok((StatusCode::CREATED, Json(resp)).into_response())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn sha256_hex(input: &str) -> String {
    let hash = Sha256::digest(input.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

fn validated_uri(uri: Option<String>, field: &'static str) -> Result<Option<String>, HttpError> {
    match uri {
        None => Ok(None),
        Some(u) if u.is_empty() => Ok(None),
        Some(u) => {
            if sui_id_store::repos::clients::is_valid_app_uri(&u) {
                Ok(Some(u))
            } else {
                Err(HttpError::api(CoreError::BadRequest(format!(
                    "field {field}: must be HTTPS (or http://localhost): {u}"
                ))))
            }
        }
    }
}
