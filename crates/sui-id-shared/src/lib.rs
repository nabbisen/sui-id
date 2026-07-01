//! # sui-id-shared
//!
//! Shared types crossing crate boundaries. Kept intentionally small: only DTOs,
//! protocol-level enums, and a public-facing API error type live here. Internal
//! domain logic stays in `sui-id-core`.

#![forbid(unsafe_code)]

pub mod api;
pub mod auth_method;
pub mod errors;
pub mod ids;
pub mod secrets;

pub use auth_method::{AuthMethod, acr_from_methods, amr_from_methods};
pub use errors::{ApiError, ApiErrorCode};
pub use secrets::{CodeHash, FamilyId, RawRefreshToken, RefreshTokenHash, RefreshTokenId};

/// Normalise an email address for case-insensitive uniqueness checks and
/// lookup. The original-case form is preserved separately in `UserRow.email`
/// for display purposes; this function produces the canonical form used for
/// database indexing and forgot-password lookup.
///
/// Rule: trim leading/trailing whitespace, then convert to lowercase.
/// We do not perform full RFC 5321 local-part canonicalisation (dots,
/// plus-addressing) — those vary by provider and are not universally
/// applicable.
pub fn normalize_email(input: &str) -> String {
    input.trim().to_lowercase()
}
