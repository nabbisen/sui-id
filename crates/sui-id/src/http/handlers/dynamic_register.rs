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
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dynamic_registration_validation::{self as validation, EnvelopeError};
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
    body_bytes: Bytes,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;

    // ── P4/P5: the token must be present, but is not spent yet ────────────────
    //
    // RFC 094 M2a (2026-10-02): consuming the token here, before the body is
    // validated, burned a caller's one-time token on a malformed request and
    // restored nothing. The token is only ever consumed below, inside the
    // same transaction that creates the client — a request that cannot
    // succeed must not spend anything.
    //
    // RFC 095 M3 stage 1: a present-but-malformed token (wrong length,
    // non-hex, uppercase, more than one `Authorization` header, a scheme
    // other than `Bearer`) takes this exact same path as no token at all —
    // the matrix requires the two be indistinguishable to the caller.
    let Some(tok) = validation::extract_registration_bearer_token(&headers) else {
        return Ok(reg_error(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Authorization: Bearer <token> is required for dynamic client registration",
        ));
    };
    // SHA-256 the supplied token for constant-time comparison.
    let token_hash = sha256_hex(&tok);

    // ── RFC 095 M3 stage 1, 1b: the envelope ───────────────────────────────────
    //
    // Bounded, duplicate-member-checked, and member-policy-checked *before*
    // this ever becomes a typed `RegistrationRequest` -- see
    // `dynamic_registration_validation`'s own doc comment for why none of
    // this can happen after a plain `Json<RegistrationRequest>` extraction.
    let envelope = match validation::parse_envelope(&body_bytes) {
        Ok(map) => map,
        Err(EnvelopeError::UnapprovedSoftwareStatement) => {
            return Ok(reg_error(
                StatusCode::BAD_REQUEST,
                "unapproved_software_statement",
                "software_statement is not approved by this deployment",
            ));
        }
        Err(e) => {
            return Ok(reg_error(
                StatusCode::BAD_REQUEST,
                "invalid_client_metadata",
                &e.to_string(),
            ));
        }
    };
    let body: RegistrationRequest =
        match serde_json::from_value(serde_json::Value::Object(envelope)) {
            Ok(b) => b,
            Err(e) => {
                return Ok(reg_error(
                    StatusCode::BAD_REQUEST,
                    "invalid_client_metadata",
                    &format!("invalid client metadata: {e}"),
                ));
            }
        };

    // ── Validate request body ─────────────────────────────────────────────────

    if body.redirect_uris.is_empty() {
        return Ok(reg_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "redirect_uris must contain at least one URI",
        ));
    }
    // Determine confidentiality from token_endpoint_auth_method, moved up
    // from below: the closed-profile derivation needs it too, and this is
    // the one place that decides what the raw string means.
    let auth_method = body
        .token_endpoint_auth_method
        .as_deref()
        .unwrap_or("client_secret_post");
    let confidential = auth_method != "none";

    // RFC 095 M3 stage 1, 1a: the derived closed profile. Replaces the old
    // per-URI `validate_redirect_uri` loop -- that function is the
    // administrator-client validator (looser: accepts `localhost` by
    // name, no profile concept) and must not be tightened for this path.
    // PKCE is already mandatory for every client regardless of profile
    // (`sui_id_core::oidc::authorize::begin_authorization`), so the
    // matrix's "PKCE required" for the two public profiles needs nothing
    // further here.
    let profile = match validation::derive_closed_profile(auth_method, &body.redirect_uris) {
        Ok(p) => p,
        Err(e) => {
            return Ok(reg_error(
                StatusCode::BAD_REQUEST,
                "invalid_redirect_uri",
                &e.to_string(),
            ));
        }
    };
    tracing::debug!(?profile, "dynamic registration: redirect profile derived");
    if let Some(post_logout) = body.post_logout_redirect_uris.as_ref()
        && let Err(bad_uri) = validation::reject_http_post_logout_uris(post_logout)
    {
        return Ok(reg_error(
            StatusCode::BAD_REQUEST,
            "invalid_client_metadata",
            &format!("post_logout_redirect_uris: {bad_uri}: must be https"),
        ));
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

    // ── Create client row ─────────────────────────────────────────────────────

    let secret_plain = if confidential {
        Some(sui_id_core::tokens::random_token(32))
    } else {
        None
    };
    let secret_hash = match secret_plain.as_deref() {
        Some(s) => Some(
            sui_id_core::password::hash_password(s)
                .await
                .map_err(HttpError::api)?,
        ),
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
        // RFC 136 D1: `FirstTime`, not `ConsentPolicy::default()`. A
        // client that registered itself through this protocol endpoint
        // is, by construction, not first-party -- no administrator ever
        // looked at it -- so it must not get the no-consent default that
        // exists for the administrator-created case
        // (`admin/clients.rs`'s own use of that default). Keep this set
        // explicit even though it looks redundant next to the enum's
        // `#[default]`: that default is for a different call site's
        // different, legitimate case, not a fallback this one should
        // ever fall into silently.
        consent_policy: ConsentPolicy::FirstTime,
        registered_via: RegistrationSource::Dynamic,
        logo_uri: logo_uri.clone(),
        homepage_uri: homepage_uri.clone(),
        privacy_policy_uri: privacy_policy_uri.clone(),
        tos_uri: tos_uri.clone(),
        created_at: now,
        updated_at: now,
    };

    // RFC 094 C15: the token is consumed, the client row created, and its
    // `registered_via` stamped, all in one transaction. A token that
    // turns out invalid, expired, revoked, or exhausted here rolls back
    // with `NotFound`, the same convention U10 (`consume_and_reset_password`)
    // established for a token-presenter's "nothing to do" outcome.
    match sui_id_store::commands::register_client_dynamically(&app.db, token_hash, row, now).await {
        Ok(_) => {}
        Err(sui_id_store::StoreError::NotFound) => {
            return Ok(reg_error(
                StatusCode::BAD_REQUEST,
                "invalid_token",
                "Registration token is invalid, expired, or exhausted.",
            ));
        }
        Err(e) => return Err(HttpError::api(CoreError::from(e))),
    }

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
