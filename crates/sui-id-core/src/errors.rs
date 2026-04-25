//! Core domain error type.
//!
//! Variants are categorised so that the HTTP layer can map them to the
//! correct `ApiErrorCode` without inspecting the underlying cause.

use sui_id_shared::errors::ApiErrorCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("authentication required")]
    Unauthenticated,

    #[error("forbidden")]
    Forbidden,

    #[error("resource not found")]
    NotFound,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("invalid request: {0}")]
    BadRequest(String),

    #[error("system not yet initialized")]
    NotInitialized,

    #[error("system already initialized")]
    AlreadyInitialized,

    #[error("OAuth/OIDC protocol error: {code}")]
    Protocol {
        code: ProtocolError,
        description: String,
    },

    #[error(transparent)]
    Store(#[from] sui_id_store::StoreError),

    #[error("password hashing failure")]
    Password,

    #[error("JWT processing failure")]
    Jwt,

    #[error("internal failure")]
    Internal,
}

/// Subset of the OAuth 2.0 / OIDC error vocabulary that sui-id may emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidRequest,
    InvalidClient,
    InvalidGrant,
    UnauthorizedClient,
    UnsupportedGrantType,
    InvalidScope,
    UnsupportedResponseType,
    AccessDenied,
    ServerError,
}

impl ProtocolError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidClient => "invalid_client",
            Self::InvalidGrant => "invalid_grant",
            Self::UnauthorizedClient => "unauthorized_client",
            Self::UnsupportedGrantType => "unsupported_grant_type",
            Self::InvalidScope => "invalid_scope",
            Self::UnsupportedResponseType => "unsupported_response_type",
            Self::AccessDenied => "access_denied",
            Self::ServerError => "server_error",
        }
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl CoreError {
    /// Map this error to a stable wire-level error code.
    pub fn api_code(&self) -> ApiErrorCode {
        match self {
            Self::InvalidCredentials | Self::Unauthenticated => ApiErrorCode::Unauthorized,
            Self::Forbidden => ApiErrorCode::Forbidden,
            Self::NotFound => ApiErrorCode::NotFound,
            Self::Conflict(_) => ApiErrorCode::Conflict,
            Self::BadRequest(_) => ApiErrorCode::BadRequest,
            Self::NotInitialized | Self::AlreadyInitialized => ApiErrorCode::InvalidState,
            Self::Protocol { .. } => ApiErrorCode::Protocol,
            Self::Store(sui_id_store::StoreError::NotFound) => ApiErrorCode::NotFound,
            Self::Store(sui_id_store::StoreError::Conflict) => ApiErrorCode::Conflict,
            Self::Store(_) | Self::Password | Self::Jwt | Self::Internal => ApiErrorCode::Internal,
        }
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
