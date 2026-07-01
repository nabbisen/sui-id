//! Public-facing API error type.
//!
//! Internal causes are deliberately not embedded in the user-visible payload.
//! Each error carries an opaque request id so that operators can correlate
//! a user complaint with detailed server-side log entries without exposing
//! internal information to the client.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable error code returned to API callers. The set is intentionally small
/// so that integrators can branch on a finite alphabet rather than parsing
/// human-readable messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    /// The caller is not authenticated, or credentials are invalid.
    Unauthorized,
    /// The caller is authenticated but lacks permission for this operation.
    Forbidden,
    /// The request payload failed validation.
    BadRequest,
    /// The requested resource does not exist.
    NotFound,
    /// The request conflicts with current state (e.g. duplicate resource).
    Conflict,
    /// The system is in setup mode and the requested endpoint is not allowed
    /// until initialization is complete (or the endpoint requires setup mode).
    InvalidState,
    /// Caller is being rate-limited.
    TooManyRequests,
    /// An OAuth 2.0 / OIDC protocol error. The exact `error` field follows
    /// the relevant RFCs (e.g. `invalid_grant`, `invalid_client`).
    Protocol,
    /// Catch-all for anything we did not anticipate. The body deliberately
    /// does not carry internal cause; correlate via `request_id`.
    Internal,
}

impl ApiErrorCode {
    /// HTTP status code commonly associated with this error.
    pub const fn http_status(self) -> u16 {
        match self {
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::BadRequest | Self::Protocol => 400,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::InvalidState => 409,
            Self::TooManyRequests => 429,
            Self::Internal => 500,
        }
    }
}

/// Wire-format error payload returned by the JSON API.
#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[error("{code:?}: {message}")]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    /// Opaque correlation id. Always present; clients should surface this
    /// when reporting a problem.
    pub request_id: String,
    /// Optional protocol-specific subcode, e.g. OAuth `error` field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_code: Option<String>,
}

impl ApiError {
    pub fn new(
        code: ApiErrorCode,
        message: impl Into<String>,
        request_id: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            request_id: request_id.into(),
            protocol_code: None,
        }
    }

    pub fn with_protocol_code(mut self, sub: impl Into<String>) -> Self {
        self.protocol_code = Some(sub.into());
        self
    }
}

#[cfg(test)]
mod tests;
