//! HTTP error response helpers.
//!
//! The pattern: handlers return `Result<R, HttpError>`. `HttpError` carries a
//! [`CoreError`] plus the request id. On `IntoResponse`, we map to either a
//! JSON body (for API endpoints), a rendered HTML error page (for browser
//! endpoints), or an RFC 6749 wire-format body (for OAuth protocol endpoints).
//!
//! - [`HttpError::api`] — internal JSON envelope (`{"code":...,"protocol_code":...}`)
//!   for admin API and UI handlers.
//! - [`HttpError::html`] — HTML error page for browser-facing handlers.
//! - [`HttpError::oauth`] — **RFC 6749 §5.2** JSON body
//!   (`{"error":...,"error_description":...}`) for the token, introspect, and
//!   revoke endpoints. Integrators depend on this exact wire format.

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use sui_id_core::errors::{CoreError, ProtocolError};
use sui_id_shared::errors::{ApiError, ApiErrorCode};
use uuid::Uuid;

#[derive(Debug)]
pub struct HttpError {
    inner: CoreError,
    request_id: String,
    representation: ErrorRepresentation,
    forced_status: Option<StatusCode>,
    retry_after_secs: Option<i64>,
    /// Locale for HTML error page rendering (RFC 042).
    /// Defaults to Ja; set via `.with_lang(loc)` in handlers that resolve locale.
    pub lang: sui_id_i18n::Locale,
}

#[derive(Debug, Clone, Copy)]
enum ErrorRepresentation {
    Json,
    Html,
    /// RFC 6749 §5.2 wire format: `{"error":"...","error_description":"..."}`
    OAuth,
}

/// RFC 6749 §5.2 / RFC 7009 / RFC 7662 error response body.
///
/// The `error` field contains a registered error code (e.g. `"invalid_grant"`).
/// The `error_description` field SHOULD contain a human-readable explanation
/// in ASCII. It is `skip_serializing_if = "Option::is_none"` because some error
/// paths do not have a natural description.
#[derive(Debug, Serialize)]
pub struct OAuthErrorBody {
    pub error: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

impl HttpError {
    pub fn api(err: CoreError) -> Self {
        Self {
            inner: err,
            request_id: new_req_id(),
            representation: ErrorRepresentation::Json,
            forced_status: None,
            retry_after_secs: None,
            lang: sui_id_i18n::Locale::default(),
        }
    }

    pub fn html(err: CoreError) -> Self {
        Self {
            inner: err,
            request_id: new_req_id(),
            representation: ErrorRepresentation::Html,
            forced_status: None,
            retry_after_secs: None,
            lang: sui_id_i18n::Locale::default(),
        }
    }

    /// Build an error response in RFC 6749 §5.2 wire format.
    ///
    /// Use this for all OAuth/OIDC **protocol endpoints** (token, introspect,
    /// revoke). OIDC integrators expect `{"error":"...","error_description":"..."}`
    /// here; the internal API envelope is inappropriate for these endpoints.
    ///
    /// Status codes per RFC 6749:
    /// - `invalid_client` authentication failures → **401 Unauthorized** with
    ///   a `WWW-Authenticate: Basic realm="sui-id"` header.
    /// - All other protocol errors → **400 Bad Request**.
    /// - Non-protocol internal errors → **500** (opaque, no `error_description`).
    ///
    /// The `WWW-Authenticate` header is attached in [`IntoResponse`] when the
    /// error resolves to `invalid_client`.
    pub fn oauth(err: CoreError) -> Self {
        Self {
            inner: err,
            request_id: new_req_id(),
            representation: ErrorRepresentation::OAuth,
            forced_status: None,
            retry_after_secs: None,
            lang: sui_id_i18n::Locale::default(),
        }
    }

    /// Convenience for handlers that want to render a 404 inside
    /// the HTML chrome (used by feature-gated routes like the
    /// forgot-password endpoints when SMTP is unconfigured).
    pub fn not_found_html() -> Self {
        let mut e = Self::html(CoreError::NotFound);
        e.force_status(StatusCode::NOT_FOUND);
        e
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Set the locale for HTML error page rendering (RFC 042).
    pub fn with_lang(mut self, lang: sui_id_i18n::Locale) -> Self {
        self.lang = lang;
        self
    }

    pub fn inner(&self) -> &CoreError {
        &self.inner
    }

    pub fn force_status(&mut self, status: StatusCode) {
        self.forced_status = Some(status);
    }

    pub fn set_retry_after_secs(&mut self, secs: i64) {
        self.retry_after_secs = Some(secs);
    }
}

fn new_req_id() -> String {
    Uuid::new_v4().simple().to_string()
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        // Always log the internal cause; never put it in the response body.
        let code = self.inner.api_code();
        tracing::warn!(
            request_id = %self.request_id,
            code = ?code,
            error = %self.inner,
            "request failed"
        );

        match self.representation {
            ErrorRepresentation::OAuth => oauth_error_response(self),
            ErrorRepresentation::Json => json_error_response(self, code),
            ErrorRepresentation::Html => html_error_response(self, code),
        }
    }
}

/// Produce an RFC 6749 §5.2 wire-format response.
fn oauth_error_response(e: HttpError) -> Response {
    let (status, error_code, description) = match &e.inner {
        CoreError::Protocol { code, description } => {
            let status = match code {
                ProtocolError::InvalidClient => StatusCode::UNAUTHORIZED,
                ProtocolError::TemporarilyUnavailable => StatusCode::TOO_MANY_REQUESTS,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, code.as_str(), Some(description.clone()))
        }
        CoreError::Unauthenticated | CoreError::InvalidCredentials => {
            (StatusCode::UNAUTHORIZED, "invalid_client", Some("client authentication failed".into()))
        }
        _ => {
            // Do not leak internal error details over the protocol wire.
            (StatusCode::INTERNAL_SERVER_ERROR, "server_error", None)
        }
    };

    let status = e.forced_status.unwrap_or(status);
    let body = OAuthErrorBody { error: error_code, error_description: description };

    let mut resp = (status, Json(body)).into_response();

    // RFC 6749 §3.2.1: when the server authenticates the client via HTTP
    // Basic and that authentication fails, it MUST include a
    // WWW-Authenticate header.
    if status == StatusCode::UNAUTHORIZED {
        resp.headers_mut()
            .insert(
                axum::http::header::WWW_AUTHENTICATE,
                axum::http::HeaderValue::from_static("Basic realm=\"sui-id\""),
            );
    }
    // RFC 6749 §5.1 / BCP 212: token endpoint responses must not be cached.
    if let Ok(val) = axum::http::HeaderValue::from_str("no-store") {
        resp.headers_mut()
            .insert(axum::http::header::CACHE_CONTROL, val);
    }
    // Add Retry-After for rate-limit (429) responses.
    if let Some(secs) = e.retry_after_secs {
        if let Ok(val) = axum::http::HeaderValue::from_str(&secs.to_string()) {
            resp.headers_mut()
                .insert(axum::http::header::RETRY_AFTER, val);
        }
    }
    resp
}

fn json_error_response(e: HttpError, code: ApiErrorCode) -> Response {
    let status = e.forced_status.unwrap_or_else(|| {
        StatusCode::from_u16(code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    });
    let payload = build_api_error(&e.inner, &e.request_id, code);
    let mut resp = (status, Json(payload)).into_response();
    if let Some(secs) = e.retry_after_secs {
        if let Ok(value) = axum::http::HeaderValue::from_str(&secs.to_string()) {
            resp.headers_mut()
                .insert(axum::http::header::RETRY_AFTER, value);
        }
    }
    resp
}

fn html_error_response(e: HttpError, code: ApiErrorCode) -> Response {
    let status = e.forced_status.unwrap_or_else(|| {
        StatusCode::from_u16(code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    });
    // Per-status titles previously fed a manual error-page renderer;
    // RFC 042 replaced that path with `sui_id_web::render_error`,
    // which derives the title from the status code internally. The
    // local mapping is no longer needed.
    let status_u16 = status.as_u16();
    let body = sui_id_web::render_error(status_u16, &e.request_id, e.lang);
    let mut resp = (status, Html(body)).into_response();
    if let Some(secs) = e.retry_after_secs {
        if let Ok(value) = axum::http::HeaderValue::from_str(&secs.to_string()) {
            resp.headers_mut()
                .insert(axum::http::header::RETRY_AFTER, value);
        }
    }
    resp
}

fn build_api_error(err: &CoreError, request_id: &str, code: ApiErrorCode) -> ApiError {
    let mut payload = ApiError::new(code, safe_user_message(err), request_id.to_owned());
    if let CoreError::Protocol { code: pe, description } = err {
        payload = payload.with_protocol_code(pe.as_str());
        payload.message = description.clone();
    }
    payload
}

/// Produce a message suitable for surfacing in API or HTML responses.
/// Internal failures are deliberately collapsed to a generic phrasing.
fn safe_user_message(err: &CoreError) -> String {
    match err {
        CoreError::InvalidCredentials | CoreError::Unauthenticated => {
            "Authentication is required.".into()
        }
        CoreError::Forbidden => "You are not allowed to perform this action.".into(),
        CoreError::NotFound => "The requested resource does not exist.".into(),
        CoreError::Conflict(msg) => msg.clone(),
        CoreError::BadRequest(msg) => msg.clone(),
        CoreError::NotInitialized => "This server has not been initialized yet.".into(),
        CoreError::AlreadyInitialized => "This server is already initialized.".into(),
        CoreError::Protocol { description, .. } => description.clone(),
        CoreError::Store(_) | CoreError::Password | CoreError::Jwt | CoreError::Internal
        | CoreError::ConfigError(_) => {
            "An internal error occurred.".into()
        }
    }
}
