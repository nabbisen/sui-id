//! HTTP error response helpers.
//!
//! The pattern: handlers return `Result<R, HttpError>`. `HttpError` carries a
//! [`CoreError`] plus the request id. On `IntoResponse`, we map to either a
//! JSON body (for API endpoints) or a rendered HTML error page (for browser
//! endpoints). The decision is made at the `From` site: API handlers wrap
//! errors with [`HttpError::api`], browser handlers with [`HttpError::html`].

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use sui_id_core::errors::CoreError;
use sui_id_shared::errors::{ApiError, ApiErrorCode};
use uuid::Uuid;

#[derive(Debug)]
pub struct HttpError {
    inner: CoreError,
    request_id: String,
    representation: ErrorRepresentation,
    forced_status: Option<StatusCode>,
    retry_after_secs: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
enum ErrorRepresentation {
    Json,
    Html,
}

impl HttpError {
    pub fn api(err: CoreError) -> Self {
        Self {
            inner: err,
            request_id: new_req_id(),
            representation: ErrorRepresentation::Json,
            forced_status: None,
            retry_after_secs: None,
        }
    }

    pub fn html(err: CoreError) -> Self {
        Self {
            inner: err,
            request_id: new_req_id(),
            representation: ErrorRepresentation::Html,
            forced_status: None,
            retry_after_secs: None,
        }
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
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
        let code = self.inner.api_code();
        let status = self.forced_status.unwrap_or_else(|| {
            StatusCode::from_u16(code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
        });

        // Always log the internal cause; never put it in the response body.
        tracing::warn!(
            request_id = %self.request_id,
            code = ?code,
            error = %self.inner,
            "request failed"
        );

        let mut resp = match self.representation {
            ErrorRepresentation::Json => {
                let payload = build_api_error(&self.inner, &self.request_id, code);
                (status, Json(payload)).into_response()
            }
            ErrorRepresentation::Html => {
                let title = match code {
                    ApiErrorCode::Unauthorized => "Sign in required",
                    ApiErrorCode::Forbidden => "Forbidden",
                    ApiErrorCode::NotFound => "Not found",
                    ApiErrorCode::Conflict => "Conflict",
                    ApiErrorCode::BadRequest => "Bad request",
                    ApiErrorCode::InvalidState => "Wrong system state",
                    ApiErrorCode::TooManyRequests => "Too many requests",
                    ApiErrorCode::Protocol => "Protocol error",
                    ApiErrorCode::Internal => "Something went wrong",
                };
                let safe_message = safe_user_message(&self.inner);
                let body = sui_id_web::render_error(
                    title.to_owned(),
                    safe_message,
                    self.request_id.clone(),
                );
                (status, Html(body)).into_response()
            }
        };
        if let Some(secs) = self.retry_after_secs {
            if let Ok(value) = axum::http::HeaderValue::from_str(&secs.to_string()) {
                resp.headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, value);
            }
        }
        resp
    }
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
        CoreError::Store(_) | CoreError::Password | CoreError::Jwt | CoreError::Internal => {
            "An internal error occurred.".into()
        }
    }
}
